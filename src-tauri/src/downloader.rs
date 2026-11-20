//! Model download manager (SPEC FR-6.3): explicit state machine per model —
//! not_downloaded -> downloading (progress % + cancel) -> verifying -> downloaded.
//! Failures keep the .partial for resume; checksum mismatches delete the file.
//!
//! This and the user-initiated catalog fetch are the ONLY permitted network
//! code paths in the app (SPEC §1), plus the localhost-only enhancement client.

use crate::registry::{DownloadKind, ModelEntry, ModelFile};
use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
}

/// Event names shared with the frontend (and mirrored by the browser mock).
pub mod events {
    pub const PROGRESS: &str = "model-download-progress";
    pub const COMPLETE: &str = "model-download-complete";
    pub const CANCELLED: &str = "model-download-cancelled";
    pub const FAILED: &str = "model-download-failed";
    pub const VERIFYING: &str = "model-verification-started";
    pub const VERIFIED: &str = "model-verification-completed";
    pub const DELETED: &str = "model-deleted";
}

/// Tracks in-flight downloads so the UI can cancel them.
#[derive(Default)]
pub struct DownloadManager {
    cancel_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

/// Abstracts `AppHandle::emit` so the CLI and tests can run downloads headless.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: serde_json::Value);
}

pub struct NullSink;
impl EventSink for NullSink {
    fn emit(&self, _event: &str, _payload: serde_json::Value) {}
}

impl DownloadManager {
    pub fn begin(&self, model_id: &str) -> Result<Arc<AtomicBool>> {
        let mut flags = self.cancel_flags.lock().unwrap();
        if flags.contains_key(model_id) {
            bail!("model '{model_id}' is already downloading");
        }
        let flag = Arc::new(AtomicBool::new(false));
        flags.insert(model_id.to_string(), flag.clone());
        Ok(flag)
    }

    pub fn finish(&self, model_id: &str) {
        self.cancel_flags.lock().unwrap().remove(model_id);
    }

    pub fn cancel(&self, model_id: &str) -> bool {
        if let Some(flag) = self.cancel_flags.lock().unwrap().get(model_id) {
            flag.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub fn is_downloading(&self, model_id: &str) -> bool {
        self.cancel_flags.lock().unwrap().contains_key(model_id)
    }
}

pub fn partial_path(entry: &ModelEntry, data_dir: &Path) -> PathBuf {
    entry.dir(data_dir).join(format!("{}.partial", entry.id))
}

/// Where one file of a Files-kind model accumulates until the whole set is
/// verified.
fn file_partial_path(entry: &ModelEntry, data_dir: &Path, file: &ModelFile) -> PathBuf {
    entry.dir(data_dir).join(format!("{}.partial", file.name))
}

pub fn partial_size(entry: &ModelEntry, data_dir: &Path) -> u64 {
    let size = |path: PathBuf| std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    match entry.kind {
        DownloadKind::Files => entry
            .files
            .iter()
            .map(|f| size(file_partial_path(entry, data_dir, f)))
            .sum(),
        _ => size(partial_path(entry, data_dir)),
    }
}

/// Stream `url` onto the end of `partial`, resuming from whatever is already
/// there. `before` is what earlier files of the same model already account
/// for and `total` the model's full size when known up front (Files kind);
/// both only shape the progress events. Returns the partial's final size, or
/// None when the user cancelled (the partial is kept for resume).
async fn fetch_into_partial(
    client: &reqwest::Client,
    url: &str,
    partial: &Path,
    entry: &ModelEntry,
    before: u64,
    total: Option<u64>,
    sink: &dyn EventSink,
    cancel: &AtomicBool,
) -> Result<Option<u64>> {
    // Resume from an existing partial if present.
    let mut downloaded = std::fs::metadata(partial).map(|m| m.len()).unwrap_or(0);
    let mut request = client.get(url);
    if downloaded > 0 {
        request = request.header("Range", format!("bytes={downloaded}-"));
    }
    let response = request.send().await.context("starting download")?;
    let status = response.status();
    if downloaded > 0 && status != reqwest::StatusCode::PARTIAL_CONTENT {
        // Server ignored the Range header; restart from scratch.
        std::fs::remove_file(partial).ok();
        downloaded = 0;
    }
    if !status.is_success() {
        bail!("download failed: HTTP {status}");
    }
    let total = total.unwrap_or_else(|| {
        response
            .content_length()
            .map(|len| len + downloaded)
            .unwrap_or(entry.size_bytes)
    });

    let mut file = tokio::task::block_in_place(|| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(partial)
    })
    .context("opening partial file")?;

    let mut stream = response.bytes_stream();
    let mut last_emit = std::time::Instant::now();
    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::SeqCst) {
            // Keep the partial for resume.
            sink.emit(events::CANCELLED, serde_json::json!(entry.id));
            return Ok(None);
        }
        let chunk = chunk.context("reading download stream")?;
        use std::io::Write;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        if last_emit.elapsed().as_millis() >= 100 {
            last_emit = std::time::Instant::now();
            sink.emit(
                events::PROGRESS,
                serde_json::to_value(DownloadProgress {
                    model_id: entry.id.clone(),
                    downloaded: before + downloaded,
                    total,
                    percentage: ((before + downloaded) as f64 / total.max(1) as f64) * 100.0,
                })
                .unwrap(),
            );
        }
    }
    Ok(Some(downloaded))
}

/// SHA-256 of a partial off the async threads; false (after deleting the
/// file and telling the UI) when it is not the expected one.
async fn verify_partial(
    partial: &Path,
    expected: &str,
    entry: &ModelEntry,
    sink: &dyn EventSink,
) -> Result<bool> {
    let sha = {
        let partial = partial.to_path_buf();
        tokio::task::spawn_blocking(move || sha256_file(&partial))
            .await
            .context("verification task")??
    };
    if sha == expected {
        return Ok(true);
    }
    std::fs::remove_file(partial).ok();
    sink.emit(
        events::FAILED,
        serde_json::json!({
            "model_id": entry.id,
            "error": format!("checksum mismatch (expected {expected}, got {sha}) — the download was corrupt and has been removed"),
        }),
    );
    Ok(false)
}

/// Download, verify, and install a model. Blocking-async; call from a spawned task.
pub async fn download_model(
    entry: &ModelEntry,
    data_dir: &Path,
    sink: &dyn EventSink,
    cancel: Arc<AtomicBool>,
) -> Result<()> {
    std::fs::create_dir_all(entry.dir(data_dir))?;
    let client = reqwest::Client::new();
    if entry.kind == DownloadKind::Files {
        return download_files(entry, data_dir, &client, sink, &cancel).await;
    }

    let partial = partial_path(entry, data_dir);
    let Some(downloaded) =
        fetch_into_partial(&client, &entry.url, &partial, entry, 0, None, sink, &cancel).await?
    else {
        return Ok(()); // cancelled
    };
    // Final progress tick so the UI lands on 100%.
    sink.emit(
        events::PROGRESS,
        serde_json::to_value(DownloadProgress {
            model_id: entry.id.clone(),
            downloaded,
            total: downloaded,
            percentage: 100.0,
        })
        .unwrap(),
    );

    // Verify checksum (SPEC FR-6.3); mismatch deletes the file.
    sink.emit(events::VERIFYING, serde_json::json!(entry.id));
    if !verify_partial(&partial, &entry.sha256, entry, sink).await? {
        bail!("checksum mismatch for {}", entry.id);
    }
    sink.emit(events::VERIFIED, serde_json::json!(entry.id));

    // Install: rename (file) or extract (archive).
    match entry.kind {
        DownloadKind::File => {
            let final_path = entry.engine_path(data_dir);
            std::fs::rename(&partial, &final_path)?;
        }
        DownloadKind::Archive => {
            let entry2 = entry.clone();
            let data_dir2 = data_dir.to_path_buf();
            let partial2 = partial.clone();
            tokio::task::spawn_blocking(move || extract_archive(&entry2, &data_dir2, &partial2))
                .await
                .context("extraction task")??;
            std::fs::remove_file(&partial).ok();
        }
        DownloadKind::Files => unreachable!("handled above"),
    }
    sink.emit(events::COMPLETE, serde_json::json!(entry.id));
    Ok(())
}

/// The Files kind: fetch every file to its own partial (each resumable),
/// verify all of them, and only then move them into place — so the model
/// reads as downloaded all at once or not at all.
async fn download_files(
    entry: &ModelEntry,
    data_dir: &Path,
    client: &reqwest::Client,
    sink: &dyn EventSink,
    cancel: &AtomicBool,
) -> Result<()> {
    let mut before = 0u64;
    for file in &entry.files {
        let partial = file_partial_path(entry, data_dir, file);
        let have = std::fs::metadata(&partial).map(|m| m.len()).unwrap_or(0);
        if have > file.size_bytes {
            // Not a prefix of the file we want: start it over.
            std::fs::remove_file(&partial).ok();
        }
        // An already-complete partial needs no request (a Range starting at
        // the end of the file is an error, not an empty success).
        if have != file.size_bytes {
            let fetched = fetch_into_partial(
                client,
                &entry.file_url(file),
                &partial,
                entry,
                before,
                Some(entry.size_bytes),
                sink,
                cancel,
            )
            .await?;
            if fetched.is_none() {
                return Ok(()); // cancelled
            }
        }
        before += file.size_bytes;
    }
    // Final progress tick so the UI lands on 100%.
    sink.emit(
        events::PROGRESS,
        serde_json::to_value(DownloadProgress {
            model_id: entry.id.clone(),
            downloaded: entry.size_bytes,
            total: entry.size_bytes,
            percentage: 100.0,
        })
        .unwrap(),
    );

    // Verify every file (SPEC FR-6.3); a mismatch deletes that file only, so
    // the retry re-fetches just the bad one.
    sink.emit(events::VERIFYING, serde_json::json!(entry.id));
    for file in &entry.files {
        let partial = file_partial_path(entry, data_dir, file);
        if !verify_partial(&partial, &file.sha256, entry, sink).await? {
            bail!("checksum mismatch for {} ({})", entry.id, file.name);
        }
    }
    sink.emit(events::VERIFIED, serde_json::json!(entry.id));

    let dir = entry.dir(data_dir);
    for file in &entry.files {
        std::fs::rename(file_partial_path(entry, data_dir, file), dir.join(&file.name))?;
    }
    sink.emit(events::COMPLETE, serde_json::json!(entry.id));
    Ok(())
}

/// Extract a tar.gz into the model dir. If the archive holds a single top-level
/// directory, that directory's contents become the model root (Handy-compatible
/// layout for the Parakeet archives).
fn extract_archive(entry: &ModelEntry, data_dir: &Path, archive_path: &Path) -> Result<()> {
    let model_dir = entry.dir(data_dir);
    let temp = model_dir.join(".extracting");
    if temp.exists() {
        std::fs::remove_dir_all(&temp)?;
    }
    std::fs::create_dir_all(&temp)?;

    let tar_gz = std::fs::File::open(archive_path).context("opening archive")?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tar_gz));
    archive.unpack(&temp).map_err(|e| {
        let _ = std::fs::remove_dir_all(&temp);
        let _ = std::fs::remove_file(archive_path); // corrupt archive: restart next time
        anyhow!("extracting archive: {e}")
    })?;

    // Locate the extracted model root.
    let top_entries: Vec<_> = std::fs::read_dir(&temp)?
        .filter_map(|e| e.ok())
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .collect();
    let source = if top_entries.len() == 1 && top_entries[0].file_type()?.is_dir() {
        top_entries[0].path()
    } else {
        temp.clone()
    };

    // Move contents into the model dir root (engine_path with filename "").
    for item in std::fs::read_dir(&source)? {
        let item = item?;
        let dest = model_dir.join(item.file_name());
        if dest.exists() {
            if dest.is_dir() {
                std::fs::remove_dir_all(&dest)?;
            } else {
                std::fs::remove_file(&dest)?;
            }
        }
        std::fs::rename(item.path(), dest)?;
    }
    std::fs::remove_dir_all(&temp).ok();
    Ok(())
}

pub fn sha256_file(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).context("opening file for checksum")?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn delete_model(entry: &ModelEntry, data_dir: &Path) -> Result<()> {
    let dir = entry.dir(data_dir);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_of_known_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        std::fs::write(&path, b"hello world").unwrap();
        assert_eq!(
            sha256_file(&path).unwrap(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    // ---- Files-kind downloads (R34–R35) against a throwaway localhost server.

    use crate::registry::{files_digest, EngineFamily};
    use std::io::{Read, Write};

    /// Serve `files` (path → bytes) over HTTP on 127.0.0.1, honouring Range.
    /// Returns the base URL and a log of "<path> <range-or-dash>" requests.
    fn serve(files: Vec<(&'static str, Vec<u8>)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let log = Arc::new(Mutex::new(Vec::new()));
        let log2 = log.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = request
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/")
                    .trim_start_matches('/')
                    .to_string();
                let from = request
                    .lines()
                    .find_map(|l| l.to_lowercase().strip_prefix("range: bytes=").map(str::to_string))
                    .and_then(|r| r.trim_end_matches('-').trim().parse::<usize>().ok());
                log2.lock().unwrap().push(format!(
                    "{path} {}",
                    from.map_or("-".to_string(), |f| f.to_string())
                ));
                let Some((_, body)) = files.iter().find(|(name, _)| *name == path) else {
                    let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    continue;
                };
                let (status, body) = match from {
                    Some(f) if f < body.len() => ("206 Partial Content", &body[f..]),
                    Some(_) => ("416 Range Not Satisfiable", &body[..0]),
                    None => ("200 OK", &body[..]),
                };
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .as_bytes(),
                );
                let _ = stream.write_all(body);
            }
        });
        (base, log)
    }

    fn sha(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    /// A Files-kind entry for `files` as the server holds them.
    fn files_entry(base: &str, files: &[(&'static str, Vec<u8>)]) -> ModelEntry {
        let list: Vec<ModelFile> = files
            .iter()
            .map(|(name, body)| ModelFile {
                name: name.to_string(),
                sha256: sha(body),
                size_bytes: body.len() as u64,
            })
            .collect();
        ModelEntry {
            id: "multi".into(),
            display_name: "Multi".into(),
            engine: EngineFamily::Parakeet,
            url: base.to_string(),
            sha256: files_digest(&list),
            size_bytes: list.iter().map(|f| f.size_bytes).sum(),
            languages: "en".into(),
            description: String::new(),
            recommended: false,
            kind: DownloadKind::Files,
            filename: String::new(),
            files: list,
        }
    }

    #[derive(Default)]
    struct RecordingSink(Mutex<Vec<(String, serde_json::Value)>>);
    impl EventSink for RecordingSink {
        fn emit(&self, event: &str, payload: serde_json::Value) {
            self.0.lock().unwrap().push((event.to_string(), payload));
        }
    }
    impl RecordingSink {
        fn names(&self) -> Vec<String> {
            let mut names: Vec<String> =
                self.0.lock().unwrap().iter().map(|(e, _)| e.clone()).collect();
            names.dedup();
            names
        }
    }

    fn sample_files() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("encoder.onnx", vec![7u8; 300_000]),
            ("decoder.onnx", vec![9u8; 40_000]),
            ("vocab.txt", b"a\nb\nc\n".to_vec()),
        ]
    }

    // R34: a files model downloads every file, verifies each, and only then
    // reads as downloaded — with the same event sequence the UI already
    // drives for single-file models.
    #[tokio::test(flavor = "multi_thread")]
    async fn r34_files_model_downloads_verifies_and_installs_all_files() {
        let files = sample_files();
        let (base, _log) = serve(files.clone());
        let entry = files_entry(&base, &files);
        let dir = tempfile::tempdir().unwrap();
        let sink = RecordingSink::default();
        assert!(!entry.is_downloaded(dir.path()));

        download_model(&entry, dir.path(), &sink, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();

        assert!(entry.is_downloaded(dir.path()));
        for (name, body) in &files {
            assert_eq!(&std::fs::read(entry.dir(dir.path()).join(name)).unwrap(), body);
        }
        assert_eq!(partial_size(&entry, dir.path()), 0, "no partials left behind");
        assert_eq!(
            sink.names(),
            [events::PROGRESS, events::VERIFYING, events::VERIFIED, events::COMPLETE]
        );
        // Progress is over the whole model, not per file, and ends on 100%.
        let events = sink.0.lock().unwrap();
        let last = events.iter().rev().find(|(e, _)| e == events::PROGRESS).unwrap();
        assert_eq!(last.1["total"], entry.size_bytes);
        assert_eq!(last.1["downloaded"], entry.size_bytes);
        assert_eq!(last.1["percentage"], 100.0);
        assert!(events
            .iter()
            .filter(|(e, _)| e == events::PROGRESS)
            .all(|(_, p)| p["total"] == entry.size_bytes));
    }

    // R35: failure and resume. A corrupt file fails the whole model, removes
    // only that file, and the retry fetches only that file again; a
    // half-fetched file resumes with a Range request; a model missing one
    // file is not "downloaded".
    #[tokio::test(flavor = "multi_thread")]
    async fn r35_files_model_rejects_a_corrupt_file_and_retries_only_it() {
        let files = sample_files();
        let mut served = files.clone();
        served[1].1 = vec![1u8; 40_000]; // same size, wrong content
        let (bad_base, _) = serve(served);
        let dir = tempfile::tempdir().unwrap();
        let sink = RecordingSink::default();
        let bad_entry = files_entry(&bad_base, &files); // expects the GOOD hashes

        let err = download_model(&bad_entry, dir.path(), &sink, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("decoder.onnx"), "{err}");
        assert!(!bad_entry.is_downloaded(dir.path()), "nothing installed");
        assert!(sink.names().contains(&events::FAILED.to_string()));
        assert!(!sink.names().contains(&events::COMPLETE.to_string()));
        let model_dir = bad_entry.dir(dir.path());
        assert!(model_dir.join("encoder.onnx.partial").exists(), "good file kept");
        assert!(!model_dir.join("decoder.onnx.partial").exists(), "bad file removed");

        // Retry against a good server: only the removed file is requested —
        // the files that verified the first time are not fetched again.
        let (good_base, log) = serve(files.clone());
        let entry = files_entry(&good_base, &files);
        download_model(&entry, dir.path(), &NullSink, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        assert!(entry.is_downloaded(dir.path()));
        assert_eq!(*log.lock().unwrap(), ["decoder.onnx -"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn r35_files_model_resumes_a_half_fetched_file() {
        let files = sample_files();
        let (base, log) = serve(files.clone());
        let entry = files_entry(&base, &files);
        let dir = tempfile::tempdir().unwrap();
        let model_dir = entry.dir(dir.path());
        std::fs::create_dir_all(&model_dir).unwrap();
        // The first 100 kB of the encoder arrived before an interruption.
        std::fs::write(model_dir.join("encoder.onnx.partial"), &files[0].1[..100_000]).unwrap();
        assert_eq!(partial_size(&entry, dir.path()), 100_000, "resume size shown to the UI");

        download_model(&entry, dir.path(), &NullSink, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        assert!(entry.is_downloaded(dir.path()));
        assert_eq!(std::fs::read(model_dir.join("encoder.onnx")).unwrap(), files[0].1);
        assert_eq!(log.lock().unwrap()[0], "encoder.onnx 100000", "resumed with a Range");
    }

    #[test]
    fn r35_files_model_missing_a_file_is_not_downloaded() {
        let files = sample_files();
        let entry = files_entry("https://example.com/", &files);
        let dir = tempfile::tempdir().unwrap();
        let model_dir = entry.dir(dir.path());
        std::fs::create_dir_all(&model_dir).unwrap();
        assert!(!entry.is_downloaded(dir.path()), "an empty folder is not a model");
        for (name, body) in &files[..2] {
            std::fs::write(model_dir.join(name), body).unwrap();
        }
        assert!(!entry.is_downloaded(dir.path()), "two of three files");
        std::fs::write(model_dir.join(files[2].0), &files[2].1).unwrap();
        assert!(entry.is_downloaded(dir.path()));
        assert_eq!(entry.engine_path(dir.path()), model_dir);
    }

    #[test]
    fn cancel_flags_lifecycle() {
        let dm = DownloadManager::default();
        assert!(!dm.is_downloading("m"));
        assert!(!dm.cancel("m"));
        let flag = dm.begin("m").unwrap();
        assert!(dm.is_downloading("m"));
        assert!(dm.begin("m").is_err(), "double download rejected");
        assert!(dm.cancel("m"));
        assert!(flag.load(Ordering::SeqCst));
        dm.finish("m");
        assert!(!dm.is_downloading("m"));
    }
}
