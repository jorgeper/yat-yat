//! Shared app state managed by Tauri.

use crate::audio::AudioRecorder;
use crate::downloader::DownloadManager;
use crate::history::HistoryStore;
use crate::hotkey::HotkeyService;
use crate::paste::Paster;
use crate::pipeline::Pipeline;
use crate::registry::Registry;
use crate::settings::Settings;
use crate::stt::LoadedModel;
use std::path::PathBuf;
use std::sync::{Mutex, RwLock};
use tauri::{AppHandle, Emitter};

pub struct AppState {
    pub data_dir: PathBuf,
    pub settings: RwLock<Settings>,
    pub registry: Registry,
    pub history: Mutex<HistoryStore>,
    pub last_transcription: Mutex<Option<String>>,
    /// Frontmost app when the current dictation started (SPEC7 FR-G1).
    pub dictation_start_app: Mutex<Option<crate::focus::FrontmostApp>>,
    /// Transcript held while the focus-guard prompt is up (SPEC7 FR-G5); the
    /// pipeline never blocks on the human — resolution comes back as an event.
    pub pending_paste: Mutex<Option<String>>,
    pub recorder: AudioRecorder,
    pub pipeline: Pipeline,
    pub paster: Mutex<Paster>,
    pub downloads: DownloadManager,
    /// None until capture is initialized (macOS: after Accessibility granted).
    pub hotkeys: Mutex<Option<HotkeyService>>,
    /// The warm STT engine for the active model (SPEC §3: loaded once).
    engine: Mutex<Option<LoadedModel>>,
    /// Per-model real-time-factor EMAs (SPEC13 FR-P2, disk-backed — see the
    /// ARCHITECTURE.md divergence note). Loaded from rtf.json at startup;
    /// persistence is debounced (SPEC14 FR-D5): observations mark the store
    /// dirty, `flush_rtf` writes at return-to-idle / app exit.
    rtf: Mutex<crate::progress::RtfStore>,
    /// SPEC14 FR-A2: set the moment `finish_recording` begins so an
    /// about-to-start live pass yields the engine to the stop path.
    pub stop_pending: std::sync::atomic::AtomicBool,
    /// Which dictation is recording (pipeline.rs's generation), so a
    /// watcher left over from an earlier one can tell and exit.
    pub recording_generation: std::sync::atomic::AtomicU64,
    /// SPEC14 FR-D3: the in-flight background write of the retained WAV.
    /// Retry must join it before reading — never a partial file.
    wav_write: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// The in-flight chunked-transcription session, if this recording has
    /// one (chunking.rs). The stop path takes and finishes it; Cancel and a
    /// new recording abandon it.
    pub chunks: Mutex<Option<ChunkSession>>,
}

/// Chunks already transcribed while the user was still talking.
#[derive(Default)]
pub struct ChunkProgress {
    /// Samples (16 kHz) already transcribed — the stop path transcribes only
    /// what comes after.
    pub committed: usize,
    /// The transcript so far, stitched across chunks, in absolute recording
    /// time. Runs a little past `committed`: each pass sees context after
    /// its cut, which the next pass is stitched onto.
    pub words: Vec<crate::stt::Word>,
    /// How many chunks were transcribed (for the stop-path log line).
    pub chunks: usize,
    /// The latest reading of the audio after `committed`, if any — reused
    /// by the stop path when nothing was said after it (chunking.rs).
    pub reading: Option<crate::chunking::TailReading>,
}

/// One recording's chunker thread plus what it has produced so far.
pub struct ChunkSession {
    pub progress: std::sync::Arc<Mutex<ChunkProgress>>,
    /// Set to make the chunker exit before starting another pass. A pass
    /// already running finishes first — its audio has to be transcribed
    /// either way.
    pub stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Wakes the chunker out of its poll wait, so stopping never waits for
    /// the poll interval to run out.
    pub wake: std::sync::mpsc::Sender<()>,
    pub thread: std::thread::JoinHandle<()>,
}

impl ChunkSession {
    /// Stop the chunker, wait for any pass in flight, and return what it
    /// transcribed.
    pub fn finish(self) -> ChunkProgress {
        self.signal_stop();
        let _ = self.thread.join();
        std::mem::take(&mut *self.progress.lock().unwrap())
    }

    /// Stop the chunker without waiting; its output is discarded.
    pub fn abandon(self) {
        self.signal_stop();
    }

    /// Tell the chunker to exit before its next pass, and wake it now.
    pub fn signal_stop(&self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = self.wake.send(());
    }
}

impl AppState {
    pub fn new(
        data_dir: PathBuf,
        settings: Settings,
        registry: Registry,
        history: HistoryStore,
        recorder: AudioRecorder,
        pipeline: Pipeline,
    ) -> Self {
        let rtf = crate::progress::load_rtf_map(&data_dir.join("rtf.json"));
        Self {
            data_dir,
            settings: RwLock::new(settings),
            registry,
            history: Mutex::new(history),
            last_transcription: Mutex::new(None),
            dictation_start_app: Mutex::new(None),
            pending_paste: Mutex::new(None),
            recorder,
            pipeline,
            paster: Mutex::new(Paster::new()),
            downloads: DownloadManager::default(),
            hotkeys: Mutex::new(None),
            engine: Mutex::new(None),
            rtf: Mutex::new(crate::progress::RtfStore::new(rtf)),
            stop_pending: std::sync::atomic::AtomicBool::new(false),
            recording_generation: std::sync::atomic::AtomicU64::new(0),
            wav_write: Mutex::new(None),
            chunks: Mutex::new(None),
        }
    }

    /// Hand off the retained-WAV write to a background thread (SPEC14 FR-D3).
    /// Joins any straggler from the previous dictation first so two writes
    /// can never interleave on the same file.
    pub fn spawn_wav_write(&self, handle: std::thread::JoinHandle<()>) {
        let mut slot = self.wav_write.lock().unwrap();
        if let Some(prev) = slot.take() {
            let _ = prev.join();
        }
        *slot = Some(handle);
    }

    /// Block until any in-flight retained-WAV write completes (the Retry
    /// path's partial-file guard, SPEC14 FR-D3).
    pub fn await_wav_write(&self) {
        if let Some(handle) = self.wav_write.lock().unwrap().take() {
            let _ = handle.join();
        }
    }

    /// Estimated raw-STT seconds for `audio_secs` on the active model
    /// (SPEC13 FR-P3).
    pub fn expected_stt_secs(&self, audio_secs: f32) -> f32 {
        let model = self.settings.read().unwrap().active_model.clone().unwrap_or_default();
        audio_secs * self.rtf.lock().unwrap().estimate(&model)
    }

    /// Fold one measured raw-STT wall time into a model's EMA (SPEC13
    /// FR-P5 — raw engine time only). Marks the store dirty; the disk write
    /// happens at `flush_rtf` (SPEC14 FR-D5), never here — live passes used
    /// to write rtf.json every 1–4 s.
    fn observe_rtf(&self, model: &str, audio_secs: f32, wall_secs: f32) {
        self.rtf.lock().unwrap().observe(model, audio_secs, wall_secs);
    }

    /// Persist the RTF map if any observation landed since the last flush
    /// (SPEC14 FR-D5: called at return-to-idle and app exit; R21's
    /// corrupt/missing-read semantics are load-side and unchanged).
    pub fn flush_rtf(&self) {
        let path = self.data_dir.join("rtf.json");
        self.rtf
            .lock()
            .unwrap()
            .flush(|map| crate::progress::save_rtf_map(&path, map));
    }

    pub fn settings_path(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }

    pub fn history_path(&self) -> PathBuf {
        self.data_dir.join("history.json")
    }

    /// Is there an active model that is actually on disk?
    pub fn active_model_ready(&self) -> bool {
        let settings = self.settings.read().unwrap();
        settings
            .active_model
            .as_deref()
            .and_then(|id| self.registry.get(id))
            .map(|entry| entry.is_downloaded(&self.data_dir))
            .unwrap_or(false)
    }

    /// Engine family of the active model, if one is set and known.
    pub fn active_engine_family(&self) -> Option<crate::registry::EngineFamily> {
        let settings = self.settings.read().unwrap();
        settings
            .active_model
            .as_deref()
            .and_then(|id| self.registry.get(id))
            .map(|entry| entry.engine)
    }

    /// Load the active model into `engine` if it isn't already the loaded one.
    /// Runs under the engine mutex the caller holds; emits the same
    /// model-state-changed events the lazy path always has. Returns the
    /// active model id (SPEC14 FR-W1).
    fn load_if_needed(
        &self,
        app: &AppHandle,
        engine: &mut Option<LoadedModel>,
    ) -> anyhow::Result<String> {
        let active_id = self
            .settings
            .read()
            .unwrap()
            .active_model
            .clone()
            .ok_or_else(|| anyhow::anyhow!("no active model"))?;
        let entry = self
            .registry
            .get(&active_id)
            .ok_or_else(|| anyhow::anyhow!("unknown model '{active_id}'"))?;

        let needs_load = engine.as_ref().map(|m| m.model_id != active_id).unwrap_or(true);
        if needs_load {
            // Drop the old engine first: avoids 2x peak RAM on big models.
            *engine = None;
            let _ = app.emit("model-state-changed", "loading_started");
            let t0 = std::time::Instant::now();
            match LoadedModel::load(entry, &self.data_dir) {
                Ok(loaded) => {
                    log::info!("loaded model {active_id} in {:?}", t0.elapsed());
                    let _ = app.emit("model-state-changed", "loading_completed");
                    *engine = Some(loaded);
                }
                Err(e) => {
                    let _ = app.emit("model-state-changed", "loading_failed");
                    return Err(e);
                }
            }
        }
        Ok(active_id)
    }

    /// Pre-warm the engine (SPEC14 FR-W2). Call from a background thread
    /// ONLY — this blocks on the engine mutex and then on the multi-second
    /// model load; a stop arriving mid-load simply queues on the same mutex,
    /// exactly as the lazy path always behaved (just starting earlier).
    pub fn ensure_loaded(&self, app: &AppHandle) -> anyhow::Result<()> {
        let mut engine = self.engine.lock().unwrap();
        self.load_if_needed(app, &mut engine).map(|_| ())
    }

    /// Cheap pre-warm probe: true when nothing is loaded AND no load is in
    /// flight (a held mutex means a loader is already working — don't stack
    /// another). Never blocks (SPEC14 FR-W2c).
    pub fn engine_needs_warm(&self) -> bool {
        match self.engine.try_lock() {
            Ok(guard) => guard.is_none(),
            Err(_) => false,
        }
    }

    /// Transcribe through the warm engine, (re)loading it if the active model
    /// changed or nothing is loaded yet. Emits model-state-changed events.
    pub fn transcribe(&self, app: &AppHandle, samples: &[f32]) -> anyhow::Result<String> {
        let mut engine = self.engine.lock().unwrap();
        let active_id = self.load_if_needed(app, &mut engine)?;
        // Time ONLY the engine call (model load above is excluded) and feed
        // the per-model RTF estimate. Live passes run through here too, so
        // with live transcription on, the progress estimate is calibrated
        // from the current session before the final transcription starts.
        let language = self.settings.read().unwrap().whisper_language_code();
        let model = engine.as_mut().unwrap();
        model.whisper_language = language;
        let t0 = std::time::Instant::now();
        let result = model.transcribe(samples);
        drop(engine);
        if result.is_ok() {
            self.observe_rtf(
                &active_id,
                samples.len() as f32 / 16_000.0,
                t0.elapsed().as_secs_f32(),
            );
        }
        result
    }

    /// Like [`Self::transcribe`], but to timestamped words (chunking.rs).
    /// Errors when the active engine has no word timing — callers only use
    /// this on the Parakeet path.
    pub fn transcribe_words(
        &self,
        app: &AppHandle,
        samples: &[f32],
    ) -> anyhow::Result<Vec<crate::stt::Word>> {
        let mut engine = self.engine.lock().unwrap();
        let active_id = self.load_if_needed(app, &mut engine)?;
        let t0 = std::time::Instant::now();
        let result = engine.as_mut().unwrap().transcribe_words(samples);
        drop(engine);
        let words = result?
            .ok_or_else(|| anyhow::anyhow!("engine '{active_id}' has no word timestamps"))?;
        self.observe_rtf(
            &active_id,
            samples.len() as f32 / 16_000.0,
            t0.elapsed().as_secs_f32(),
        );
        Ok(words)
    }

    /// Unload the engine (model switched or deleted).
    pub fn unload_engine(&self) {
        *self.engine.lock().unwrap() = None;
    }

    pub fn hotkeys_set_cancel(&self, registered: bool) {
        if let Some(service) = self.hotkeys.lock().unwrap().as_ref() {
            service.set_cancel_registered(registered);
        }
    }
}
