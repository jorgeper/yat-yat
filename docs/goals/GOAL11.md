# Launching the uninstall build with /goal

Run from this directory (`yat-yat/`). docs/specs/SPEC.md–SPEC10.md remain
authoritative except where docs/specs/SPEC11.md adds uninstall.
`scripts/deep-clean.sh` is the source of truth for what a full scrub
removes — the Rust plan must mirror it.

## Interactive (paste into a Claude Code session)

```
/goal Implement docs/specs/SPEC11.md in full (read docs/specs/SPEC.md through SPEC10.md first; scripts/deep-clean.sh defines the scrub list the uninstall plan must mirror; do NOT implement uninstall surveys/telemetry, Linux support, or removal of the Menu Bar allowance / Dock pin — the dialog names those honestly). Done when: 'npm run validate' exits 0 on macOS with its complete output — Rust tests R1–R15, frontend tests U1–U15, e2e tests E1–E16b, integration tests I1–I2, and the final line 'VALIDATION: ALL PASSED' — printed in the transcript, AND 'npm run tauri build' with the signing env exits 0 with the .app path and size (< 80 MB) printed, AND the pushed commit's test-windows CI job is green (gh run view output printed) proving the Windows target still compiles, AND README.md has the Uninstalling section and docs/ARCHITECTURE.md the uninstall paragraph per SPEC11 §5, AND 'npm run licenses' exits 0, AND 'grep -rn ".skip\|.only\|.todo" tests/' prints nothing. Constraints: the SPEC and GOAL files and this condition must not be modified; uninstall_plan must be a pure exported function covered by R15 (temp-dir table tests: sizes, keep_data exclusion keeps models removable, missing paths omitted) whose path list mirrors scripts/deep-clean.sh; execution must be best-effort in SPEC11 §2's order, must move the bundle to the Trash (never hard-delete the running app), and must reset only the app's own TCC entries via /usr/bin/tccutil; the Windows button launches the NSIS uninstaller and the deleteAppDataOnUninstall-equivalent NSIS option is enabled (BLOCKERS.md if our tauri version lacks it); the dialog must use testids uninstall-open/-dialog/-item/-keep-data/-confirm/-cancel with formatBytes pure and U15-tested; existing tests may not be weakened, stubbed, or deleted; no new network access; if something is infeasible, record it in BLOCKERS.md instead of gaming the check. Stop after 60 turns or 6 hours even if incomplete, and summarize remaining work.
```

## Notes

- Manual QA needs a SCRATCH install (the uninstall deletes models — don't
  eat your own 2 GB Parakeet download casually): install a fresh build,
  download Whisper Tiny, run the uninstall both ways, verify Trash +
  app-support + TCC + login item, then `npm run install:app` to restore
  your daily setup.
- The tccutil resets cover Accessibility and Microphone; if either fails
  the completion note must point at System Settings → Privacy & Security.
- Watch the DoD grep: avoid the letter-sequences "skip"/"only"/"todo" in
  test names and comments.
