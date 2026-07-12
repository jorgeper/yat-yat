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
    #[cfg(target_os = "windows")]
    {
        let _ = app; // Win32 foreground queries are callable from any thread.
        frontmost_app_windows()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        None
    }
}

/// SPEC10 FR-S1: normalize a Windows executable path into the guard's
/// comparison key — the lowercased, backslash-normalized full path (Windows'
/// analog of a bundle id) — plus a display name (the file stem). Pure and
/// compiled on every platform so R14 runs everywhere. None on anything
/// unusable: the guard fails open.
pub fn windows_exe_key(path: &str) -> Option<FrontmostApp> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.replace('/', "\\");
    let file = normalized.rsplit('\\').next().unwrap_or(&normalized);
    let name = std::path::Path::new(file)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file)
        .to_string();
    if name.is_empty() {
        return None;
    }
    Some(FrontmostApp {
        bundle_id: normalized.to_lowercase(),
        name,
    })
}

#[cfg(target_os = "windows")]
fn frontmost_app_windows() -> Option<FrontmostApp> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 || len == 0 {
            return None;
        }
        windows_exe_key(&String::from_utf16_lossy(&buf[..len as usize]))
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

    // R14: the Windows executable-path key normalization (SPEC10 FR-S1) —
    // pure, so it runs on every platform.
    #[test]
    fn r14_lowercases_the_full_path_as_the_key() {
        let a = windows_exe_key(r"C:\Program Files\Microsoft Office\WINWORD.EXE").unwrap();
        assert_eq!(a.bundle_id, r"c:\program files\microsoft office\winword.exe");
        assert_eq!(a.name, "WINWORD");
    }

    #[test]
    fn r14_case_and_slash_variants_compare_equal() {
        let a = windows_exe_key(r"C:\Windows\System32\Notepad.exe").unwrap();
        let b = windows_exe_key("c:/windows/system32/NOTEPAD.EXE").unwrap();
        assert_eq!(a.bundle_id, b.bundle_id);
    }

    #[test]
    fn r14_display_name_is_the_file_stem() {
        assert_eq!(windows_exe_key(r"D:\Tools\slack.exe").unwrap().name, "slack");
        // No extension: the file name itself.
        assert_eq!(windows_exe_key(r"C:\odd\binary").unwrap().name, "binary");
    }

    #[test]
    fn r14_unusable_paths_fail_open_as_none() {
        assert!(windows_exe_key("").is_none());
        assert!(windows_exe_key("   ").is_none());
        assert!(windows_exe_key(r"C:\ends\in\slash\").is_none());
    }
}
