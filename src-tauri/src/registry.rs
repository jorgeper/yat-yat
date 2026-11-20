//! Data-driven model registry (SPEC §3). One JSON entry fully defines a model;
//! the UI renders whatever this registry contains. Adding a model = adding an
//! entry to `models.json`.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Embedded catalog. Kept as JSON (not Rust consts) so an entry is the only
/// thing needed to add a model.
pub const EMBEDDED_CATALOG: &str = include_str!("../models.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineFamily {
    /// GGUF/ggml models run through whisper.cpp (transcribe-cpp).
    Whisper,
    /// ONNX models (Parakeet) run through transcribe-rs.
    Parakeet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadKind {
    /// Single model file stored as `<data_dir>/models/<id>/<filename>`.
    File,
    /// tar.gz archive extracted into `<data_dir>/models/<id>/`.
    Archive,
    /// Several files fetched one by one from `url` (a base ending in `/`)
    /// into `<data_dir>/models/<id>/` — how model repos publish a
    /// directory-shaped model without anyone repackaging it.
    Files,
}

/// Which weights a Parakeet model's files hold. The engine loads different
/// file names for each, so the catalog has to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Precision {
    /// 8-bit quantized weights: a quarter of the size, faster on a CPU.
    #[default]
    Int8,
    /// The weights as trained (32-bit floats).
    Fp32,
}

impl Precision {
    fn is_default(&self) -> bool {
        *self == Precision::Int8
    }
}

/// One file of a [`DownloadKind::Files`] model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelFile {
    /// File name, appended to the entry's base `url` and used on disk.
    pub name: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub display_name: String,
    pub engine: EngineFamily,
    pub url: String,
    /// SHA-256 of the downloaded artifact (the archive itself for Archive
    /// kind). For Files kind: the digest of the file list
    /// ([`files_digest`]), so the entry still pins its exact contents.
    pub sha256: String,
    pub size_bytes: u64,
    pub languages: String,
    pub description: String,
    #[serde(default)]
    pub recommended: bool,
    pub kind: DownloadKind,
    /// Filename to store (File kind) or the directory member holding the model
    /// root after extraction (Archive kind, "" = archive root).
    pub filename: String,
    /// Parakeet only: the weights these files hold (default int8).
    #[serde(default, skip_serializing_if = "Precision::is_default")]
    pub precision: Precision,
    /// The files of a Files-kind model; empty for the other kinds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<ModelFile>,
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// The digest that stands for a whole file list: SHA-256 over one
/// `<sha256>  <name>` line per file, in catalog order (the `shasum` listing
/// format, so it can be reproduced by hand).
pub fn files_digest(files: &[ModelFile]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for f in files {
        hasher.update(format!("{}  {}\n", f.sha256, f.name));
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub models: Vec<ModelEntry>,
}

impl Registry {
    pub fn embedded() -> Result<Self> {
        Self::from_json(EMBEDDED_CATALOG)
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let reg: Registry = serde_json::from_str(json).context("parsing model registry")?;
        reg.validate()?;
        Ok(reg)
    }

    fn validate(&self) -> Result<()> {
        if self.models.is_empty() {
            bail!("model registry is empty");
        }
        let mut seen = std::collections::HashSet::new();
        for m in &self.models {
            for (field, value) in [
                ("id", &m.id),
                ("display_name", &m.display_name),
                ("url", &m.url),
                ("sha256", &m.sha256),
                ("languages", &m.languages),
            ] {
                if value.trim().is_empty() {
                    bail!("model entry '{}' has empty {field}", m.id);
                }
            }
            if !is_sha256(&m.sha256) {
                bail!("model '{}' sha256 is not a hex sha-256", m.id);
            }
            if m.kind == DownloadKind::Files {
                if m.files.is_empty() {
                    bail!("model '{}' is a files model with no files", m.id);
                }
                if !m.url.ends_with('/') {
                    bail!("model '{}' files base url must end with '/'", m.id);
                }
                let mut names = std::collections::HashSet::new();
                for f in &m.files {
                    // The name becomes a path on disk and a URL segment:
                    // a plain file name only, never a way out of the folder.
                    let plain = !f.name.is_empty()
                        && !f.name.starts_with('.')
                        && f.name
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
                    if !plain {
                        bail!("model '{}' has an unsafe file name '{}'", m.id, f.name);
                    }
                    if f.name.ends_with(".partial") {
                        bail!("model '{}' file '{}' collides with partial downloads", m.id, f.name);
                    }
                    if !is_sha256(&f.sha256) {
                        bail!("model '{}' file '{}' sha256 is not a hex sha-256", m.id, f.name);
                    }
                    if f.size_bytes == 0 {
                        bail!("model '{}' file '{}' has zero size", m.id, f.name);
                    }
                    if !names.insert(&f.name) {
                        bail!("model '{}' lists file '{}' twice", m.id, f.name);
                    }
                }
                if m.size_bytes != m.files.iter().map(|f| f.size_bytes).sum::<u64>() {
                    bail!("model '{}' size is not the sum of its files", m.id);
                }
                if m.sha256 != files_digest(&m.files) {
                    bail!("model '{}' sha256 does not match its file list", m.id);
                }
            } else if !m.files.is_empty() {
                bail!("model '{}' lists files but is not a files model", m.id);
            }
            if m.size_bytes == 0 {
                bail!("model '{}' has zero size", m.id);
            }
            if !m.url.starts_with("https://") {
                bail!("model '{}' url must be https", m.id);
            }
            if !seen.insert(m.id.clone()) {
                bail!("duplicate model id '{}'", m.id);
            }
        }
        let recommended = self.models.iter().filter(|m| m.recommended).count();
        if recommended != 1 {
            bail!("registry must flag exactly one recommended model, found {recommended}");
        }
        Ok(())
    }

    /// The listing API the UI consumes (settings Models section).
    pub fn list(&self) -> &[ModelEntry] {
        &self.models
    }

    pub fn get(&self, id: &str) -> Option<&ModelEntry> {
        self.models.iter().find(|m| m.id == id)
    }

    pub fn recommended(&self) -> &ModelEntry {
        // validate() guarantees exactly one.
        self.models.iter().find(|m| m.recommended).unwrap()
    }
}

impl ModelEntry {
    /// Directory holding this model's artifacts.
    pub fn dir(&self, data_dir: &Path) -> PathBuf {
        data_dir.join("models").join(&self.id)
    }

    /// Path handed to the engine: the model file (File) or model root dir (Archive).
    pub fn engine_path(&self, data_dir: &Path) -> PathBuf {
        match self.kind {
            DownloadKind::File => self.dir(data_dir).join(&self.filename),
            DownloadKind::Archive => {
                if self.filename.is_empty() {
                    self.dir(data_dir)
                } else {
                    self.dir(data_dir).join(&self.filename)
                }
            }
            DownloadKind::Files => self.dir(data_dir),
        }
    }

    pub fn is_downloaded(&self, data_dir: &Path) -> bool {
        match self.kind {
            // The folder exists from the first downloaded byte: a files
            // model is only there when every one of its files is.
            DownloadKind::Files => {
                let dir = self.dir(data_dir);
                self.files.iter().all(|f| dir.join(&f.name).is_file())
            }
            _ => self.engine_path(data_dir).exists(),
        }
    }

    /// Where a file of a Files-kind model is fetched from.
    pub fn file_url(&self, file: &ModelFile) -> String {
        format!("{}{}", self.url, file.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // R7: registry parses, fields present, exactly one recommended, paths resolve,
    // synthetic entries surface through the listing API.
    #[test]
    fn r7_embedded_registry_parses_and_validates() {
        let reg = Registry::embedded().expect("embedded models.json must be valid");
        assert!(reg.list().len() >= 4, "catalog should have at least 4 models");
        for m in reg.list() {
            assert!(!m.id.is_empty());
            assert!(!m.url.is_empty());
            assert_eq!(m.sha256.len(), 64, "{} sha256", m.id);
            assert!(m.size_bytes > 0, "{} size", m.id);
        }
    }

    #[test]
    fn r7_exactly_one_recommended_and_it_is_parakeet() {
        let reg = Registry::embedded().unwrap();
        let rec: Vec<_> = reg.list().iter().filter(|m| m.recommended).collect();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].engine, EngineFamily::Parakeet);
        assert_eq!(reg.recommended().id, rec[0].id);
    }

    #[test]
    fn r7_path_resolution() {
        let reg = Registry::embedded().unwrap();
        let data = Path::new("/tmp/yat-yat-data");
        for m in reg.list() {
            let p = m.engine_path(data);
            assert!(
                p.starts_with(data.join("models").join(&m.id)),
                "{} resolves inside its model dir: {p:?}",
                m.id
            );
        }
    }

    #[test]
    fn r7_synthetic_entry_surfaces_through_listing_api() {
        // Proves extensibility: a new registry entry is all a new model needs.
        let mut json: serde_json::Value = serde_json::from_str(EMBEDDED_CATALOG).unwrap();
        json["models"].as_array_mut().unwrap().push(serde_json::json!({
            "id": "test-model-9000",
            "display_name": "Test Model 9000",
            "engine": "whisper",
            "url": "https://example.com/model.bin",
            "sha256": "a".repeat(64),
            "size_bytes": 1234,
            "languages": "Testish",
            "description": "synthetic",
            "recommended": false,
            "kind": "file",
            "filename": "model.bin"
        }));
        let reg = Registry::from_json(&json.to_string()).unwrap();
        let found = reg.get("test-model-9000").expect("listed");
        assert_eq!(found.display_name, "Test Model 9000");
        assert!(reg.list().iter().any(|m| m.id == "test-model-9000"));
    }

    // R36: the files kind in the catalog — Parakeet comes straight from the
    // conversion's own repository, pinned so the URL can never serve
    // something else, and a malformed files entry is rejected.
    #[test]
    fn r36_parakeet_is_fetched_from_a_pinned_repository_revision() {
        let reg = Registry::embedded().unwrap();
        let m = reg.recommended();
        assert_eq!(m.kind, DownloadKind::Files);
        let rest = m
            .url
            .strip_prefix("https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/")
            .expect("served from the conversion's Hugging Face repository");
        let revision = rest.strip_suffix('/').expect("a base url ends with /");
        assert!(
            revision.len() == 40 && revision.chars().all(|c| c.is_ascii_hexdigit()),
            "pinned to a commit, not a branch: {revision}"
        );
        // Exactly the files the engine loads (transcribe-rs Parakeet, int8).
        let mut names: Vec<&str> = m.files.iter().map(|f| f.name.as_str()).collect();
        names.sort();
        assert_eq!(
            names,
            [
                "config.json",
                "decoder_joint-model.int8.onnx",
                "encoder-model.int8.onnx",
                "nemo128.onnx",
                "vocab.txt"
            ]
        );
        assert_eq!(m.sha256, files_digest(&m.files));
        assert_eq!(m.size_bytes, m.files.iter().map(|f| f.size_bytes).sum::<u64>());
        assert_eq!(
            m.file_url(&m.files[0]),
            format!("{}{}", m.url, m.files[0].name)
        );
        // Installs made from the old archive hold the same file names in the
        // same folder, so they keep reading as downloaded.
        assert_eq!(m.engine_path(Path::new("/d")), Path::new("/d/models").join(&m.id));
    }

    // R44: the full-precision Parakeet — same pinned repository, the
    // unquantized files, never the default download.
    #[test]
    fn r44_full_precision_parakeet_is_in_the_catalog() {
        let reg = Registry::embedded().unwrap();
        let standard = reg.recommended();
        assert_eq!(standard.precision, Precision::Int8, "absent field = int8");
        let full = reg.get("parakeet-tdt-0.6b-v3-full").expect("in the catalog");
        assert_eq!(full.engine, EngineFamily::Parakeet);
        assert_eq!(full.precision, Precision::Fp32);
        assert!(!full.recommended, "a 2.5 GB download is opt-in");
        assert_eq!(full.url, standard.url, "same repository, same pinned commit");
        let names: Vec<&str> = full.files.iter().map(|f| f.name.as_str()).collect();
        // What transcribe-rs loads for FP32: the graph, its external weights
        // file, the decoder, and the shared preprocessor / vocabulary.
        for needed in [
            "encoder-model.onnx",
            "encoder-model.onnx.data",
            "decoder_joint-model.onnx",
            "nemo128.onnx",
            "vocab.txt",
            "config.json",
        ] {
            assert!(names.contains(&needed), "{needed} missing");
        }
        assert!(!names.iter().any(|n| n.contains("int8")));
        // The field survives the trip to the UI and stays out of the JSON
        // of models that do not need it.
        let json = serde_json::to_value(full).unwrap();
        assert_eq!(json["precision"], "fp32");
        assert!(serde_json::to_value(standard).unwrap().get("precision").is_none());
    }

    #[test]
    fn r36_malformed_files_entries_are_rejected() {
        let base: serde_json::Value = serde_json::from_str(EMBEDDED_CATALOG).unwrap();
        let broken = |edit: &dyn Fn(&mut serde_json::Value)| {
            let mut json = base.clone();
            edit(&mut json["models"][0]);
            Registry::from_json(&json.to_string())
        };
        assert!(broken(&|_| {}).is_ok(), "the unedited catalog is the control");
        for name in ["../escape", "sub/dir.onnx", ".hidden", "", "x.partial", "sp ace"] {
            let err = broken(&|m| m["files"][0]["name"] = serde_json::json!(name));
            assert!(err.is_err(), "file name {name:?} must be rejected");
        }
        assert!(broken(&|m| m["files"] = serde_json::json!([])).is_err(), "no files");
        assert!(broken(&|m| m["files"][0]["sha256"] = serde_json::json!("abc")).is_err());
        assert!(broken(&|m| m["files"][0]["size_bytes"] = serde_json::json!(0)).is_err());
        assert!(
            broken(&|m| m["files"][1]["name"] = m["files"][0]["name"].clone()).is_err(),
            "duplicate name"
        );
        assert!(
            broken(&|m| m["url"] = serde_json::json!("https://example.com/no-slash")).is_err(),
            "base url without a trailing slash"
        );
        // Editing a file hash without the entry digest (or the reverse) is
        // caught: the entry pins its exact contents.
        assert!(broken(&|m| m["files"][0]["sha256"] = serde_json::json!("0".repeat(64))).is_err());
        assert!(broken(&|m| m["size_bytes"] = serde_json::json!(1)).is_err());
        // A file list on a single-file model is a catalog mistake.
        let mut json = base.clone();
        json["models"][1]["files"] = base["models"][0]["files"].clone();
        assert!(Registry::from_json(&json.to_string()).is_err());
    }

    #[test]
    fn r7_invalid_registries_rejected() {
        // Empty sha
        let bad = r#"{"models":[{"id":"x","display_name":"X","engine":"whisper",
            "url":"https://e.com/f","sha256":"","size_bytes":1,"languages":"en",
            "description":"d","recommended":true,"kind":"file","filename":"f"}]}"#;
        assert!(Registry::from_json(bad).is_err());
        // Two recommended
        let mut json: serde_json::Value = serde_json::from_str(EMBEDDED_CATALOG).unwrap();
        for m in json["models"].as_array_mut().unwrap().iter_mut() {
            m["recommended"] = serde_json::json!(true);
        }
        assert!(Registry::from_json(&json.to_string()).is_err());
        // Non-https URL
        let mut json: serde_json::Value = serde_json::from_str(EMBEDDED_CATALOG).unwrap();
        json["models"][0]["url"] = serde_json::json!("http://insecure.com/m.bin");
        assert!(Registry::from_json(&json.to_string()).is_err());
    }
}
