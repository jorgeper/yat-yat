# SPEC13: Yat Yat — transcription progress in the overlay

An increment over docs/specs/SPEC.md–SPEC12.md (authoritative elsewhere).
Today the overlay shows an indeterminate thinking shimmer for the whole
transcription (`overlay::state::TRANSCRIBING` → `thinking-wave` in
OverlayApp.tsx) — on a long recording with a slow model the user has no
idea whether the app is at 10% or 90%. The STT call
(`AppState::transcribe` → transcribe-rs) is a single blocking call with
**no progress callback** for either engine, so this spec adds an
**estimated** progress fill: expected transcription time = audio duration
× a per-model real-time factor learned from previous runs, ticked to the
overlay while the blocking call runs. The estimate is presented as a fill
sweeping the existing shimmer bars — deliberately **no numeric percent
text**, so an estimate never masquerades as a measurement.

Out of scope: patching/forking transcribe-rs for real whisper.cpp
progress; live-mode passes (`live_loop` has its own partial-text
feedback); the Retry path (it never shows the transcribing overlay —
tray + notification only); persisting learned factors across launches
(in-memory is enough; first run uses the seed).

## 1. Progress estimation (FR-P)

1. New pure module `src-tauri/src/progress.rs` (house pattern:
   `cleanup.rs`), no Tauri types:
   - `fraction(elapsed_secs: f32, expected_secs: f32) -> f32` with these
     **properties** (exact curve is implementer's choice; R18 asserts the
     properties, not the formula): strictly increasing in `elapsed`;
     `fraction(0, e) == 0`; ≈0.9 when `elapsed == expected`; approaches
     but never exceeds **0.95** — completion alone shows full.
     `expected` is clamped to ≥ 0.1 s so the function is total.
   - `Rtf` (exponential moving average of measured real-time factor =
     wall-clock STT secs ÷ audio secs): `estimate(&self) -> f32` and
     `observe(&mut self, audio_secs: f32, wall_secs: f32)` with
     `ALPHA = 0.3`. Seed `DEFAULT_RTF = 0.5` — deliberately slow-side:
     an over-estimate makes the bar finish early (pleasant), an
     under-estimate parks it at the cap (annoying). Non-finite or
     non-positive observations are ignored.
2. `AppState` gains `rtf: Mutex<HashMap<String, Rtf>>` keyed by model id
   (in-memory only; entry created on first use from the seed).
3. `finish_recording` (pipeline.rs), after `samples` is known and the
   TRANSCRIBING overlay is shown: compute `audio_secs =
   samples.len() as f32 / 16_000.0` and
   `expected = audio_secs × rtf.estimate()`
   for the active model, then spawn a short-lived **ticker thread** that
   emits `transcribe-progress` (bare `f32` 0..1, same shape discipline as
   `mic-level`) via `emit_to(OVERLAY_LABEL, ..)` every **100 ms** until a
   stop flag (e.g. `Arc<AtomicBool>`) is set. The pipeline thread then
   runs `transcribe_and_clean` exactly as today.
4. The stop flag is set on **every** exit from the transcription block —
   success, empty text, and error — before the overlay changes state or
   hides, so no tick can land on a dead or re-shown overlay. On success
   one final `transcribe-progress` of `1.0` is emitted before the
   delivery/focus-guard logic runs.
5. After a **successful** `state.transcribe` (raw STT only — cleanup and
   enhancement excluded from the measurement), `observe` the measured
   RTF for the active model. Time it around the STT call inside
   `transcribe_and_clean` or by threading the wall time out — either
   way the enhancement round-trip must not pollute the EMA. While
   enhancement runs, the ticker simply keeps easing toward the cap;
   that's honest enough and costs nothing.
6. The pipeline thread never blocks on the ticker and the ticker never
   touches pipeline state beyond the flag — the SPEC invariant that only
   the pipeline thread owns lifecycle decisions is untouched.

## 2. Overlay fill (FR-O)

1. New pure helpers in `src/overlay/progress.ts`:
   `advance(prev: number, next: number) -> number` (clamp `next` to
   [0, 1], never return less than `prev` — the UI only moves forward)
   and `filledBars(fraction: number, barCount: number) -> number`
   (0..barCount, full only at fraction ≥ 1).
2. OverlayApp listens for `transcribe-progress`; the transcribing state
   keeps the existing `thinking-wave` bars (`data-testid` unchanged, E6
   untouched) but marks the first `filledBars(fraction, BAR_COUNT)` bars
   with a `fill` class — a left-to-right sweep across the shimmer. The
   pill also exposes `data-progress="<rounded percent>"` for tests.
   Progress resets to 0 on every `show-overlay` with state
   `transcribing`; it is ignored in every other state.
3. Fill colors come from the active theme's CSS variables only — no
   color literals (the U8 rule's spirit; this is overlay CSS, not an
   effect module, but the same discipline applies). Reduced motion:
   the shimmer keyframes stay suppressed as today; the discrete fill
   itself is a width/class change, not an animation, and remains.
4. No numeric percent is ever rendered. No new overlay states; no
   change to recording, focus-prompt, or live behavior.

## 3. IPC mirror (FR-M)

1. `transcribe-progress` is added to `src/ipc/types.ts` (event name +
   payload type `number`) alongside `mic-level`. No new command exists,
   so `api.ts` and the mock's `invoke` switch are unchanged; e2e drives
   the event through the existing generic `window.__mock.emit`, exactly
   as overlay.spec.ts already drives `show-overlay`/`mic-level`.

## 4. Tests (added: R18, U19, E19a–E19b)

- **R18**: `progress::fraction` is strictly increasing over a sampled
  range, 0 at 0, ≈0.9 at `elapsed == expected` (±0.05), never exceeds
  0.95 (including `elapsed = 100 × expected` and `expected = 0`);
  `Rtf` returns the seed before any observation, moves toward an
  observed value by ALPHA, and ignores non-finite/non-positive
  observations.
- **U19**: `advance` clamps to [0, 1] and never regresses;
  `filledBars` is 0 at 0, `barCount` only at ≥ 1, monotonic, and
  in-range for fractional inputs.
- **E19a**: show-overlay transcribing → `data-progress` is 0; emitting
  `transcribe-progress` 0.25 then 0.6 advances the fill (`fill`-class
  bar count and `data-progress` both increase); a subsequent 0.3 does
  **not** regress either.
- **E19b**: `transcribe-progress` 1.0 fills all bars; after
  hide-overlay, a fresh show-overlay transcribing starts back at 0
  (no bleed-through from the previous session); progress events during
  the recording state leave the waveform untouched.
- validate.mjs step labels update to R1–R18, U1–U19, E1–E19b.
- No existing test is amended. Existing tests may never be weakened,
  stubbed, or deleted.

## 5. Docs

docs/ARCHITECTURE.md: one short section — estimated progress (audio
duration × per-model RTF EMA, in-memory, seed 0.5), the
`transcribe-progress` event, the ticker-thread/stop-flag shape, and the
explicit note that this is an **estimate** because transcribe-rs exposes
no progress callback (revisit if the crate ever grows one). README
untouched.

## 6. Definition of Done

1. `npm run validate` exits 0 with complete output — R1–R18, U1–U19,
   E1–E19b, I1–I2, `VALIDATION: ALL PASSED` — printed in the transcript.
2. `npm run tauri build` (signed env) exits 0; app size still < 80 MB.
   macOS first: Windows compile is verified by the release pipeline's
   test-windows job at the next cut, not gated here.
3. docs/ARCHITECTURE.md updated per §5;
   `grep -rn ".skip\|.only\|.todo" tests/` prints nothing.
4. Anything infeasible → BLOCKERS.md; never game a check.

Manual checks after green: dictate ~30 s of speech on the slowest
installed model — the shimmer fills left-to-right during the pause and
never sits full before the paste lands; dictate twice in a row — the
second transcription starts from an empty fill and (thanks to the EMA)
tracks noticeably better than the first; System Settings → Accessibility
→ Reduce Motion on — the shimmer is still, the fill still advances.
