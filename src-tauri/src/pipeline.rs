//! Dictation pipeline coordinator (SPEC FR-1/FR-3): a single thread owning the
//! Idle -> Recording -> Processing state machine, so every lifecycle event is
//! serialized (pattern adapted from Handy, MIT).

use crate::history::HistoryEntry;
use crate::settings::{ActivationMode, OutputMethod};
use crate::state::AppState;
use crate::{audio, overlay, tray};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const PRESS_DEBOUNCE: Duration = Duration::from_millis(30);
/// Unanswered focus-guard prompts fall back to the clipboard (SPEC7 FR-G4).
const FOCUS_PROMPT_TIMEOUT_SECS: u64 = 10;

#[derive(Debug)]
enum Msg {
    Hotkey { pressed: bool },
    Toggle,
    Cancel,
    Retry,
    AutoStop { generation: u64 },
    FocusResolve { action: FocusAction },
    FocusTimeout { generation: u64 },
}

/// How a pending focus-guard prompt resolves (SPEC7 FR-G3/FR-G4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusAction {
    /// Deliver to the now-frontmost app (button, or the hotkey pressed again).
    Paste,
    /// Clipboard only (the "Copy only" button).
    Copy,
    /// Clipboard + notification (timeout, or a new dictation preempting).
    CopyAndNotify,
    /// Esc: no delivery — the text stays in history and last-transcription.
    Dismiss,
}

#[derive(Debug, PartialEq)]
enum Stage {
    Idle,
    Recording,
    /// Focus moved mid-dictation; the transcript waits in
    /// `AppState::pending_paste` while the overlay asks (SPEC7 FR-G5 — the
    /// pipeline thread keeps consuming messages, it never blocks on the human).
    AwaitFocusConfirm,
}

#[derive(Clone)]
pub struct Pipeline {
    tx: mpsc::Sender<Msg>,
}

impl Pipeline {
    pub fn start(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel();
        let tx_clone = tx.clone();
        std::thread::Builder::new()
            .name("pipeline".into())
            .spawn(move || run(app, rx, tx_clone))
            .expect("spawn pipeline thread");
        Self { tx }
    }

    pub fn hotkey(&self, pressed: bool) {
        let _ = self.tx.send(Msg::Hotkey { pressed });
    }

    pub fn toggle(&self) {
        let _ = self.tx.send(Msg::Toggle);
    }

    pub fn cancel(&self) {
        let _ = self.tx.send(Msg::Cancel);
    }

    pub fn retry(&self) {
        let _ = self.tx.send(Msg::Retry);
    }

    /// Resolve a pending focus-guard prompt (overlay buttons via IPC).
    pub fn focus_resolve(&self, action: FocusAction) {
        let _ = self.tx.send(Msg::FocusResolve { action });
    }
}

fn run(app: AppHandle, rx: mpsc::Receiver<Msg>, tx: mpsc::Sender<Msg>) {
    let mut stage = Stage::Idle;
    let mut last_press: Option<Instant> = None;
    let mut generation: u64 = 0;

    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Hotkey { pressed } => {
                if pressed {
                    let now = Instant::now();
                    if last_press.is_some_and(|t| now.duration_since(t) < PRESS_DEBOUNCE) {
                        continue;
                    }
                    last_press = Some(now);
                }
                // SPEC7 FR-G3: while the prompt is up, the dictation hotkey
                // means "yes, paste here". Key releases are ignored.
                if stage == Stage::AwaitFocusConfirm {
                    if pressed {
                        resolve_focus(&app, FocusAction::Paste);
                        stage = Stage::Idle;
                    }
                    continue;
                }
                let hold_mode = {
                    let state = app.state::<AppState>();
                    let settings = state.settings.read().unwrap();
                    settings.activation_mode == ActivationMode::Hold
                };
                if hold_mode {
                    match (&stage, pressed) {
                        (Stage::Idle, true) => {
                            if start_recording(&app, &mut generation, &tx) {
                                stage = Stage::Recording;
                            }
                        }
                        (Stage::Recording, false) => {
                            stage = finish_recording(&app, generation, &tx);
                        }
                        _ => {}
                    }
                } else if pressed {
                    match stage {
                        Stage::Idle => {
                            if start_recording(&app, &mut generation, &tx) {
                                stage = Stage::Recording;
                            }
                        }
                        Stage::Recording => {
                            stage = finish_recording(&app, generation, &tx);
                        }
                        Stage::AwaitFocusConfirm => unreachable!("handled above"),
                    }
                }
            }
            Msg::Toggle => match stage {
                Stage::Idle => {
                    if start_recording(&app, &mut generation, &tx) {
                        stage = Stage::Recording;
                    }
                }
                Stage::Recording => {
                    stage = finish_recording(&app, generation, &tx);
                }
                Stage::AwaitFocusConfirm => {
                    // A new dictation preempts the prompt like the timeout
                    // does (SPEC7 FR-G4): copy + notify, then record.
                    resolve_focus(&app, FocusAction::CopyAndNotify);
                    stage = Stage::Idle;
                    if start_recording(&app, &mut generation, &tx) {
                        stage = Stage::Recording;
                    }
                }
            },
            Msg::AutoStop { generation: g } => {
                if stage == Stage::Recording && g == generation {
                    log::warn!("5-minute safety cap reached; stopping dictation");
                    stage = finish_recording(&app, generation, &tx);
                }
            }
            Msg::Cancel => match stage {
                Stage::Recording => {
                    let state = app.state::<AppState>();
                    state.recorder.cancel();
                    state.hotkeys_set_cancel(false);
                    overlay::hide(&app);
                    tray::set_state(&app, tray::TrayState::Idle);
                    stage = Stage::Idle;
                }
                Stage::AwaitFocusConfirm => {
                    // Esc dismisses the prompt; the transcript stays in
                    // history and last-transcription (SPEC7 FR-G4).
                    resolve_focus(&app, FocusAction::Dismiss);
                    stage = Stage::Idle;
                }
                Stage::Idle => {}
            },
            Msg::Retry => {
                if stage == Stage::Idle {
                    retry_last(&app);
                    stage = Stage::Idle;
                }
            }
            Msg::FocusResolve { action } => {
                // Ignore stale clicks arriving after a timeout already resolved.
                if stage == Stage::AwaitFocusConfirm {
                    resolve_focus(&app, action);
                    stage = Stage::Idle;
                }
            }
            Msg::FocusTimeout { generation: g } => {
                if stage == Stage::AwaitFocusConfirm && g == generation {
                    resolve_focus(&app, FocusAction::CopyAndNotify);
                    stage = Stage::Idle;
                }
            }
        }
    }
}

/// Returns true if recording actually started.
fn start_recording(app: &AppHandle, generation: &mut u64, tx: &mpsc::Sender<Msg>) -> bool {
    let t0 = Instant::now();
    let state = app.state::<AppState>();

    // No-model flow (SPEC FR-1.6).
    if !state.active_model_ready() {
        overlay::show_state(app, overlay::state::NO_MODEL);
        return false;
    }

    let (device, enhancement, live_mode, focus_guard) = {
        let settings = state.settings.read().unwrap();
        (
            settings.input_device.clone(),
            settings.enhancement.clone(),
            settings.live_transcription,
            settings.focus_guard,
        )
    };

    if let Err(e) = state.recorder.start(device) {
        log::error!("failed to start recording: {e}");
        let _ = app.emit("pipeline-error", format!("Could not start recording: {e}"));
        return false;
    }

    crate::sounds::play(app, crate::sounds::Cue::Start);

    overlay::show_state_with_mode(app, overlay::state::RECORDING, live_mode);
    tray::set_state(app, tray::TrayState::Recording);
    state.hotkeys_set_cancel(true);

    // SPEC7 FR-G1: the app frontmost right now IS the paste target — the
    // overlay panel never activates, so capturing after the show is
    // equivalent and keeps the main-thread roundtrip off the <150 ms
    // hotkey→overlay path. None simply disables the guard for this
    // dictation — fail open.
    *state.dictation_start_app.lock().unwrap() = if focus_guard {
        crate::focus::frontmost_app(app)
    } else {
        None
    };

    // Live transcription loop (SPEC3 FR-L2): periodic passes over the audio
    // so far, emitted to the overlay. Ends by itself when recording stops;
    // never touches the authoritative stop path.
    if live_mode {
        let app_live = app.clone();
        std::thread::Builder::new()
            .name("live-transcribe".into())
            .spawn(move || live_loop(app_live))
            .expect("spawn live loop");
    }

    // Pre-warm the enhancement model so a cold Ollama never delays the paste.
    if enhancement.enabled {
        tauri::async_runtime::spawn(async move {
            crate::enhance::warmup(&enhancement).await;
        });
    }

    // 5-minute safety cap watchdog (SPEC FR-1.4).
    *generation += 1;
    let g = *generation;
    let tx = tx.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(audio::MAX_RECORD_SECS));
        let _ = tx.send(Msg::AutoStop { generation: g });
    });

    log::info!("hotkey -> recording overlay in {:?}", t0.elapsed());
    true
}

/// Periodic live passes while recording (SPEC3 FR-L2). Degrades gracefully:
/// engine failure ends the loop (waveform-only recording continues), slow
/// passes stretch the interval via the perf guard.
fn live_loop(app: AppHandle) {
    let mut interval = crate::live::LiveInterval::default();
    loop {
        std::thread::sleep(interval.current());
        let state = app.state::<AppState>();
        if !state.recorder.is_recording() {
            return;
        }
        let Some(samples) = state.recorder.snapshot_16k() else {
            return; // stopped between the check and the snapshot
        };
        // Sub-second audio produces engine noise; wait for the next tick.
        if samples.len() < crate::audio::TARGET_RATE as usize {
            continue;
        }
        let t0 = Instant::now();
        match state.transcribe(&app, &samples) {
            Ok(text) => {
                // The user may have stopped while the pass ran; a late emit
                // would flash stale text over the transcribing state.
                if state.recorder.is_recording() {
                    overlay::emit_stream_text(&app, &text);
                }
            }
            Err(e) => {
                log::warn!("live pass failed; continuing waveform-only: {e}");
                return;
            }
        }
        interval.on_pass(t0.elapsed());
    }
}

/// Stop, transcribe, and deliver — or hand off to the focus-guard prompt.
/// Returns the pipeline's next stage (Idle, or AwaitFocusConfirm when the
/// prompt is showing).
fn finish_recording(app: &AppHandle, generation: u64, tx: &mpsc::Sender<Msg>) -> Stage {
    let t0 = Instant::now();
    let state = app.state::<AppState>();
    state.hotkeys_set_cancel(false);
    overlay::show_state(app, overlay::state::TRANSCRIBING);
    tray::set_state(app, tray::TrayState::Processing);

    let done = |app: &AppHandle| {
        tray::set_state(app, tray::TrayState::Idle);
    };

    let samples = match state.recorder.stop() {
        Ok(s) => s,
        Err(e) => {
            log::error!("stopping recorder: {e}");
            overlay::hide(app);
            done(app);
            return Stage::Idle;
        }
    };
    if samples.is_empty() {
        overlay::show_state(app, overlay::state::NOTHING_HEARD);
        done(app);
        return Stage::Idle;
    }

    // Retain audio for Retry (FR-5) unless history is disabled.
    let keep_history = state.settings.read().unwrap().keep_history;
    if keep_history {
        let path = crate::history::last_audio_path(&state.data_dir);
        if let Err(e) = audio::write_wav_16k_mono(&path, &samples) {
            log::warn!("could not retain last recording: {e}");
        }
    }

    match transcribe_and_clean(app, &samples) {
        Ok(text) if text.is_empty() => {
            overlay::show_state(app, overlay::state::NOTHING_HEARD);
        }
        Ok(text) => {
            // SPEC7 FR-G4: the transcript reaches history and
            // last-transcription BEFORE any delivery decision — no guard
            // outcome can lose text.
            record_transcription(app, &text);

            // The focus-guard check (SPEC7 FR-G2) runs here — after STT,
            // cleanup AND enhancement, immediately before delivery: exactly
            // the window in which the user wanders off to another app.
            let started = state.dictation_start_app.lock().unwrap().take();
            let (guard_enabled, method) = {
                let settings = state.settings.read().unwrap();
                (settings.focus_guard, settings.output_method)
            };
            // Without Accessibility the delivery degrades to clipboard-only
            // anyway — nothing to guard.
            let effective_method = if method == OutputMethod::Paste
                && !state.paster.lock().unwrap().can_paste()
            {
                OutputMethod::ClipboardOnly
            } else {
                method
            };
            let current = crate::focus::frontmost_app(app);
            match crate::focus::decide(
                guard_enabled,
                effective_method,
                started.as_ref(),
                current.as_ref(),
            ) {
                crate::focus::GuardDecision::Deliver => {
                    deliver_text(app, &text, false);
                    overlay::hide(app);
                    log::info!("stop -> delivered in {:?}", t0.elapsed());
                }
                crate::focus::GuardDecision::Ask => {
                    // decide() only Asks when both reads were Some.
                    let from = started.map(|a| a.display_name().to_string()).unwrap_or_default();
                    let to = current.map(|a| a.display_name().to_string()).unwrap_or_default();
                    *state.pending_paste.lock().unwrap() = Some(text);
                    state.hotkeys_set_cancel(true); // Esc dismisses the prompt
                    overlay::show_focus_prompt(app, &from, &to);
                    // Unanswered prompts fall back to the clipboard (FR-G4).
                    let tx = tx.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_secs(FOCUS_PROMPT_TIMEOUT_SECS));
                        let _ = tx.send(Msg::FocusTimeout { generation });
                    });
                    log::info!("stop -> focus prompt in {:?} ({from} -> {to})", t0.elapsed());
                    done(app);
                    return Stage::AwaitFocusConfirm;
                }
            }
        }
        Err(e) => {
            log::error!("transcription failed: {e}");
            let _ = app.emit("pipeline-error", format!("Transcription failed: {e}"));
            overlay::hide(app);
        }
    }
    done(app);
    Stage::Idle
}

/// Resolve a pending focus-guard prompt (SPEC7 FR-G3/FR-G4). The pending
/// paste re-checks nothing — the user just confirmed the target (FR-G5).
fn resolve_focus(app: &AppHandle, action: FocusAction) {
    let state = app.state::<AppState>();
    state.hotkeys_set_cancel(false);
    let text = state.pending_paste.lock().unwrap().take();
    if let Some(text) = text {
        match action {
            FocusAction::Paste => deliver_text(app, &text, false),
            FocusAction::Copy => {
                let _ = crate::paste::copy_to_clipboard(&text);
                crate::sounds::play(app, crate::sounds::Cue::Delivered);
            }
            FocusAction::CopyAndNotify => {
                let _ = crate::paste::copy_to_clipboard(&text);
                crate::sounds::play(app, crate::sounds::Cue::Delivered);
                notify(
                    app,
                    "Focus changed",
                    "The transcription was copied to the clipboard instead.",
                );
            }
            FocusAction::Dismiss => {}
        }
    }
    overlay::hide(app);
    tray::set_state(app, tray::TrayState::Idle);
}

/// STT + deterministic cleanup + optional enhancement (SPEC §4).
fn transcribe_and_clean(app: &AppHandle, samples: &[f32]) -> anyhow::Result<String> {
    let state = app.state::<AppState>();
    let raw = state.transcribe(app, samples)?;
    let (clean_opts, enhancement) = {
        let settings = state.settings.read().unwrap();
        (settings.clean_options(), settings.enhancement.clone())
    };
    let filtered = crate::cleanup::clean(&raw, &clean_opts);
    if filtered.is_empty() || !enhancement.enabled {
        return Ok(filtered);
    }
    match tauri::async_runtime::block_on(crate::enhance::enhance(&filtered, &enhancement)) {
        Ok(enhanced) => Ok(enhanced),
        Err(e) => {
            log::warn!("enhancement skipped: {e}");
            let _ = app.emit("enhancement-skipped", e.to_string());
            Ok(filtered)
        }
    }
}

/// History + last transcription (FR-5) — runs before any delivery decision
/// so every focus-guard outcome keeps the text (SPEC7 FR-G4).
fn record_transcription(app: &AppHandle, text: &str) {
    let state = app.state::<AppState>();
    *state.last_transcription.lock().unwrap() = Some(text.to_string());
    let (keep_history, model_id) = {
        let settings = state.settings.read().unwrap();
        (settings.keep_history, settings.active_model.clone())
    };
    if keep_history {
        let mut history = state.history.lock().unwrap();
        history.push(HistoryEntry {
            text: text.to_string(),
            timestamp: chrono::Utc::now(),
            model_id: model_id.unwrap_or_default(),
        });
        let _ = history.save(&state.history_path());
    }
    let _ = app.emit("history-changed", ());
    tray::refresh_menu(app, tray::TrayState::Idle);
}

/// Paste (or copy) the final text on the main thread.
fn deliver_text(app: &AppHandle, text: &str, copy_only: bool) {
    let state = app.state::<AppState>();
    let output_method = if copy_only {
        OutputMethod::ClipboardOnly
    } else {
        state.settings.read().unwrap().output_method
    };

    // Paste must happen on the main thread on macOS.
    let (done_tx, done_rx) = mpsc::channel::<bool>();
    {
        let app2 = app.clone();
        let text2 = text.to_string();
        let _ = app.run_on_main_thread(move || {
            let state = app2.state::<AppState>();
            let mut paster = state.paster.lock().unwrap();
            if output_method == OutputMethod::Paste && !paster.can_paste() {
                paster.reinit();
            }
            let pasted = match paster.deliver(&text2, output_method) {
                Ok(pasted) => pasted,
                Err(e) => {
                    log::error!("delivering text: {e}");
                    false
                }
            };
            if output_method == OutputMethod::Paste && !pasted {
                // Degraded to clipboard-only (FR-3.3): tell the user why.
                let _ = app2.emit(
                    "paste-degraded",
                    "Pasting needs the Accessibility permission — the text was copied to the clipboard instead.",
                );
                notify(
                    &app2,
                    "Copied to clipboard",
                    "Grant Accessibility in System Settings to let Yat Yat paste directly.",
                );
            }
            let _ = done_tx.send(pasted);
        });
    }
    let _ = done_rx.recv_timeout(Duration::from_secs(10));
    // Delivered cue (SPEC7 FR-C2): after deliver returns, both output methods.
    crate::sounds::play(app, crate::sounds::Cue::Delivered);
}

/// Retry Last Transcription (FR-4.3): re-run STT + cleanup on the retained
/// audio, replace the stored last transcription, copy to clipboard, notify.
fn retry_last(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.active_model_ready() {
        overlay::show_state(app, overlay::state::NO_MODEL);
        return;
    }
    let path = crate::history::last_audio_path(&state.data_dir);
    if !path.exists() {
        notify(app, "Nothing to retry", "No previous recording is available.");
        return;
    }
    tray::set_state(app, tray::TrayState::Processing);
    let result = (|| -> anyhow::Result<String> {
        let samples = crate::stt::read_wav_16k_mono(&path)?;
        transcribe_and_clean(app, &samples)
    })();
    match result {
        Ok(text) if !text.is_empty() => {
            {
                let mut history = state.history.lock().unwrap();
                let model_id = state
                    .settings
                    .read()
                    .unwrap()
                    .active_model
                    .clone()
                    .unwrap_or_default();
                history.replace_latest(HistoryEntry {
                    text: text.clone(),
                    timestamp: chrono::Utc::now(),
                    model_id,
                });
                let _ = history.save(&state.history_path());
            }
            *state.last_transcription.lock().unwrap() = Some(text.clone());
            let _ = crate::paste::copy_to_clipboard(&text);
            let _ = app.emit("history-changed", ());
            notify(app, "Retry complete", "The new transcription was copied to the clipboard.");
        }
        Ok(_) => notify(app, "Retry produced nothing", "The recording contained no speech."),
        Err(e) => {
            log::error!("retry failed: {e}");
            notify(app, "Retry failed", &e.to_string());
        }
    }
    tray::set_state(app, tray::TrayState::Idle);
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title(title).body(body).show();
}
