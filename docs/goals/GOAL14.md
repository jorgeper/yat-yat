# Launching the performance build with /goal

Run from this directory (`yat-yat/`). docs/specs/SPEC.md–SPEC13.md remain
authoritative; docs/specs/SPEC14.md is a pure performance increment —
no user-visible behavior changes, only latency, memory, and battery:
the ~210 ms of fixed sleeps on stop→paste shrink to ≤60 ms, the engine
pre-warms so the 4.8 s cold load never lands on a stop path, capture
moves to incremental 16 kHz (killing live mode's O(n²) resample/clone),
the settings webview stops being built at launch and stops polling
while hidden, the overlay frame-caps at 60 fps with dt-based decay, and
the binary drops to one HTTP/TLS stack.

The command deliberately does NOT restate the FRs — /goal conditions
are capped at 4000 characters (see AGENTS.md), so the SPEC carries the
detail and the condition binds "implement it in full" plus the
transcript-provable checks.

## Interactive (paste into a Claude Code session)

```
/goal Implement docs/specs/SPEC14.md in full — a performance spec with ZERO user-visible behavior change (read src-tauri/src/pipeline.rs, paste.rs, audio.rs, state.rs, lib.rs and src/overlay/effects/engine.ts first). The SPEC is authoritative for every FR-D/W/A/S/R/B requirement and every invariant it names (one pipeline thread, non-activating overlay, IPC mirror commands.rs⇄mock.ts, server-owned settings preserved, localhost-only enhancement, exactly 15 effects / 12 themes) — macOS-first, fully local. Done when: 'npm run validate' exits 0 on macOS with its complete output — Rust tests R1–R25, frontend tests U1–U22, e2e tests E1–E20b, integration tests I1–I2, and the final line 'VALIDATION: ALL PASSED' — printed in the transcript, AND 'npm run tauri build' with the signing env exits 0 with the .app path and size (< 80 MB) printed, AND the transcript proves the stop→paste fixed-sleep budget is ≤ 60 ms (the adaptive-drain cap and the two paste constants shown at their new values, and no new sleeps between finish_recording and deliver), AND docs/ARCHITECTURE.md carries the re-measured §7 numbers (YATYAT_PERF_PROBE output shown; idle RSS re-measured without the resident settings webview; the fixed-delay budget table) plus the 16 kHz-capture / pre-warm / lazy-settings-window / frame-cap design notes, AND 'grep -rn ".skip\|.only\|.todo" tests/' prints nothing. Constraints: the SPEC and GOAL files and this condition must not be modified; every SPEC14 FR is implemented as written — where this condition is silent, the SPEC decides; the new tests R22–R25, U20–U22, E20a–E20b match the SPEC §7 definitions; NO existing test is amended, weakened, or deleted; no new network access — the fully-local guarantee is the product; Retry must never read a partially written WAV (the join-handle rule in SPEC FR-D1.3); if something is infeasible, record it in BLOCKERS.md instead of gaming the check; after validate is first green, run one adversarial review pass over the ENTIRE diff hunting the regressions SPEC14 risks — dropped/garbled pastes from the shorter delays, audio-tail loss when the hotkey is released right after a short word, event-ordering races (progress ticker vs overlay state, deep-link emit vs listener registration, pre-warm mutex vs a fast first stop), stale settings UI after re-show — and fix findings and re-run validate before declaring done. Stop after 50 turns or 5 hours even if incomplete, and summarize remaining work.
```

## Unattended one-liner

Same prompt via `claude -p "/goal …"` if you want it headless; the
interactive form is strongly recommended here — the manual checks
(paste timing across five real apps, a fresh model download over
rustls, Activity Monitor idle inspection, a 3-minute live dictation)
need a human at the machine.

## Notes

- **Paste-delay shrink is the risky FR.** 50→20 ms clipboard settle and
  100→20 ms modifier hold are conservative but timing-sensitive apps
  exist (some Electron apps). They stay named constants; if a target app
  drops pastes in the manual checks, bump that constant and note the
  measured floor in BLOCKERS.md — don't silently restore the old values.
- **Lazy settings window is the fiddly FR.** `emit_to` into a
  just-created webview races its listener registration — the spec
  requires the deep-link section to ride the window URL query or a
  ready-signal replay. Test dock-click Reopen and "Check for Updates…"
  by hand; e2e can't see Rust window creation (it drives the mock).
- **Incremental resample must flush its tail** at stop or the last
  ~20–60 ms of speech (word endings!) is lost — R24's length assertion
  exists exactly for this.
- **Pre-warm holds the engine mutex while loading.** That's fine — a
  stop arriving mid-load waits exactly as it does today, just with a
  head start — but never call ensure_loaded from the pipeline or main
  thread.
- The interval governor already bounds live-pass *cadence*; this spec
  bounds per-pass *cost* (16 kHz snapshots) and stop-path *queuing*
  (stop_pending). The tail-window strategy that would bound
  transcription cost itself is explicitly out of scope.
- Watch the DoD grep: avoid the letter-sequences "skip"/"only"/"todo"
  in test names and comments (e.g. write "no-op", not "only-once").
