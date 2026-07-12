# Launching the dance-egg build with /goal

Run from this directory (`yat-yat/`). docs/specs/SPEC.md–SPEC11.md remain
authoritative except where docs/specs/SPEC12.md supersedes the wiggle
egg's trigger ("yat yat" → "dance"), adds the tray wiggle, and adds the
Easter-eggs setting.

## Interactive (paste into a Claude Code session)

```
/goal Implement docs/specs/SPEC12.md in full (read docs/specs/SPEC11.md §5 first for the eggs it amends; do NOT change the Konami/Yat95 egg or the sleepy waveform beyond gating them behind the new setting, and add no in-app documentation or hints for any egg — macOS-first, fully local, Windows compile deferred to the next release cut's CI). Done when: 'npm run validate' exits 0 on macOS with its complete output — Rust tests R1–R17, frontend tests U1–U18, e2e tests E1–E18b, integration tests I1–I2, and the final line 'VALIDATION: ALL PASSED' — printed in the transcript, AND 'npm run tauri build' with the signing env exits 0 with the .app path and size (< 80 MB) printed, AND docs/EASTER-EGGS.md documents the dance trigger (pill + menu-bar wiggle) and the settings switch with docs/ARCHITECTURE.md carrying the SPEC11 §5.1 divergence note, AND 'node scripts/gen-icons.mjs' exits 0, AND 'grep -rn ".skip\|.only\|.todo" tests/' prints nothing. Constraints: the SPEC and GOAL files and this condition must not be modified; eggs stay COSMETIC ONLY — nothing may touch recording, transcription, cleanup, or paste behavior — with the pure detector danceMatches in src/lib/eggs.ts covered by U18 and all motion respecting prefers-reduced-motion (reduced motion suppresses the wiggle_tray call at the source); wiggle_tray must be mirrored across commands.rs, src/ipc/api.ts, types.ts, and mock.ts (the mock records calls for E17a/E18a), must restore the current TrayState icon in template mode after the frames, must ignore re-entrant calls, and must no-op server-side when easter_eggs is false; easter_eggs defaults true via the existing serde(default) pattern so legacy JSON loads ON (R17), with the eggs-toggle checkbox in General above the Danger zone; wiggle frames come deterministically from scripts/gen-icons.mjs; the ONLY sanctioned existing-test amendments are U16 losing its yat-yat block (replaced by U18) and E17a switching its trigger to a dance stream plus asserting one recorded wiggle_tray call — nothing else may be modified, weakened, or deleted; no new network access; if something is infeasible (e.g. macOS throttles rapid tray set_icon swaps — then ship the single-tilted-frame fallback), record it in BLOCKERS.md instead of gaming the check. Stop after 40 turns or 4 hours even if incomplete, and summarize remaining work.
```

## Notes

- The trigger change is evidence-driven: Whisper tiny transcribed spoken
  "yat yat" as "that you add" / "that yet" / "you're at" across four
  `say` voices (reproduce with the release CLI's `transcribe --raw` on
  synthesized WAVs); "dance" came back exact in 3/4 voices in sentence
  context. Keep the reproduction handy for QA.
- Egg QA: dictate a sentence containing "dance" with live transcription
  on and watch BOTH the pill and the menu-bar icon; the menu-bar glyph
  wiggles inside macOS 26's orange privacy capsule (template mode is why
  it renders there at all — see the SPEC7 FR-T1 divergence note).
- The tray frames must go through scripts/gen-icons.mjs (deterministic),
  not hand-exported PNGs.
- Watch the DoD grep: avoid the letter-sequences "skip"/"only"/"todo" in
  test names and comments.
