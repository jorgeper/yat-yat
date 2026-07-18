# Launching the transcription-progress build with /goal

Run from this directory (`yat-yat/`). docs/specs/SPEC.md–SPEC12.md remain
authoritative; docs/specs/SPEC13.md adds an estimated progress fill to
the overlay's transcribing state (audio duration × learned per-model
real-time factor — transcribe-rs exposes no real progress callback).

## Interactive (paste into a Claude Code session)

```
/goal Implement docs/specs/SPEC13.md in full (read src-tauri/src/pipeline.rs finish_recording and src/overlay/OverlayApp.tsx first — the fill rides the EXISTING thinking-wave bars; do NOT fork or patch transcribe-rs, do NOT touch live mode or the Retry path, and render no numeric percent text — macOS-first, fully local). Done when: 'npm run validate' exits 0 on macOS with its complete output — Rust tests R1–R18, frontend tests U1–U19, e2e tests E1–E19b, integration tests I1–I2, and the final line 'VALIDATION: ALL PASSED' — printed in the transcript, AND 'npm run tauri build' with the signing env exits 0 with the .app path and size (< 80 MB) printed, AND docs/ARCHITECTURE.md documents the estimated-progress design (RTF EMA, transcribe-progress event, ticker/stop-flag shape, and that it's an estimate for lack of a crate callback), AND 'grep -rn ".skip\|.only\|.todo" tests/' prints nothing. Constraints: the SPEC and GOAL files and this condition must not be modified; the estimator lives in a new pure module src-tauri/src/progress.rs covered by R18 (fraction: strictly increasing, 0 at 0, ~0.9 at elapsed==expected, capped below 0.95, total for expected<=0; Rtf EMA: seed 0.5, alpha 0.3, ignores junk observations) and the EMA observes RAW STT wall time only — never cleanup or enhancement; the ticker thread emits transcribe-progress (bare f32, emit_to overlay, ~100 ms) and its stop flag is set on EVERY exit path — success, empty, error — before the overlay changes state, with a final 1.0 emitted only on success; the pipeline thread must never block on the ticker; frontend helpers advance/filledBars in src/overlay/progress.ts are pure and covered by U19, the fill only moves forward, resets on each new transcribing show-overlay, uses theme CSS variables only (no color literals), and survives prefers-reduced-motion as a static advancing fill; transcribe-progress is added to src/ipc/types.ts and e2e (E19a–E19b) drives it via window.__mock.emit like overlay.spec.ts drives mic-level; NO existing test is amended, weakened, or deleted (E6's thinking-wave assertion must still pass) and the thinking-wave testid is unchanged; no new network access; if something is infeasible, record it in BLOCKERS.md instead of gaming the check. Stop after 40 turns or 4 hours even if incomplete, and summarize remaining work.
```

## Unattended one-liner

Same prompt via `claude -p "/goal …"` if you want it headless; the
interactive form is recommended — the manual checks at the end of the
SPEC's DoD need a human dictating anyway.

## Notes

- The fill is an ESTIMATE presented honestly: it eases toward a 0.95 cap
  and only completion snaps it full. If the bar habitually parks at the
  cap, the seed RTF (0.5) is too low for that machine/model — the EMA
  self-corrects from the second dictation onward.
- Enhancement (localhost LLM) time is deliberately outside the estimate;
  with enhancement on, the bar crawling near the cap during the LLM
  round-trip is expected behavior, not a bug.
- Ticker hygiene is the risky part: a tick landing after the overlay
  switched to the focus prompt (or a re-shown recording) would flash
  stale state — the stop-flag-before-any-overlay-change ordering in
  SPEC13 FR-P4 is load-bearing.
- Watch the DoD grep: avoid the letter-sequences "skip"/"only"/"todo" in
  test names and comments.
