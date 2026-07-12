# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Yat Yat is a fully-local voice-dictation app for macOS (Tauri 2: Rust host +
React/TS frontend) — a menu-bar icon for dictation plus a regular Dock
presence (Dock click opens Settings). Hotkey → record → local STT → deterministic
cleanup → paste at cursor. **Nothing may ever touch the network** except
user-initiated model downloads and the localhost-only enhancement endpoint —
that guarantee is the product; treat any new network access as a bug.

## How this repo is developed (the owner's workflow)

- **Spec-driven milestones.** Every feature milestone is a numbered delta
  spec in `docs/specs/SPEC<N>.md` with a matching launcher in
  `docs/goals/GOAL<N>.md`. New feature work = write the next SPEC/GOAL pair
  **in those folders** (never at repo root), then implement it via the
  `/goal` command whose Done-when condition mirrors the spec's Definition of
  Done. Study SPEC7/GOAL7 and SPEC8/GOAL8 for the house style: terse FR-x
  requirement sections, explicit test IDs, a DoD provable from the
  transcript, and a "record it in BLOCKERS.md instead of gaming the check"
  escape hatch.
- **Existing SPEC/GOAL files are immutable history.** Never edit them, even
  when later work supersedes them (note divergences in docs/ARCHITECTURE.md
  instead — see the vu-needle→heartbeat note for the pattern).
- **Evidence before claims.** Success means the actual command output —
  `VALIDATION: ALL PASSED`, build paths, sizes — shown in the conversation,
  not asserted.
- **Test IDs are continuous across specs**: Rust `R<n>`, frontend unit
  `U<n>`, e2e `E<n>[letter]`, integration `I<n>`. New tests take the next
  free numbers; test function/title names embed the ID (e.g.
  `r13_fails_open_on_missing_reads`). Existing tests may never be weakened,
  stubbed, or deleted to make something pass.
- The owner asks to "commit and push" explicitly — don't commit
  mid-implementation unless a goal's DoD requires it (e.g. `git mv` history
  checks). The sibling repo `~/src/marky-mark` is the reference for release/
  infra patterns; when in doubt about process, look there.

## Commands

`cargo` is not on the default PATH — prefix Rust commands with
`PATH="$HOME/.cargo/bin:$PATH"` (scripts/validate.mjs does this itself).

```bash
npm install                 # once
npm run tauri dev           # run the app in dev mode
npm run tauri build         # release .app + .dmg under src-tauri/target/release/bundle/
                            # NOTE: createUpdaterArtifacts means this needs the minisign key env
                            # (see docs/RELEASING.md "Local signed builds") or it exits 1 at the end
                            # — the .app/.dmg are still produced before the failure.
npm run validate            # THE gate: R → U → build → E → I suites, ends "VALIDATION: ALL PASSED"

npm run test:unit           # vitest (frontend units)
npx vitest run tests/unit/focus-prompt.test.tsx        # one unit file
npm run test:e2e            # playwright (requires `npm run build` first — it serves dist/)
npx playwright test tests/e2e/focusguard.spec.ts       # one e2e file
npm run test:rust           # cargo test --release
cargo test r13 --manifest-path src-tauri/Cargo.toml    # tests matching a substring (debug = faster)

npm run install:app         # kill running app, install bundle to /Applications, reset+re-ask Accessibility
npm run clean:app           # full uninstall incl. permissions/models (true first-run)
npm run release:prepare -- 0.1.0-alpha.2   # ONLY way to move the version (3 files + locks, in lock-step)
npm run licenses            # regenerate THIRD-PARTY-NOTICES.md; fails on non-permissive licenses
node scripts/gen-icons.mjs  # regenerate tray/app icons (deterministic)
node scripts/gen-cues.mjs   # regenerate sound-cue WAVs (deterministic)
```

Releases: tag push `v<version>` triggers `.github/workflows/release.yml`
(validate gate → dmg → **draft** GitHub Release); a human publishes. Full
operator flow in `docs/RELEASING.md`. Strict semver, pre-release ids
(`-alpha.N`) are never stripped.

## Architecture (the parts that span files)

Full map with measured perf numbers: `docs/ARCHITECTURE.md`. The essentials:

- **One pipeline thread** (`src-tauri/src/pipeline.rs`) owns the
  Idle → Recording → AwaitFocusConfirm state machine; every lifecycle event
  (hotkey, Esc, tray, timeouts, focus-prompt resolution) arrives as a
  message on its channel, so nothing races. It must never block on the user
  — e.g. the focus-guard prompt parks text in `AppState::pending_paste` and
  returns to the loop.
- **The overlay never takes focus** (non-activating NSPanel via
  tauri-nspanel) — that invariant is why "frontmost app at hotkey time ==
  paste target" holds and why prompt buttons work without stealing focus.
  Paste itself must run on the main thread (macOS).
- **Platform boundary**: every macOS-specific mechanism lives in one module
  with a non-mac stub (`overlay.rs::platform`, `paste.rs`, `focus.rs`,
  `sounds.rs`, `tray_probe.rs`) — the Windows port is a porting task, not a
  rewrite. Keep new platform code behind the same seam.
- **IPC mirror**: `src-tauri/src/commands.rs` (Tauri commands) is mirrored
  event-for-event by `src/ipc/mock.ts` (browser shim). Playwright drives the
  real UI through the mock — **any new command/event must be added to both**
  plus `src/ipc/api.ts`/`types.ts`, or e2e can't cover it.
- **Settings**: whole-object writes from the UI; every field
  `#[serde(default)]`-safe (legacy JSON must load); server-owned fields
  (`active_model`) survive UI writes via `preserve_server_owned` — never let
  a UI save clobber them (that was a real field bug, R5 guards it).
- **Cleanup** (`cleanup.rs::clean`) is a pure function and the product's
  heart — fillers → artifacts → repeat-collapse → personal dictionary, in
  that order. Enhancement (localhost LLM) runs after and is enforced
  localhost-only in code (R8).
- **Appearance**: effects draw ONLY with `frame.colors` from the active
  theme's CSS variables — U8 fails any color literal in an effect module.
  Exactly 15 effects / 12 built-in themes are asserted by tests; swapping
  one means keeping the counts.
- **Onboarding** advances only on *verified system state* (polled), never on
  clicks; gate logic is the pure `firstUnmetStep`. macOS re-keys the
  Accessibility grant on every ad-hoc-signed rebuild — `install:app` resets
  it deliberately; the wizard and banners are the recovery path.

## Traps

- The DoD grep `grep -rn ".skip\|.only\|.todo" tests/` matches those words
  with ANY preceding character — strings like "Copy only" or "GPL-3.0-only"
  in tests fail it. Phrase around it ("copy-fallback", bare "GPL-3.0").
- `npm run tauri build` and `cargo` invocations contend on the same target
  dir — don't run them concurrently with `npm run validate`, and if you kill
  a validate run, check for orphaned `rustc` processes (they starve
  Playwright into bogus timeouts).
- Playwright e2e runs against `dist/` (vite preview) — rebuild
  (`npm run build`) before `npx playwright test` or you test stale UI.
- Models live in `~/Library/Application Support/com.yatyat.app/` and survive
  reinstalls; the validate harness caches Whisper tiny in
  `~/.cache/yatyat-validate/` (its only sanctioned network).
