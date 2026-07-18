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
use std::collections::HashMap;
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
    /// Per-model real-time-factor EMAs (SPEC13 FR-P2, since made
    /// disk-backed — see the ARCHITECTURE.md divergence note). Loaded from
    /// rtf.json at startup; a model never measured estimates from the seed.
    rtf: Mutex<HashMap<String, crate::progress::Rtf>>,
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
            rtf: Mutex::new(rtf),
        }
    }

    /// Estimated raw-STT seconds for `audio_secs` on the active model
    /// (SPEC13 FR-P3).
    pub fn expected_stt_secs(&self, audio_secs: f32) -> f32 {
        let model = self.settings.read().unwrap().active_model.clone().unwrap_or_default();
        let rtf = self.rtf.lock().unwrap().get(&model).copied().unwrap_or_default();
        audio_secs * rtf.estimate()
    }

    /// Fold one measured raw-STT wall time into a model's EMA (SPEC13
    /// FR-P5 — raw engine time only) and persist the map so relaunches
    /// start calibrated.
    fn observe_rtf(&self, model: &str, audio_secs: f32, wall_secs: f32) {
        let mut rtf = self.rtf.lock().unwrap();
        rtf.entry(model.to_string()).or_default().observe(audio_secs, wall_secs);
        crate::progress::save_rtf_map(&self.data_dir.join("rtf.json"), &rtf);
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

    /// Transcribe through the warm engine, (re)loading it if the active model
    /// changed or nothing is loaded yet. Emits model-state-changed events.
    pub fn transcribe(&self, app: &AppHandle, samples: &[f32]) -> anyhow::Result<String> {
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

        let mut engine = self.engine.lock().unwrap();
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
        // Time ONLY the engine call (model load above is excluded) and feed
        // the per-model RTF estimate. Live passes run through here too, so
        // with live transcription on, the progress estimate is calibrated
        // from the current session before the final transcription starts.
        let t0 = std::time::Instant::now();
        let result = engine.as_mut().unwrap().transcribe(samples);
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
