# SPEC11: Yat Yat — Uninstall (leave nothing behind)

An increment over docs/specs/SPEC.md–SPEC10.md (authoritative elsewhere).
Deleting the .app leaves everything the app ever created: settings, history,
**the downloaded models (up to ~2 GB)**, TCC grants, preferences, WebKit
storage, caches, saved state, and the login item. `scripts/deep-clean.sh`
scrubs all of it for developers; end users get nothing. This spec ships a
user-facing **Uninstall Yat Yat…** that removes everything removable,
itemized and consented, then puts the app itself in the Trash.

Out of scope: uninstall telemetry/surveys (never), removing the two things
macOS offers no API for (the Menu Bar allowance toggle and a user's Dock
pin — the dialog names them honestly), Linux.

## 1. The uninstall plan (FR-P) — pure and testable

1. **`uninstall_plan(data_dir, home, keep_data) -> Vec<PlanItem>`** in a new
   `src-tauri/src/uninstall.rs`: a pure function returning the exact items
   an uninstall would remove — `{ path, kind, bytes }` where `kind` ∈
   `app_data | models | preferences | webkit | caches | saved_state`.
   With `keep_data = true`, `app_data` (settings/history) is excluded but
   `models` still goes (they are re-downloadable and huge). Paths mirror
   `scripts/deep-clean.sh` (the doc comment cross-references it; drift
   between the two is a bug). Sizes are computed by directory walk;
   missing paths are simply omitted.
2. **R15** table-tests the plan against temp dirs: items found with correct
   byte counts, `keep_data` exclusion, missing-path omission, and that
   models under `<data_dir>/models` are classified `models`, not
   `app_data`.
3. Commands: `get_uninstall_plan(keep_data) -> Vec<PlanItem>` (drives the
   dialog) and `uninstall_app(keep_data)` (executes §2). Both mirrored in
   the e2e mock.

## 2. Execution (FR-X)

On confirm, in order, continuing past individual failures (best-effort,
each failure logged — an uninstall must never strand the user half-way):

1. Stop dictation if active; disable launch-at-login via the autostart
   plugin.
2. Delete every plan item.
3. Reset the app's own TCC grants: spawn `/usr/bin/tccutil reset
   Accessibility com.yatyat.app` and `… reset Microphone com.yatyat.app`
   (needs no privileges for the caller's own entries; if it fails, the
   dialog's completion note tells the user the System Settings path).
4. Move the app bundle itself to the **Trash** (NSWorkspace recycle —
   never a hard delete of the running bundle), then exit(0).
5. **Windows**: the in-app button instead launches the NSIS uninstaller
   (`uninstall.exe` beside the installed binary) and exits — Add/Remove
   Programs remains the canonical path. `deleteAppDataOnUninstall` (or the
   current tauri-NSIS equivalent) is enabled in tauri.conf so the NSIS
   uninstaller also clears app data; if no such option exists in our tauri
   version, record it in BLOCKERS.md and scope the Windows button to
   launching the uninstaller.

## 3. UI (FR-U)

1. Settings → General, bottom, beneath Clear history: a **Danger zone**
   row — "Uninstall Yat Yat… Removes the app and everything it stored."
   with an `uninstall-open` button.
2. Clicking opens a modal (`uninstall-dialog`, reuses the `.modal` styles):
   the itemized plan with human-readable sizes (`uninstall-item` rows, e.g.
   "Models — 1.9 GB"), a **keep my settings & history** checkbox
   (`uninstall-keep-data`, re-fetches the plan when toggled), the honest
   un-scrubbables note (Menu Bar allowance, Dock pin), and
   `uninstall-confirm` / `uninstall-cancel` buttons. Confirm shows a brief
   "Removing…" state, then the app quits (macOS) or hands off to the NSIS
   uninstaller (Windows).
3. `formatBytes` (pure, `src/lib/format.ts`) renders sizes — **U15** covers
   it (B/KB/MB/GB boundaries, 0, rounding).

## 4. Tests (added: R15, U15, E16a–E16b)

- **R15**: the plan function (§1.2).
- **U15**: `formatBytes`.
- **E16a**: General shows the Danger-zone row; opening lists the mocked
  plan items with sizes; toggling keep-data refetches and drops the
  app-data row; confirm calls `uninstall_app` with the flag.
- **E16b**: cancel closes the dialog with NO `uninstall_app` call and
  settings remain fully functional.
- No existing test may be modified, weakened, or deleted.

## 5. Docs

README: an **Uninstalling** section (the in-app path, what's removed, the
two manual leftovers, Windows = Add/Remove Programs); the "Full reset"
dev section points at it. docs/ARCHITECTURE.md: one paragraph — plan
purity, best-effort execution order, trash-not-delete, deep-clean.sh
parity.

## 6. Definition of Done

1. `npm run validate` exits 0 with complete output — R1–R15, U1–U15,
   E1–E16b, I1–I2, `VALIDATION: ALL PASSED` — printed in the transcript.
2. `npm run tauri build` (signed env) exits 0; app size still < 80 MB.
3. `cargo check --target x86_64-pc-windows-msvc` clean via CI's
   test-windows job on the pushed commit (or cargo-xwin locally).
4. README + ARCHITECTURE updated per §5; `npm run licenses` green (the
   trash crate, if one is added, must pass the allowlist);
   `grep -rn ".skip\|.only\|.todo" tests/` prints nothing.
5. Anything infeasible → BLOCKERS.md; never game a check.

Manual checks after green (a scratch install, not your daily one): run the
in-app uninstall with models downloaded — app lands in Trash, app-support
gone, `tccutil` grants cleared (System Settings shows no Yat Yat rows),
login item gone; keep-data run preserves settings.json/history.json;
Windows: the button opens the NSIS uninstaller and Add/Remove works.
