//! Recording overlay (SPEC FR-2): a small pill near the bottom-center of the
//! active screen that NEVER takes focus. macOS: non-activating NSPanel via
//! tauri-nspanel; Windows/other: non-focusable always-on-top window. This file
//! is the platform-abstraction boundary for the overlay (see ARCHITECTURE.md).
//! Patterns adapted from Handy (MIT).

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl};

pub const OVERLAY_LABEL: &str = "overlay";
/// The recording box (owner-requested 2026-11-20: smaller, gentler corners
/// than the original 320 x 64 capsule). `overlay_size` picks between these.
/// Compact is the small 288 x 56 bar; regular (the default) is exactly 25%
/// larger each way (owner's sizing, third pass the same day).
pub const OVERLAY_WIDTH: f64 = 360.0;
pub const OVERLAY_HEIGHT: f64 = 70.0;
pub const OVERLAY_COMPACT_WIDTH: f64 = 288.0;
pub const OVERLAY_COMPACT_HEIGHT: f64 = 56.0;
/// States that carry a sentence or buttons (no-model, focus prompt) keep the
/// original roomier box at either size setting.
pub const OVERLAY_NOTICE_WIDTH: f64 = 320.0;
pub const OVERLAY_NOTICE_HEIGHT: f64 = 64.0;
/// Taller pill for live transcription (SPEC3 FR-L3): text above the waveform.
pub const OVERLAY_LIVE_WIDTH: f64 = 420.0;
pub const OVERLAY_LIVE_HEIGHT: f64 = 110.0;
/// Wider pill for the focus-guard prompt (SPEC7 FR-G3): question + 2 buttons.
pub const OVERLAY_FOCUS_WIDTH: f64 = 460.0;
const BOTTOM_OFFSET: f64 = 24.0;

/// Overlay states rendered by the frontend pill (SPEC FR-1.6, FR-2, FR-3.4,
/// SPEC7 FR-G3).
pub mod state {
    pub const RECORDING: &str = "recording";
    pub const TRANSCRIBING: &str = "transcribing";
    pub const NOTHING_HEARD: &str = "nothing-heard";
    pub const NO_MODEL: &str = "no-model";
    pub const FOCUS_CHANGED: &str = "focus-changed";
}

static OVERLAY_VISIBLE: AtomicBool = AtomicBool::new(false);
static SHOW_GENERATION: AtomicU64 = AtomicU64::new(0);
static LAST_LEVEL_EMIT: AtomicU64 = AtomicU64::new(0);
/// Whether the most recent recording used the taller live box, so the
/// transcribing state that follows can stay in that same box.
static LAST_RECORDING_LIVE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
struct ShowPayload {
    state: String,
    live: bool,
    effect: String,
    theme: String,
    /// SPEC14 FR-R5: the eggs switch rides the show payload — the settings
    /// lock is already held here, so the overlay no longer round-trips a
    /// get_settings on every recording start.
    easter_eggs: bool,
    /// App names for the focus-guard prompt (SPEC7 FR-G3); absent elsewhere.
    #[serde(skip_serializing_if = "Option::is_none")]
    from_app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to_app: Option<String>,
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use tauri_nspanel::{tauri_panel, CollectionBehavior, PanelBuilder, PanelLevel, StyleMask};

    tauri_panel! {
        panel!(OverlayPanel {
            config: {
                can_become_key_window: false,
                is_floating_panel: true
            }
        })
    }

    pub fn create(app: &AppHandle) -> anyhow::Result<()> {
        PanelBuilder::<_, OverlayPanel>::new(app, OVERLAY_LABEL)
            .url(WebviewUrl::App("index.html?window=overlay".into()))
            .title("Yat Yat")
            .level(PanelLevel::Status)
            .size(tauri::Size::Logical(tauri::LogicalSize {
                width: OVERLAY_WIDTH,
                height: OVERLAY_HEIGHT,
            }))
            .has_shadow(false)
            .transparent(true)
            .no_activate(true)
            .corner_radius(0.0)
            .style_mask(StyleMask::empty().borderless().nonactivating_panel())
            .with_window(|w| w.decorations(false).transparent(true).focusable(false))
            .collection_behavior(
                CollectionBehavior::new()
                    .can_join_all_spaces()
                    .full_screen_auxiliary(),
            )
            .build()
            .map_err(|e| anyhow::anyhow!("creating overlay panel: {e}"))?;
        if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
            let _ = window.hide();
        }
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::*;
    use tauri::WebviewWindowBuilder;

    pub fn create(app: &AppHandle) -> anyhow::Result<()> {
        WebviewWindowBuilder::new(
            app,
            OVERLAY_LABEL,
            WebviewUrl::App("index.html?window=overlay".into()),
        )
        .title("Yat Yat")
        .inner_size(OVERLAY_WIDTH, OVERLAY_HEIGHT)
        .resizable(false)
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .transparent(true)
        .focusable(false)
        .focused(false)
        .visible(false)
        .accept_first_mouse(true)
        .closable(false)
        .maximizable(false)
        .minimizable(false)
        .build()?;
        Ok(())
    }
}

pub fn create(app: &AppHandle) -> anyhow::Result<()> {
    platform::create(app)
}

/// Position the pill bottom-center of the monitor containing the cursor
/// (logical coordinates — physical positions convert with the wrong monitor's
/// scale factor when moving across screens).
fn position(app: &AppHandle, width: f64, height: f64) -> Option<tauri::LogicalPosition<f64>> {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|pos| app.monitor_from_point(pos.x, pos.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten())?;
    let scale = monitor.scale_factor();
    let mpos = monitor.position().to_logical::<f64>(scale);
    let msize = monitor.size().to_logical::<f64>(scale);
    let x = mpos.x + (msize.width - width) / 2.0;
    let y = mpos.y + msize.height - height - BOTTOM_OFFSET;
    Some(tauri::LogicalPosition { x, y })
}

pub fn show_state(app: &AppHandle, state: &str) {
    show_state_with_mode(app, state, false);
}

/// Show the overlay; `live` selects the taller live-transcription pill for
/// the recording state (SPEC3 FR-L3).
pub fn show_state_with_mode(app: &AppHandle, state: &str, live: bool) {
    show_internal(app, state, live, None, None);
}

/// The focus-guard prompt (SPEC7 FR-G3): "Started in {from} — paste into
/// {to}?" with Paste / Copy only buttons. No auto-dismiss here — the pipeline
/// owns the 10 s timeout and its clipboard fallback.
pub fn show_focus_prompt(app: &AppHandle, from_app: &str, to_app: &str) {
    show_internal(
        app,
        state::FOCUS_CHANGED,
        false,
        Some(from_app.to_string()),
        Some(to_app.to_string()),
    );
}

/// Window size for the non-live, non-prompt states: the recording box follows
/// the `overlay_size` setting ("compact", anything else = regular); the
/// no-model notice keeps the roomier box because it holds a button.
pub fn box_size(state: &str, overlay_size: &str) -> (f64, f64) {
    if state == state::NO_MODEL {
        (OVERLAY_NOTICE_WIDTH, OVERLAY_NOTICE_HEIGHT)
    } else if overlay_size == "compact" {
        (OVERLAY_COMPACT_WIDTH, OVERLAY_COMPACT_HEIGHT)
    } else {
        (OVERLAY_WIDTH, OVERLAY_HEIGHT)
    }
}

/// Whether a show uses the taller live box. Recording decides it; the
/// transcribing state inherits the recording's choice so the box does not
/// change shape when the user stops talking. Every other state is never live.
pub fn effective_live(state: &str, requested_live: bool, last_recording_live: bool) -> bool {
    match state {
        state::RECORDING => requested_live,
        state::TRANSCRIBING => last_recording_live,
        _ => false,
    }
}

fn show_internal(
    app: &AppHandle,
    state: &str,
    live: bool,
    from_app: Option<String>,
    to_app: Option<String>,
) {
    let generation = SHOW_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let live = effective_live(state, live, LAST_RECORDING_LIVE.load(Ordering::SeqCst));
    if state == state::RECORDING {
        LAST_RECORDING_LIVE.store(live, Ordering::SeqCst);
    }
    let (width, height) = if live {
        (OVERLAY_LIVE_WIDTH, OVERLAY_LIVE_HEIGHT)
    } else if state == state::FOCUS_CHANGED {
        (OVERLAY_FOCUS_WIDTH, OVERLAY_NOTICE_HEIGHT)
    } else {
        let size = {
            let app_state = app.state::<crate::state::AppState>();
            let settings = app_state.settings.read().unwrap();
            settings.overlay_size.clone()
        };
        box_size(state, &size)
    };
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }));
        if let Some(pos) = position(app, width, height) {
            let _ = window.set_position(tauri::Position::Logical(pos));
        }
        let _ = window.show();
        OVERLAY_VISIBLE.store(true, Ordering::SeqCst);
        // Appearance travels with every show so the overlay never renders
        // with stale effect/theme choices (SPEC6 FR-A5).
        let (effect, theme, easter_eggs) = {
            use tauri::Manager;
            let state = app.state::<crate::state::AppState>();
            let settings = state.settings.read().unwrap();
            (
                settings.overlay_effect.clone(),
                settings.overlay_theme.clone(),
                settings.easter_eggs,
            )
        };
        let _ = app.emit_to(
            OVERLAY_LABEL,
            "show-overlay",
            ShowPayload {
                state: state.to_string(),
                live,
                effect,
                theme,
                easter_eggs,
                from_app,
                to_app,
            },
        );
    }
    // Transient states auto-dismiss (SPEC: nothing-heard ~1 s, no-model ~6 s).
    let auto_hide_ms = match state {
        state::NOTHING_HEARD => Some(1200),
        state::NO_MODEL => Some(6000),
        _ => None,
    };
    if let Some(ms) = auto_hide_ms {
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(ms));
            if SHOW_GENERATION.load(Ordering::SeqCst) == generation {
                hide(&app);
            }
        });
    }
}

pub fn hide(app: &AppHandle) {
    OVERLAY_VISIBLE.store(false, Ordering::SeqCst);
    let _ = app.emit_to(OVERLAY_LABEL, "hide-overlay", ());
    let app = app.clone();
    // Let the CSS fade-out play before the window vanishes.
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(220));
        if !OVERLAY_VISIBLE.load(Ordering::SeqCst) {
            if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
                let _ = window.hide();
            }
        }
    });
}

/// Forward a mic level (0..1) to the overlay at most ~30×/s. Even a hidden
/// webview pays for every event, so skip entirely when not visible.
pub fn emit_level(app: &AppHandle, level: f32) {
    if !OVERLAY_VISIBLE.load(Ordering::SeqCst) {
        return;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let last = LAST_LEVEL_EMIT.load(Ordering::Relaxed);
    if now.saturating_sub(last) < 30 {
        return;
    }
    LAST_LEVEL_EMIT.store(now, Ordering::Relaxed);
    let _ = app.emit_to(OVERLAY_LABEL, "mic-level", level);
}

/// Push estimated transcription progress (0..1) to the overlay (SPEC13
/// FR-P3). Bare fraction, same shape discipline as "mic-level".
pub fn emit_progress(app: &AppHandle, fraction: f32) {
    if !OVERLAY_VISIBLE.load(Ordering::SeqCst) {
        return;
    }
    let _ = app.emit_to(OVERLAY_LABEL, "transcribe-progress", fraction);
}

/// Push a live-transcription pass result to the overlay (SPEC3 FR-L2).
pub fn emit_stream_text(app: &AppHandle, text: &str) {
    if !OVERLAY_VISIBLE.load(Ordering::SeqCst) {
        return;
    }
    let _ = app.emit_to(
        OVERLAY_LABEL,
        "stream-text",
        serde_json::json!({ "text": text }),
    );
}

#[cfg(test)]
mod size_tests {
    use super::*;

    // R50 (owner-requested 2026-11-20): the recording box follows the
    // overlay_size setting, unknown values read as regular, the no-model
    // notice keeps its roomier box, and legacy settings JSON loads regular.
    #[test]
    fn r50_box_size_follows_setting_and_legacy_json_is_regular() {
        assert_eq!(box_size(state::RECORDING, "regular"), (360.0, 70.0));
        assert_eq!(box_size(state::RECORDING, "compact"), (288.0, 56.0));
        assert_eq!(box_size(state::TRANSCRIBING, "compact"), (288.0, 56.0));
        assert_eq!(box_size(state::RECORDING, "nonsense"), (360.0, 70.0));
        assert_eq!(box_size(state::RECORDING, ""), (360.0, 70.0));
        // Regular is compact scaled by 1.25 each way.
        assert_eq!(OVERLAY_WIDTH, OVERLAY_COMPACT_WIDTH * 1.25);
        assert_eq!(OVERLAY_HEIGHT, OVERLAY_COMPACT_HEIGHT * 1.25);
        assert_eq!(box_size(state::NO_MODEL, "compact"), (320.0, 64.0));

        let legacy: crate::settings::Settings =
            serde_json::from_str(r#"{"hotkey":"command_right"}"#).unwrap();
        assert_eq!(legacy.overlay_size, "regular");
        assert_eq!(crate::settings::Settings::default().overlay_size, "regular");
    }

    // R51 (owner-requested 2026-11-20): transcribing stays in the box the
    // recording used — live or not — and no other state is ever live.
    #[test]
    fn r51_transcribing_inherits_the_recording_box() {
        assert!(effective_live(state::RECORDING, true, false));
        assert!(!effective_live(state::RECORDING, false, true));
        assert!(effective_live(state::TRANSCRIBING, false, true));
        assert!(!effective_live(state::TRANSCRIBING, true, false));
        for other in [state::NOTHING_HEARD, state::NO_MODEL, state::FOCUS_CHANGED] {
            assert!(!effective_live(other, true, true), "{other} is never live");
        }
    }
}
