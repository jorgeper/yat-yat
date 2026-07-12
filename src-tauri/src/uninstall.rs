//! User-facing uninstall (SPEC11): a pure, testable plan of everything the
//! app ever created, plus best-effort execution that ends with the bundle in
//! the Trash. The path list MIRRORS scripts/deep-clean.sh — drift between
//! the two is a bug. (The launch-at-login agent is handled at execution via
//! the autostart plugin rather than appearing as a plan item.)

use serde::Serialize;
use std::path::{Path, PathBuf};

const BUNDLE_ID: &str = "com.yatyat.app";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PlanItem {
    pub path: String,
    pub kind: &'static str,
    pub bytes: u64,
}

/// Recursive size of a file or directory; 0 for anything unreadable.
fn size_of(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| size_of(&e.path()))
        .sum()
}

/// The uninstall plan (SPEC11 §1, R15): exactly what an uninstall removes.
/// `keep_data = true` excludes app data (settings/history) but models still
/// go — they are re-downloadable and huge. Missing paths are omitted.
pub fn uninstall_plan(data_dir: &Path, home: &Path, keep_data: bool) -> Vec<PlanItem> {
    let mut items: Vec<PlanItem> = Vec::new();
    let mut push = |path: PathBuf, kind: &'static str, bytes: u64| {
        if bytes > 0 || path.exists() {
            items.push(PlanItem {
                path: path.to_string_lossy().into_owned(),
                kind,
                bytes,
            });
        }
    };

    let models = data_dir.join("models");
    if models.exists() {
        push(models.clone(), "models", size_of(&models));
    }
    if !keep_data && data_dir.exists() {
        // Everything in the data dir EXCEPT the models subtree (counted above).
        let bytes = size_of(data_dir).saturating_sub(size_of(&models));
        push(data_dir.to_path_buf(), "app_data", bytes);
    }

    let library = home.join("Library");
    for (rel, kind) in [
        (format!("Preferences/{BUNDLE_ID}.plist"), "preferences"),
        (format!("WebKit/{BUNDLE_ID}"), "webkit"),
        (format!("Caches/{BUNDLE_ID}"), "caches"),
        (format!("HTTPStorages/{BUNDLE_ID}"), "caches"),
        (format!("Saved Application State/{BUNDLE_ID}.savedState"), "saved_state"),
    ] {
        let path = library.join(&rel);
        if path.exists() {
            let bytes = size_of(&path);
            push(path, kind, bytes);
        }
    }

    items
}

/// Best-effort execution (SPEC11 §2): every failure is logged and skipped —
/// an uninstall must never strand the user half-way. macOS only; the
/// Windows path launches the NSIS uninstaller instead (see `execute`).
#[cfg(target_os = "macos")]
fn execute_macos(app: &tauri::AppHandle, keep_data: bool) {
    use tauri::Manager;
    let state = app.state::<crate::state::AppState>();

    // 1. Disable launch-at-login (also removes the agent).
    {
        use tauri_plugin_autostart::ManagerExt;
        if let Err(e) = app.autolaunch().disable() {
            log::warn!("uninstall: autostart disable failed: {e}");
        }
    }

    // 2. Delete every plan item.
    let home = dirs::home_dir().unwrap_or_default();
    for item in uninstall_plan(&state.data_dir, &home, keep_data) {
        let path = Path::new(&item.path);
        let result = if path.is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        };
        if let Err(e) = result {
            log::warn!("uninstall: removing {} failed: {e}", item.path);
        }
    }

    // 3. Reset our own TCC grants (no privileges needed for our own entries).
    for service in ["Accessibility", "Microphone"] {
        match std::process::Command::new("/usr/bin/tccutil")
            .args(["reset", service, BUNDLE_ID])
            .status()
        {
            Ok(s) if s.success() => {}
            other => log::warn!("uninstall: tccutil reset {service}: {other:?}"),
        }
    }

    // 4. The bundle goes to the Trash — never a hard delete of a running app.
    if let Some(bundle) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.ancestors().nth(3).map(Path::to_path_buf))
        .filter(|p| p.extension().is_some_and(|e| e == "app"))
    {
        trash_bundle(&bundle);
    } else {
        log::warn!("uninstall: not running from an .app bundle; nothing to trash");
    }

    // Give the IPC response a beat to flush, then exit.
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(600));
        app.exit(0);
    });
}

#[cfg(target_os = "macos")]
fn trash_bundle(bundle: &Path) {
    use objc2::runtime::{AnyObject, Bool};
    use objc2::{class, msg_send};
    let Ok(c_path) = std::ffi::CString::new(bundle.to_string_lossy().into_owned()) else {
        return;
    };
    unsafe {
        let ns_path: *mut AnyObject =
            msg_send![class!(NSString), stringWithUTF8String: c_path.as_ptr()];
        if ns_path.is_null() {
            return;
        }
        let url: *mut AnyObject = msg_send![class!(NSURL), fileURLWithPath: ns_path];
        let fm: *mut AnyObject = msg_send![class!(NSFileManager), defaultManager];
        if url.is_null() || fm.is_null() {
            return;
        }
        let ok: Bool = msg_send![
            fm,
            trashItemAtURL: url,
            resultingItemURL: std::ptr::null_mut::<*mut AnyObject>(),
            error: std::ptr::null_mut::<*mut AnyObject>()
        ];
        if ok.as_bool() {
            log::info!("uninstall: moved {} to the Trash", bundle.display());
        } else {
            log::warn!("uninstall: trashing {} failed", bundle.display());
        }
    }
}

/// Entry point for the `uninstall_app` command.
pub fn execute(app: &tauri::AppHandle, keep_data: bool) {
    #[cfg(target_os = "macos")]
    execute_macos(app, keep_data);

    #[cfg(target_os = "windows")]
    {
        let _ = keep_data; // the NSIS uninstaller owns data removal (hooks.nsh)
        if let Some(uninst) = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|d| d.join("uninstall.exe")))
            .filter(|p| p.exists())
        {
            if let Err(e) = std::process::Command::new(uninst).spawn() {
                log::warn!("uninstall: launching NSIS uninstaller failed: {e}");
                return;
            }
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(600));
                app.exit(0);
            });
        } else {
            log::warn!("uninstall: NSIS uninstaller not found next to the binary");
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (app, keep_data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, bytes: usize) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![0u8; bytes]).unwrap();
    }

    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let data = home.join("Library/Application Support/com.yatyat.app");
        write(&data.join("settings.json"), 100);
        write(&data.join("history.json"), 50);
        write(&data.join("models/tiny.bin"), 4000);
        write(&home.join("Library/Preferences/com.yatyat.app.plist"), 10);
        write(&home.join("Library/Caches/com.yatyat.app/blob"), 300);
        (tmp, data, home)
    }

    fn kinds(items: &[PlanItem]) -> Vec<&'static str> {
        items.iter().map(|i| i.kind).collect()
    }

    // R15: the uninstall plan (SPEC11 §1).
    #[test]
    fn r15_full_plan_finds_items_with_sizes() {
        let (_tmp, data, home) = fixture();
        let plan = uninstall_plan(&data, &home, false);
        let models = plan.iter().find(|i| i.kind == "models").unwrap();
        assert_eq!(models.bytes, 4000);
        let app_data = plan.iter().find(|i| i.kind == "app_data").unwrap();
        assert_eq!(app_data.bytes, 150, "app_data excludes the models subtree");
        assert!(kinds(&plan).contains(&"preferences"));
        assert!(kinds(&plan).contains(&"caches"));
    }

    #[test]
    fn r15_keep_data_excludes_app_data_but_models_still_go() {
        let (_tmp, data, home) = fixture();
        let plan = uninstall_plan(&data, &home, true);
        assert!(!kinds(&plan).contains(&"app_data"));
        assert!(kinds(&plan).contains(&"models"));
    }

    #[test]
    fn r15_missing_paths_are_omitted() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let data = home.join("Library/Application Support/com.yatyat.app");
        // Nothing exists at all -> empty plan, no phantom entries.
        assert!(uninstall_plan(&data, &home, false).is_empty());

        // Only webkit exists -> exactly that one item.
        write(&home.join("Library/WebKit/com.yatyat.app/store"), 20);
        let plan = uninstall_plan(&data, &home, false);
        assert_eq!(kinds(&plan), vec!["webkit"]);
        assert_eq!(plan[0].bytes, 20);
    }

    #[test]
    fn r15_models_classified_as_models_not_app_data() {
        let (_tmp, data, home) = fixture();
        let plan = uninstall_plan(&data, &home, false);
        let models = plan.iter().find(|i| i.kind == "models").unwrap();
        assert!(models.path.ends_with("models"));
        let app_data = plan.iter().find(|i| i.kind == "app_data").unwrap();
        assert!(!app_data.path.ends_with("models"));
    }
}
