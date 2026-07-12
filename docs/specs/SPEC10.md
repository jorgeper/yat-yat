# SPEC10: Yat Yat — the Windows port (alpha)

An increment over docs/specs/SPEC.md–SPEC9.md (authoritative elsewhere).
SPEC §1 required Windows-portability from day one and every macOS mechanism
sits behind a seam (docs/ARCHITECTURE.md table) — this spec cashes that in:
a building, unit-tested, CI-packaged **Windows x64 alpha** with the updater
wired, plus the two remaining platform stubs implemented. Behavioral polish
on real Windows hardware is explicitly a manual-QA phase, not a DoD item —
headless proof covers what it can, the checklist covers the rest, and
anything found broken there becomes the next spec.

Out of scope: Windows code signing (SmartScreen "Run anyway" is the
documented experience), ARM64 Windows, MSI (NSIS only), the Microsoft
Store, and per-monitor-DPI fine-tuning beyond what Tauri gives for free.

---

## 1. Build & compile (FR-B)

1. The workspace compiles for `x86_64-pc-windows-msvc` with no
   `cfg(windows)` gaps. Whisper acceleration: the Cargo target table gives
   Windows `transcribe-rs` with `whisper-vulkan` (CPU fallback is automatic)
   in place of macOS's `whisper-metal`; ONNX/Parakeet rides ort's stock
   Windows binaries.
2. Existing platform seams keep their contracts — non-mac branches already
   exist for overlay creation, paste keystroke (Ctrl+V), tray probe
   (always visible), Dock/app-menu code (cfg'd out), and onboarding's
   per-platform step list (no Accessibility or Menu-Bar gates on Windows;
   `capture_ready` is true once the handy-keys hook starts — no permission
   exists to gate on).
3. Local dev fallback documented: `cargo-xwin` cross-compile from macOS for
   compile-checking without a Windows machine (native CI remains the truth).

## 2. Platform stubs become real (FR-S)

1. **Focus guard** (`focus.rs`): Windows `frontmost_app()` via
   `GetForegroundWindow` → `GetWindowThreadProcessId` →
   `QueryFullProcessImageNameW`. The comparison key is the lowercased
   executable path (Windows' analog of a bundle id); `display_name()` is
   the file stem ("WINWORD" → "WINWORD" is acceptable for the alpha). All
   failure paths stay `None` — the guard still fails open. Key
   normalization is a pure exported function, **R14**-table-tested on all
   platforms.
2. **Sound cues** (`sounds.rs`): `PlaySoundW` with
   `SND_ASYNC | SND_FILENAME | SND_NODEFAULT` on the resolved resource
   path — still fire-and-forget, still nothing on the dictation timing
   path. Use the `windows` (or `windows-sys`) crate, Windows-target-only
   dependency.
3. The Reopen-style "app icon clicked" affordance: on Windows the tray
   already covers Settings access; taskbar behavior needs no macOS-style
   nudge (no LaunchServices hidden state exists). No new code beyond
   verifying the settings window shows/hides sanely.

## 3. CI & packaging (FR-C)

1. `release.yml` gains **build-windows** (windows-latest, needs test):
   `npm ci`, `npm run tauri build -- --bundles nsis` with the signing env,
   uploading `*-setup.exe` + `*-setup.exe.sig` (tauri's Windows updater
   artifact is the signed installer itself). Marky-mark's job is the
   template.
2. A **test-windows** job (windows-latest, needs nothing): `cargo test`
   + `npx vitest run` + the frontend build — the cross-platform half of
   the suite proven on the target OS. The full validate gate (Playwright
   e2e + I-suite) remains the macOS test job's responsibility for now.
3. The **release** job folds Windows in: versioned asset names
   (`Yat.Yat_<ver>_x64-setup.exe[.sig]`), checksums, asset guard grows to
   exactly one dmg + one setup.exe + both updater artifacts + latest.json
   + SHA256SUMS.txt, and the notes header gains the Windows row
   (SmartScreen → More info → Run anyway).
4. **`scripts/updater-manifest.mjs`** re-gains the `windows-x86_64`
   platform (URL + embedded signature, exactly marky-mark's shape); the
   CLI takes the optional `--win-url`/`--win-sig-file` pair and the
   release job passes them.
5. `tauri.conf.json`: any Windows bundle config needed for NSIS (product
   name collisions, icons — `icon.ico` already exists). CSP untouched.

## 4. Docs (FR-D)

README: Windows row in the Download table + a "First launch on Windows"
note (SmartScreen), and the platform line in the intro updated from
"(Windows-portable)" to shipping alpha. docs/RELEASING.md: the new
artifacts and smoke-test items. docs/ARCHITECTURE.md: the platform table
rows flip from "port point" to "implemented"; note the Vulkan/CPU whisper
path and that Windows mic consent is the OS-level toggle (no in-app gate).

## 5. Sanctioned test changes

1. **U14** updates: `windows-x86_64` becomes a VALID platform (assert both
   platform keys compose; the unknown-platform throw case moves to a
   genuinely unknown id like `linux-x86_64`). This is the only existing
   test whose assertions may change, and only in this direction.
2. New **R14** (focus key normalization, runs everywhere) and the
   manifest's Windows cases folded into U14.
3. Nothing else may be modified, weakened, or deleted.

## 6. Definition of Done

1. `npm run validate` (macOS) exits 0 with complete output — R1–R14,
   U1–U14, E1–E15b, I1–I2, `VALIDATION: ALL PASSED` — printed in the
   transcript.
2. A pushed branch/tag shows **test-windows** and **build-windows** green
   in GitHub Actions (`gh run view` output printed), with the NSIS
   installer + `.sig` among the artifacts.
3. `scripts/updater-manifest.mjs` with both platforms emits a schema-valid
   `latest.json` (shown); U14 covers both keys.
4. `cargo check --target x86_64-pc-windows-msvc` (via cargo-xwin or CI)
   exits 0 — no cfg gaps.
5. README, RELEASING, ARCHITECTURE updated per §4; `npm run licenses`
   green (windows crate added); `grep -rn ".skip\|.only\|.todo" tests/`
   prints nothing.
6. Anything infeasible → BLOCKERS.md; never game a check.

## 7. Manual QA checklist (real Windows hardware — the port isn't "done
   done" until these pass, but they gate the next spec, not this DoD)

- Onboarding: mic consent appears (or the Settings→Privacy pointer works),
  model download, Try It dictation.
- Hotkey: bare Right-Ctrl works system-wide; Esc cancels; hold mode.
- Overlay: appears bottom-center, never steals focus, correct on mixed-DPI
  multi-monitor.
- Paste lands in Notepad, Word, a browser textarea, and Windows Terminal;
  clipboard restore works.
- Focus guard: start dictation in Notepad, Alt-Tab to another app, stop —
  prompt names both apps; Paste/Copy-only/timeout all behave.
- Sound cues audible; tray menu complete; Check for Updates… full hop on
  the NEXT published release.
- Whisper Vulkan vs CPU sanity (a 15 s utterance transcribes < ~3 s on a
  midrange GPU box; CPU fallback merely slower, never broken).
