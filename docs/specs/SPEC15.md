# SPEC15: Yat Yat — post-update hotkey recovery & updater honesty

An increment over docs/specs/SPEC.md–SPEC14.md (authoritative elsewhere).
Field report: after a SPEC9 in-app update installed and relaunched, the
dictation hotkey silently died. Root cause is structural, not a state-machine
bug: release builds are **ad-hoc signed** (`signingIdentity: "-"`; the release
workflow carries only the minisign updater key), so every release has a
different code signature, and macOS keys the Accessibility (TCC) grant to that
signature. After the updater swaps the bundle and relaunches,
`check_accessibility()` is false for the new binary, `init_capture` defers
forever, and the hotkey never arms — while System Settings still shows Yat Yat
**checked** (a stale entry). Today the only surface that says anything is the
capture-dead banner (SPEC4 FR-F2.1) inside a settings window that a tray-only
relaunch never shows.

**What ships:** the app *notices* the dead grant at launch and leads the user
back to health (notification + settings window + the existing banner/wizard
path), and the update dialog stops being silent about what an update will do.

Out of scope, seams only: Developer ID signing/notarization (the definitive
fix — a signed app's TCC grant survives updates; tracked for a future spec),
automatic permission repair (impossible — TCC is user-only by design), any
change to the update transport/manifest machinery (SPEC9 stays as is).

---

## 1. Launch-time recovery (FR-L)

1. **Pure decision core.** A pure function (house pattern: `ready_activation`,
   `firstUnmetStep`) computes the launch recovery action from
   `(macos, onboarding_complete, accessibility_granted, accessibility_skipped,
   version_changed)` → `None | Recover { after_update: bool }`. Recover iff
   macOS ∧ onboarding complete ∧ Accessibility NOT granted ∧ the user did NOT
   defer the accessibility gate (`onboarding_skips`); `after_update` iff
   `version_changed`. Every other combination — non-mac, mid-onboarding
   (wizard already owns the screen), deliberate copy-fallback skip, healthy
   grant — is `None`.
2. **Trigger point:** once per process, at `RunEvent::Ready` after the
   SPEC14 FR-S6 `init_capture` attempt (so a healthy grant arms first and
   yields `None`). The 3-second capture watcher must NOT re-trigger it —
   one launch, one recovery action, never a nag loop.
3. **Actions on `Recover`:**
   - Post a native notification (existing `tauri-plugin-notification`, the
     pipeline's timeout pattern): `after_update: true` → body says the update
     re-keyed Accessibility and the hotkey is off until it's re-granted;
     `false` → generic "Accessibility permission is missing" wording.
     Notification delivery is **best-effort** (macOS may not have notification
     permission for an ad-hoc build) — it must never be the only signal.
   - Show the settings window (existing lazy `show_settings_window`, no deep
     link) and focus it. The capture-dead banner (SPEC4 FR-F2.1) is already
     the recovery UI there — "Fix in setup" walks the wizard's accessibility
     gate, whose stale-grant hint (remove − / re-add +) is exactly this case.
     No new webview UI.
4. **`last_run_version`** — new settings field, `#[serde(default)]` (legacy
   JSON loads as empty ⇒ first SPEC15 launch reads as `version_changed`,
   which is honest: the SPEC15 build itself arrived by update or reinstall).
   **Server-owned** like `active_model`: written by Rust at every launch
   (after the recovery decision reads the previous value), preserved across
   whole-object UI saves via `preserve_server_owned` — a UI save must never
   clobber it (R5's real-bug lineage; R27 guards it).
5. The recovery path adds **zero network** and zero new dependencies. The
   opt-out for a user who *wants* no Accessibility is the existing wizard
   skip (it lands in `onboarding_skips` and gates the decision to `None`).

## 2. Update-dialog honesty (FR-D)

All in `src/settings/UpdateDialog.tsx` behind the SPEC9 seam — no plugin API
use in app code, testids preserved.

1. **No silent background installs:** while phase is `progress`, the dialog
   is not dismissable — Escape and backdrop-click are ignored (today they
   close the modal while `downloadAndInstall` keeps running headless and
   replaces the bundle with no visible restart-pending state). Every other
   phase keeps Esc/click-outside close (SPEC9 FR-U2).
2. **Pre-restart warning:** the `ready` phase, on macOS (platform via
   `get_app_info`, threaded as a prop), renders a warning line — testid
   `update-ax-warning` — saying that after restart macOS will treat the new
   build as a new app and the dictation hotkey stays off until Accessibility
   is re-granted, and that Yat Yat will guide the re-grant on next launch
   (§1). Non-mac platforms render no warning.
3. **Restart failures surface:** `updates.restart()` rejection lands in the
   existing dismissable `update-error` phase with the reason — today the
   `void` click handler swallows it and the button just looks dead.

## 3. Tests (added: R26–R27, U23, E21a–E21b)

Mock-seam extension for e2e (SPEC9 FR-U3's `window.__yyUpdate` hook, mirrored
in `src/ipc/updates.ts`'s browser branch): a `hold: boolean` the mock's
`downloadAndInstall` awaits before finishing (deterministic mid-download
state), and a `restartError: string | null` that makes mock `restart()`
reject. Tauri branch untouched.

1. **R26 (recovery decision):** the §1.1 pure function, full matrix — the
   four `None` gates (non-mac; onboarding incomplete; accessibility skipped;
   grant healthy) and both `Recover` variants (`after_update` true/false
   driven by `version_changed`).
2. **R27 (server-owned last_run_version):** a whole-object settings save from
   the UI path preserves a previously written `last_run_version` (mirror of
   R5); legacy settings JSON without the field loads with the serde default.
3. **U23 (dialog component):** with a fake `UpdatesApi` — during `progress`,
   Escape and backdrop-click do not close the dialog; `ready` on `macos`
   renders `update-ax-warning` and on other platforms does not; a rejecting
   `restart()` transitions to `update-error` with the message.
4. **E21a (held download honesty, mock):** set `hold` → install → during
   progress, Escape and backdrop-click leave the dialog attached → release
   `hold` → `ready` shows `update-ax-warning` → restart records on
   `__yyUpdate` (E15a's flow, now with the close-lock and warning asserted).
5. **E21b (restart failure):** set `restartError` → install → ready →
   restart → `update-error` shows the reason, dialog dismissable, settings
   fully functional after (E15b's honesty bar applied to restart).
6. Test IDs continue existing sequences; function/title names embed the IDs.
   No existing test may be modified, weakened, or deleted.

## 4. Docs

1. **README** troubleshooting gains an "After an update" entry: unsigned
   alpha builds re-key Accessibility on every update; the hotkey is off until
   the permission is re-granted (remove − / re-add + in the Accessibility
   list); Yat Yat notifies and opens Settings to walk it on first launch
   after the update.
2. **docs/ARCHITECTURE.md**: a short "post-update recovery" note — the
   root-cause chain (ad-hoc signature → TCC re-key → `init_capture` defers),
   the §1 decision function and its trigger point, and the pointer that
   Developer ID signing is the eventual structural fix.
3. Existing SPEC/GOAL files untouched (immutable history).

## 5. Definition of Done

1. `npm run validate` exits 0 with complete output — R1–R27, U1–U23,
   E1–E21b, I1–I2, `VALIDATION: ALL PASSED` — printed in the transcript.
2. `npm run tauri build` (signing env per docs/RELEASING.md "Local signed
   builds") exits 0 with the `.app` path and size (< 80 MB) printed.
3. The transcript shows the §1.1 decision function is pure (no Tauri/TCC
   types in its signature) and that the Ready-path calls it exactly once
   (the watcher loop contains no recovery-action calls).
4. `grep -rn ".skip\|.only\|.todo" tests/` prints nothing (mind the trap:
   phrase around the letter-sequences in names and copy).
5. README and docs/ARCHITECTURE.md updated per §4; no changes under
   docs/specs/SPEC*.md–SPEC14.md or docs/goals/ except the new pair.
6. No new network access, no new dependencies; the local-only guarantee and
   every existing invariant (one pipeline thread, non-activating overlay,
   IPC mirror commands.rs⇄mock.ts, server-owned settings preserved,
   localhost-only enhancement, 15 effects / 12 themes) hold.
7. Anything infeasible → BLOCKERS.md; never game a check.

Manual checks after green (need a human at the machine): install the build,
grant Accessibility, then `tccutil reset Accessibility com.yatyat.app` and
relaunch — a notification appears (if notification permission allows) AND the
settings window opens showing the capture-dead banner; re-granting arms the
hotkey within ~3 s with no relaunch. Run the update dialog against the live
endpoint: mid-download Esc does nothing; the ready screen shows the
Accessibility warning.
