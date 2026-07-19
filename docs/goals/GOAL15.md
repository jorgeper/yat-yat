# Launching the post-update recovery build with /goal

Run from this directory (`yat-yat/`). docs/specs/SPEC.md–SPEC14.md remain
authoritative; docs/specs/SPEC15.md is a small recovery-UX increment born
from a field failure: an in-app update relaunched into a dead hotkey,
because ad-hoc-signed builds re-key the macOS Accessibility grant on every
update and nothing told the user. SPEC15 makes the app notice the dead
grant at launch (pure decision fn → best-effort notification + settings
window with the existing banner), adds a server-owned `last_run_version`
so the message can honestly say "the update did this", and makes the
update dialog honest (no dismiss mid-download, a pre-restart Accessibility
warning, restart errors surfaced). Signing is explicitly out of scope.

The command deliberately does NOT restate the FRs — /goal conditions are
capped at 4000 characters (see AGENTS.md), so the SPEC carries the detail
and the condition binds "implement it in full" plus the transcript-provable
checks.

## Interactive (paste into a Claude Code session)

```
/goal Implement docs/specs/SPEC15.md in full — post-update Accessibility recovery + update-dialog honesty (read src-tauri/src/lib.rs, settings.rs, hotkey.rs, src/settings/UpdateDialog.tsx, SettingsApp.tsx, src/ipc/updates.ts, mock.ts first). The SPEC is authoritative for every FR-L/FR-D requirement and every invariant it names (one pipeline thread, non-activating overlay, IPC mirror commands.rs⇄mock.ts, server-owned settings preserved via preserve_server_owned, localhost-only enhancement, exactly 15 effects / 12 themes) — macOS-first, fully local, ZERO new network and ZERO new dependencies. Done when: 'npm run validate' exits 0 on macOS with its complete output — Rust tests R1–R27, frontend tests U1–U23, e2e tests E1–E21b, integration tests I1–I2, and the final line 'VALIDATION: ALL PASSED' — printed in the transcript, AND 'npm run tauri build' with the signing env exits 0 with the .app path and size (< 80 MB) printed, AND the transcript shows the launch-recovery decision function is pure (no Tauri/TCC types in its signature; R26 drives the full matrix) and is acted on exactly once per process at RunEvent::Ready after the init_capture attempt (the 3 s capture watcher contains no recovery-action calls — show the watcher loop), AND the new settings field last_run_version is #[serde(default)] and listed in preserve_server_owned (R27 proves a UI whole-object save cannot clobber it and legacy JSON loads), AND docs/ARCHITECTURE.md carries the post-update-recovery note and README the "After an update" troubleshooting entry, AND 'grep -rn ".skip\|.only\|.todo" tests/' prints nothing. Constraints: the SPEC and GOAL files and this condition must not be modified; every SPEC15 FR is implemented as written — where this condition is silent, the SPEC decides; new tests R26–R27, U23, E21a–E21b match the SPEC §3 definitions and embed their IDs in test names; NO existing test is amended, weakened, or deleted; the mock-seam additions (hold, restartError on window.__yyUpdate) touch only the browser branch of src/ipc/updates.ts and mock plumbing — the Tauri branch and SPEC9 transport/manifest machinery stay byte-identical in behavior; the notification is best-effort and the settings window + banner must appear regardless; the recovery must NOT fire mid-onboarding, on non-mac, when Accessibility is healthy, or when the user deferred the accessibility gate in onboarding_skips; if something is infeasible, record it in BLOCKERS.md instead of gaming the check; after validate is first green, run one adversarial review pass over the ENTIRE diff hunting the regressions SPEC15 risks — a nag loop (recovery re-firing from the watcher or on every poll), the settings window stealing focus on healthy tray-only launches, last_run_version writes racing or clobbering user settings fields, the progress-phase close-lock trapping users on a hung download error path (errors must still transition to dismissable update-error), and the ready-phase warning leaking onto non-mac — fix findings and re-run validate before declaring done. Stop after 40 turns or 4 hours even if incomplete, and summarize remaining work.
```

## Unattended one-liner

Same prompt via `claude -p "/goal …"` works, but the interactive form is
recommended — the decisive checks are manual: `tccutil reset Accessibility
com.yatyat.app` + relaunch must produce the notification and the settings
window with the banner, and re-granting must arm the hotkey within ~3 s.

## Notes

- **The decision gate is the risky FR.** Get the matrix exactly right or
  the app nags: mid-onboarding, deferred-accessibility (copy-fallback
  users), and non-mac must all be `None`, and the action fires once per
  process — never from the 3 s watcher.
- **Notifications may silently not appear** (ad-hoc builds may lack
  notification permission) — that's why the settings window + banner is
  the primary signal and the notification is best-effort by spec.
- **last_run_version is server-owned.** It rides preserve_server_owned
  exactly like active_model; R5's lineage is a real UI-save clobber bug.
  Write it after the recovery decision reads the previous value, or
  "after_update" is always false.
- **Close-lock only during `progress`.** A download that *errors* must
  still land in dismissable `update-error` — locking the user into a
  dead modal is worse than the silent install it replaces.
- Watch the DoD grep: avoid the letter-sequences "skip"/"only"/"todo" in
  test names and comments (write "deferred", not "skipped", in copy).
