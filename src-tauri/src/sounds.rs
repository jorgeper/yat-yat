//! Sound cues (SPEC7 FR-C): a soft tick when recording starts, a soft click
//! when text is delivered. Playback is fire-and-forget — no sleeps, nothing
//! on the dictation path ever waits on audio. This file is the platform
//! boundary for cue playback: macOS spawns the system `afplay`; other
//! platforms are a no-op stub (Windows port point: PlaySoundW SND_ASYNC).

use tauri::AppHandle;

#[derive(Debug, Clone, Copy)]
pub enum Cue {
    Start,
    Delivered,
}

impl Cue {
    fn file(self) -> &'static str {
        match self {
            Cue::Start => "cue-start.wav",
            Cue::Delivered => "cue-delivered.wav",
        }
    }
}

/// Play `cue` if the sound_cues setting is on (SPEC7 FR-C3). Returns
/// immediately in every case.
pub fn play(app: &AppHandle, cue: Cue) {
    let enabled = {
        use tauri::Manager;
        let state = app.state::<crate::state::AppState>();
        let settings = state.settings.read().unwrap();
        settings.sound_cues
    };
    if enabled {
        play_now(app, cue);
    }
}

#[cfg(target_os = "macos")]
fn play_now(app: &AppHandle, cue: Cue) {
    use tauri::path::BaseDirectory;
    use tauri::Manager;
    let Ok(path) = app
        .path()
        .resolve(format!("resources/{}", cue.file()), BaseDirectory::Resource)
    else {
        return;
    };
    match std::process::Command::new("afplay").arg(&path).spawn() {
        Ok(mut child) => {
            // Reap off-path so short-lived players never pile up as zombies.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(e) => log::debug!("sound cue playback failed: {e}"),
    }
}

/// SPEC10 FR-S2: PlaySoundW on a dedicated worker thread. SND_SYNC on that
/// thread (not SND_ASYNC) so the wide-string buffer provably outlives
/// playback — the dictation path itself never waits.
#[cfg(target_os = "windows")]
fn play_now(app: &AppHandle, cue: Cue) {
    use std::os::windows::ffi::OsStrExt;
    use tauri::path::BaseDirectory;
    use tauri::Manager;
    let Ok(path) = app
        .path()
        .resolve(format!("resources/{}", cue.file()), BaseDirectory::Resource)
    else {
        return;
    };
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    std::thread::spawn(move || unsafe {
        use windows_sys::Win32::Media::Audio::{
            PlaySoundW, SND_FILENAME, SND_NODEFAULT, SND_SYNC,
        };
        PlaySoundW(wide.as_ptr(), std::ptr::null_mut(), SND_FILENAME | SND_NODEFAULT | SND_SYNC);
    });
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn play_now(_app: &AppHandle, _cue: Cue) {}
