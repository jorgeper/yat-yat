//! Output stage (SPEC FR-3): clipboard write + synthesized paste keystroke,
//! then clipboard restore. Layout-independent key codes (adapted from Handy,
//! MIT): macOS kVK_ANSI_V (9) + Meta, Windows VK_V (0x56) + Ctrl.

use crate::settings::OutputMethod;
use anyhow::{Context, Result};
use enigo::{Direction, Enigo, Key, Keyboard};
use std::time::Duration;

/// Milliseconds to let the clipboard settle before the paste keystroke
/// (SPEC14 FR-D2: arboard's NSPasteboard write is synchronous — 20 ms is
/// margin, not a wait for the write itself).
const PRE_PASTE_DELAY_MS: u64 = 20;
/// Milliseconds to hold the paste modifier after the V click (SPEC14 FR-D2:
/// the target registered ⌘V at the V key-down; this hold only covers apps
/// that sample modifier state late).
const PASTE_MODIFIER_HOLD_MS: u64 = 20;
/// SPEC FR-3.1: restore the previous clipboard ~300 ms after the paste.
const RESTORE_DELAY_MS: u64 = 300;

pub struct Paster {
    enigo: Option<Enigo>,
}

/// Enigo settings that NEVER auto-open the macOS accessibility prompt.
/// (`enigo::Settings::default()` prompts — with a retry loop that means
/// dialog spam. Prompting is the onboarding wizard's job, on a user click.)
fn quiet_settings() -> enigo::Settings {
    enigo::Settings {
        open_prompt_to_get_permissions: false,
        ..Default::default()
    }
}

impl Paster {
    /// On macOS, Enigo construction fails without Accessibility permission —
    /// callers treat `enigo: None` as "degrade to clipboard-only" (FR-3.3).
    /// Construction is silent: no system dialogs, ever.
    pub fn new() -> Self {
        let enigo = Enigo::new(&quiet_settings())
            .map_err(|e| log::debug!("enigo init failed (accessibility not granted?): {e}"))
            .ok();
        Self { enigo }
    }

    pub fn can_paste(&self) -> bool {
        self.enigo.is_some()
    }

    /// Retry Enigo init (after the user grants Accessibility mid-session).
    pub fn reinit(&mut self) -> bool {
        if self.enigo.is_none() {
            self.enigo = Enigo::new(&quiet_settings()).ok();
        }
        self.can_paste()
    }

    /// Deliver `text` per the configured output method. Returns true if a paste
    /// keystroke was sent (false = clipboard-only, either by setting or fallback).
    /// Must run on the main thread on macOS.
    pub fn deliver(&mut self, text: &str, method: OutputMethod) -> Result<bool> {
        let mut clipboard = arboard::Clipboard::new().context("opening clipboard")?;

        if method == OutputMethod::ClipboardOnly || self.enigo.is_none() {
            clipboard.set_text(text).context("writing clipboard")?;
            return Ok(false);
        }

        // Save -> write -> paste -> schedule the restore. The restore MUST
        // NOT block this (main) thread: the paste target can be Yat Yat's
        // own webview (the onboarding try-box), whose ⌘V sits queued on the
        // main thread until deliver returns — a blocking restore ran first
        // and handed it the OLD clipboard.
        let saved = clipboard.get_text().ok();
        clipboard.set_text(text).context("writing clipboard")?;
        std::thread::sleep(Duration::from_millis(PRE_PASTE_DELAY_MS));

        let enigo = self.enigo.as_mut().unwrap();
        send_paste_keystroke(enigo)?;

        if let Some(saved) = saved {
            restore_clipboard_later(text.to_string(), saved, RESTORE_DELAY_MS);
        }
        Ok(true)
    }

}

/// Restore `saved` to the clipboard after `delay_ms`, WITHOUT blocking the
/// caller (SPEC FR-3.1). The restore is conditional: it only happens if the
/// clipboard still holds `pasted` — if anything else wrote the clipboard in
/// the meantime (the user copied something, a newer dictation delivered),
/// it is left alone.
pub fn restore_clipboard_later(pasted: String, saved: String, delay_ms: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(delay_ms));
        let Ok(mut clipboard) = arboard::Clipboard::new() else {
            return;
        };
        if clipboard.get_text().ok().as_deref() == Some(pasted.as_str()) {
            let _ = clipboard.set_text(saved);
        }
    });
}

/// Plain clipboard write (Copy Last Transcription, history clicks, retry) —
/// needs no Enigo/Accessibility.
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new().context("opening clipboard")?;
    clipboard.set_text(text).context("writing clipboard")?;
    Ok(())
}

fn send_paste_keystroke(enigo: &mut Enigo) -> Result<()> {
    #[cfg(target_os = "macos")]
    let (modifier, v_key) = (Key::Meta, Key::Other(9)); // kVK_ANSI_V — layout-independent
    #[cfg(target_os = "windows")]
    let (modifier, v_key) = (Key::Control, Key::Other(0x56)); // VK_V
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let (modifier, v_key) = (Key::Control, Key::Unicode('v'));

    enigo
        .key(modifier, Direction::Press)
        .context("pressing paste modifier")?;
    enigo.key(v_key, Direction::Click).context("pressing V")?;
    std::thread::sleep(Duration::from_millis(PASTE_MODIFIER_HOLD_MS));
    enigo
        .key(modifier, Direction::Release)
        .context("releasing paste modifier")?;
    Ok(())
}

#[cfg(test)]
mod restore_tests {
    use super::*;

    // R20: the deferred clipboard restore must not block the caller (that
    // block is exactly what made pasting into Yat Yat's own windows deliver
    // the OLD clipboard) and must not clobber a clipboard that changed
    // underneath it.
    #[test]
    fn r20_restore_is_deferred_and_conditional() {
        let mut cb = arboard::Clipboard::new().unwrap();
        let original = cb.get_text().ok(); // preserve the dev machine's clipboard

        // Still holding the pasted text after the delay -> restored.
        cb.set_text("r20-pasted").unwrap();
        let t0 = std::time::Instant::now();
        restore_clipboard_later("r20-pasted".into(), "r20-saved".into(), 60);
        assert!(
            t0.elapsed() < std::time::Duration::from_millis(50),
            "restore_clipboard_later must return immediately"
        );
        assert_eq!(cb.get_text().unwrap(), "r20-pasted", "no early restore");
        std::thread::sleep(std::time::Duration::from_millis(250));
        assert_eq!(cb.get_text().unwrap(), "r20-saved", "restored after delay");

        // Clipboard changed in the meantime -> left alone.
        cb.set_text("r20-pasted").unwrap();
        restore_clipboard_later("r20-pasted".into(), "r20-saved".into(), 60);
        cb.set_text("r20-user-copied").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(250));
        assert_eq!(cb.get_text().unwrap(), "r20-user-copied", "newer write wins");

        if let Some(original) = original {
            let _ = cb.set_text(original);
        }
    }
}
