# Launching the uninstall + easter-eggs build with /goal

Run from this directory (`yat-yat/`). docs/specs/SPEC.md–SPEC10.md remain
authoritative except where docs/specs/SPEC11.md adds uninstall, the
sound-cues default flip, and the three easter eggs.
`scripts/deep-clean.sh` is the source of truth for what a full scrub
removes — the Rust plan must mirror it.

## Interactive (paste into a Claude Code session)

```
/goal Implement docs/specs/SPEC11.md in full (read docs/specs/SPEC.md through SPEC10.md first; scripts/deep-clean.sh defines the scrub list the uninstall plan must mirror; do NOT implement uninstall surveys/telemetry, Linux support, removal of the Menu Bar allowance / Dock pin, or any in-app documentation or hints for the easter eggs — this is a macOS-first pass, fully local, with the Windows compile deferred to the next release cut's CI). Done when: 'npm run validate' exits 0 on macOS with its complete output — Rust tests R1–R15, frontend tests U1–U17, e2e tests E1–E17c, integration tests I1–I2, and the final line 'VALIDATION: ALL PASSED' — printed in the transcript, AND 'npm run tauri build' with the signing env exits 0 with the .app path and size (< 80 MB) printed, AND README.md has the Uninstalling section, the sound-cues default flip, and a one-line teaser linking to a new docs/EASTER-EGGS.md that documents all three eggs and their triggers, with docs/ARCHITECTURE.md updated per SPEC11 §7, AND 'npm run licenses' exits 0, AND 'grep -rn ".skip\|.only\|.todo" tests/' prints nothing. Constraints: the SPEC and GOAL files and this condition must not be modified; uninstall_plan must be a pure exported function covered by R15 (temp-dir table tests: sizes, keep_data exclusion keeps models removable, missing paths omitted) whose path list mirrors scripts/deep-clean.sh; execution must be best-effort in SPEC11 §2's order, must move the bundle to the Trash (never hard-delete the running app), and must reset only the app's own TCC entries via /usr/bin/tccutil; the Windows button launches the NSIS uninstaller with the deleteAppDataOnUninstall-equivalent enabled (BLOCKERS.md if unavailable); the uninstall dialog uses testids uninstall-open/-dialog/-item/-keep-data/-confirm/-cancel with formatBytes pure and U15-tested; every easter egg is COSMETIC ONLY — none may touch recording, transcription, cleanup, or paste behavior — with pure helpers in src/lib/eggs.ts (yatYatMatches, konamiProgress, SleepTracker with injected clock) covered by U16/U17 and all motion respecting prefers-reduced-motion; the secret Yat95 theme lives outside the built-in registry (id secret:yat95) so U9's exactly-12 assertion and the Appearance picker's 12 swatches are untouched; the ONLY sanctioned existing-test amendments are r13_legacy_json_defaults_new_fields and E14b flipping their sound_cues-default assertions to ON (SPEC11 §4) — nothing else may be modified, weakened, or deleted; no new network access; if something is infeasible, record it in BLOCKERS.md instead of gaming the check. Stop after 60 turns or 6 hours even if incomplete, and summarize remaining work.
```

## Notes

- Manual QA needs a SCRATCH install (the uninstall deletes models — don't
  eat your own 2 GB Parakeet download casually): install a fresh build,
  download Whisper Tiny, run the uninstall both ways, verify Trash +
  app-support + TCC + login item, then `npm run install:app` to restore
  your daily setup.
- The tccutil resets cover Accessibility and Microphone; if either fails
  the completion note must point at System Settings → Privacy & Security.
- Egg QA is the fun pass: say "yat yat" mid-dictation and watch the pill
  wiggle before the collapse eats it; Konami in Settings for Yat95; leave
  the pill open silently for 20 s and watch it fall asleep (💤), then
  clap.
- E17c uses Playwright's clock API for the 20 s silence — no real waiting,
  no shrunken-timeout test hooks in product code.
- Watch the DoD grep: avoid the letter-sequences "skip"/"only"/"todo" in
  test names and comments.
