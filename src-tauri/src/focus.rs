//! Focus guard (SPEC7 FR-G): frontmost-application capture and the pure
//! paste/ask decision. This file is the platform-abstraction boundary for
//! frontmost queries (see ARCHITECTURE.md): macOS asks NSWorkspace; other
//! platforms return None, which disables the guard — the guard always fails
//! open and can never block a paste (FR-G1). Windows port point:
//! GetForegroundWindow + QueryFullProcessImageName in `frontmost_app`.

use crate::settings::OutputMethod;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontmostApp {
    /// Comparison key (FR-G2): same bundle = same app, whatever the window.
    pub bundle_id: String,
    /// Human name for the prompt ("Terminal", "Slack"); may be empty.
    pub name: String,
}

impl FrontmostApp {
    /// The name the prompt shows; bundle id when localizedName was empty.
    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.bundle_id
        } else {
            &self.name
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardDecision {
    Deliver,
    Ask,
}

/// The pure focus-guard decision (SPEC7 FR-G2/FR-G3, table-tested by R13).
/// Prompt ONLY when the guard is on, a paste keystroke is coming, and both
/// frontmost reads succeeded with different apps. Everything else delivers.
pub fn decide(
    guard_enabled: bool,
    output: OutputMethod,
    started: Option<&FrontmostApp>,
    current: Option<&FrontmostApp>,
) -> GuardDecision {
    if !guard_enabled || output == OutputMethod::ClipboardOnly {
        return GuardDecision::Deliver;
    }
    match (started, current) {
        (Some(started), Some(current)) if started.bundle_id != current.bundle_id => {
            GuardDecision::Ask
        }
        _ => GuardDecision::Deliver,
    }
}

/// Query the frontmost application from any thread; the AppKit call is
/// marshalled to the main thread (same pattern as tray_probe). Every failure
/// path — no main thread, timeout, nil bundle id — is None: fail open.
pub fn frontmost_app(app: &tauri::AppHandle) -> Option<FrontmostApp> {
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let _ = tx.send(frontmost_app_main_thread());
        })
        .ok()?;
        rx.recv_timeout(std::time::Duration::from_millis(500))
            .ok()
            .flatten()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        None
    }
}

#[cfg(target_os = "macos")]
fn frontmost_app_main_thread() -> Option<FrontmostApp> {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use std::ffi::CStr;

    unsafe fn nsstring(obj: *mut AnyObject) -> Option<String> {
        if obj.is_null() {
            return None;
        }
        let cstr: *const std::os::raw::c_char = unsafe { objc2::msg_send![obj, UTF8String] };
        if cstr.is_null() {
            return None;
        }
        Some(unsafe { CStr::from_ptr(cstr) }.to_string_lossy().into_owned())
    }

    unsafe {
        let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return None;
        }
        let front: *mut AnyObject = msg_send![workspace, frontmostApplication];
        if front.is_null() {
            return None;
        }
        let bundle: *mut AnyObject = msg_send![front, bundleIdentifier];
        let name: *mut AnyObject = msg_send![front, localizedName];
        // Unbundled processes have no bundle id — without a comparison key
        // the guard stands down (None).
        let bundle_id = nsstring(bundle)?;
        Some(FrontmostApp {
            bundle_id,
            name: nsstring(name).unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(bundle: &str) -> FrontmostApp {
        FrontmostApp {
            bundle_id: bundle.into(),
            name: String::new(),
        }
    }

    // R13: the pure decision table (SPEC7 FR-G2/FR-G5).
    #[test]
    fn r13_same_app_delivers() {
        let a = app("com.apple.Terminal");
        assert_eq!(
            decide(true, OutputMethod::Paste, Some(&a), Some(&a.clone())),
            GuardDecision::Deliver
        );
    }

    #[test]
    fn r13_different_app_asks() {
        let a = app("com.apple.Terminal");
        let b = app("com.tinyspeck.slackmacgap");
        assert_eq!(
            decide(true, OutputMethod::Paste, Some(&a), Some(&b)),
            GuardDecision::Ask
        );
    }

    #[test]
    fn r13_fails_open_on_missing_reads() {
        let a = app("com.apple.Terminal");
        // Either read (or both) unavailable -> deliver, never block.
        assert_eq!(
            decide(true, OutputMethod::Paste, None, Some(&a)),
            GuardDecision::Deliver
        );
        assert_eq!(
            decide(true, OutputMethod::Paste, Some(&a), None),
            GuardDecision::Deliver
        );
        assert_eq!(
            decide(true, OutputMethod::Paste, None, None),
            GuardDecision::Deliver
        );
    }

    #[test]
    fn r13_clipboard_only_never_asks() {
        let a = app("com.apple.Terminal");
        let b = app("com.tinyspeck.slackmacgap");
        assert_eq!(
            decide(true, OutputMethod::ClipboardOnly, Some(&a), Some(&b)),
            GuardDecision::Deliver
        );
    }

    #[test]
    fn r13_disabled_guard_never_asks() {
        let a = app("com.apple.Terminal");
        let b = app("com.tinyspeck.slackmacgap");
        assert_eq!(
            decide(false, OutputMethod::Paste, Some(&a), Some(&b)),
            GuardDecision::Deliver
        );
    }

    #[test]
    fn r13_same_bundle_different_window_is_same_app() {
        // Window/tab moves within one app never prompt (FR-G2 compares
        // bundle ids, nothing finer).
        let a = FrontmostApp {
            bundle_id: "com.apple.Terminal".into(),
            name: "Terminal — window 1".into(),
        };
        let b = FrontmostApp {
            bundle_id: "com.apple.Terminal".into(),
            name: "Terminal — window 2".into(),
        };
        assert_eq!(
            decide(true, OutputMethod::Paste, Some(&a), Some(&b)),
            GuardDecision::Deliver
        );
    }

    #[test]
    fn r13_display_name_falls_back_to_bundle_id() {
        assert_eq!(app("com.example.x").display_name(), "com.example.x");
        let named = FrontmostApp {
            bundle_id: "com.example.x".into(),
            name: "Example".into(),
        };
        assert_eq!(named.display_name(), "Example");
    }
}
