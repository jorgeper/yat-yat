# Launching the focus-guard / dictionary / cues / red-dot build with /goal

Run from this directory (`yat-yat/`). SPEC.md through SPEC6.md remain
authoritative except where SPEC7.md adds the four features.

## Interactive (paste into a Claude Code session)

```
/goal Implement SPEC7.md in full (read SPEC.md through SPEC6.md first; they remain authoritative elsewhere). Done when: 'npm run validate' exits 0 with its complete output — Rust tests R1–R13, frontend tests U1–U11, e2e tests E1–E13c and E14a–E14c, integration tests I1–I2, and the final line 'VALIDATION: ALL PASSED' — printed in the transcript, AND 'npm run tauri build' exits 0 with the produced .app path and size (< 80 MB) printed, AND 'grep -rn ".skip\|.only\|.todo" tests/' prints nothing, AND README.md covers the personal dictionary, the focus guard, sound cues, and the red recording dot per SPEC7 §7.4, AND ARCHITECTURE.md contains the "Focus guard" note per SPEC7 §7.5. Constraints: SPEC.md through SPEC7.md and this condition must not be modified; the focus-guard decision logic must be a pure table-tested function and the pipeline thread must never block waiting on the prompt (SPEC7 FR-G5); the guard fails open — a None frontmost read never blocks a paste; the transcript must reach history and last-transcription on every guard outcome including Esc and timeout; dictionary matching is literal whole-word phrases, never regex from user input; frontmost-app capture and sound playback stay behind the platform-abstraction boundary with no-op non-macOS stubs; no new permissions and no new network access; sound playback must be async and add no latency to the hotkey→overlay or stop→paste paths; existing tests may not be weakened, stubbed, or deleted to pass; if something is infeasible, record it in BLOCKERS.md instead of gaming the check. Stop after 60 turns or 6 hours even if incomplete, and summarize remaining work.
```

## Unattended one-liner

```bash
claude -p "/goal Implement SPEC7.md in full (read SPEC.md through SPEC6.md first; they remain authoritative elsewhere). Done when: 'npm run validate' exits 0 with its complete output — Rust tests R1–R13, frontend tests U1–U11, e2e tests E1–E13c and E14a–E14c, integration tests I1–I2, and the final line 'VALIDATION: ALL PASSED' — printed in the transcript, AND 'npm run tauri build' exits 0 with the produced .app path and size (< 80 MB) printed, AND 'grep -rn \".skip\|.only\|.todo\" tests/' prints nothing, AND README.md covers the personal dictionary, the focus guard, sound cues, and the red recording dot per SPEC7 §7.4, AND ARCHITECTURE.md contains the \"Focus guard\" note per SPEC7 §7.5. Constraints: SPEC.md through SPEC7.md and this condition must not be modified; the focus-guard decision logic must be a pure table-tested function and the pipeline thread must never block waiting on the prompt (SPEC7 FR-G5); the guard fails open — a None frontmost read never blocks a paste; the transcript must reach history and last-transcription on every guard outcome including Esc and timeout; dictionary matching is literal whole-word phrases, never regex from user input; frontmost-app capture and sound playback stay behind the platform-abstraction boundary with no-op non-macOS stubs; no new permissions and no new network access; sound playback must be async and add no latency to the hotkey→overlay or stop→paste paths; existing tests may not be weakened, stubbed, or deleted to pass; if something is infeasible, record it in BLOCKERS.md instead of gaming the check. Stop after 60 turns or 6 hours even if incomplete, and summarize remaining work."
```

## Notes

- The fun manual pass: dictate into Terminal while ⌘-tabbing to Slack
  mid-sentence — the prompt should name both apps, and **Paste** should land
  the text in Slack with zero focus flicker. Then let the prompt time out
  (10 s) and check the clipboard. Add "yat yat → Yat Yat" to the dictionary
  and say it. Toggle sound cues and listen for the tick/click. Watch the
  menu-bar mic go red while recording.
- The frontmost-app capture must happen at recording START (the overlay
  panel is non-activating, so frontmost == paste target at that moment) and
  the re-check immediately before the paste keystroke — after enhancement,
  which is exactly when users wander off to another app.
- tccutil/permissions are untouched — NSWorkspace frontmost queries need no
  new grants. If macOS 26 menu-bar gating hides the red dot, that's the
  existing Tahoe allowance issue (README Troubleshooting), not a SPEC7 bug.
- Watch the DoD grep: avoid the letter-sequences "skip"/"only"/"todo" in
  test names and comments (use "bypass"/"solely"/"pending" phrasing) — note
  the guard prompt's second button must not be literally named "Copy only"
  in a test name; call the test "copy-fallback" or similar.
