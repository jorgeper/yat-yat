# SPEC14: Yat Yat — performance: delivery latency, idle cost, startup

An increment over docs/specs/SPEC.md–SPEC13.md (authoritative elsewhere).
A code-level performance review (2026-07-18) found the app slower and
heavier than its measured components justify. The headline evidence:

- **≈210 ms of fixed `thread::sleep` on every stop→paste** — 60 ms audio
  flush (`audio.rs` stop), 50 ms clipboard settle + 100 ms paste-modifier
  hold (`paste.rs`, both on the macOS **main thread**). That fixed budget
  rivals an entire whisper-tiny inference (97–431 ms measured).
- **The 4.8 s cold model load lands on the first dictation's stop path**
  when live transcription is off (engine loads lazily inside
  `AppState::transcribe`), and again after every model switch.
- **Live mode does O(n²) work**: every pass clones the whole device-rate
  buffer (5 min @ 48 kHz ≈ 57 MB memcpy), resamples all of it, and
  re-transcribes all of it — and the stop path can queue behind a full
  in-flight pass on the engine mutex.
- **The overlay renders at native refresh** (120 Hz on ProMotion) for a
  30 Hz level stream, with a per-frame level decay that is
  refresh-rate-dependent, under an 18 px backdrop blur.
- **The hidden settings webview is built at every launch and polls
  forever**: `get_app_info` every 2 s for the app's lifetime, 5 IPC
  round-trips/s (incl. the tray window-server probe) while the wizard is
  up, a 25 Hz preview timer if hidden on Appearance — a background
  utility with a permanent timer heartbeat.
- **Two complete HTTP/TLS stacks are linked** (app reqwest 0.12
  native-tls + updater reqwest 0.13 rustls), and both webviews parse one
  216 KB bundle containing both apps.

This spec removes those costs. It changes **no user-visible behavior**
except speed, memory, and battery; every existing invariant (one
pipeline thread, non-activating overlay, IPC mirror, whole-object
settings writes, localhost-only enhancement) stands.

Out of scope, deliberately: `panic = "abort"` (crash-behavior decision —
an abort mid-dictation loses the take); moving the pill's blur material
to an NSVisualEffectView (platform-seam churn for a cost FR-R1 already
shrinks); a tail-window live-transcription strategy (changes visible
live-text behavior; FR-A1/FR-A2 bound today's costs first); dynamic ort
loading (breaks the single-binary story); keeping the mic stream warm
between dictations (the orange mic indicator would stay lit — a privacy
non-starter); idle engine unload (product choice, revisit on demand).

## 1. Delivery latency — stop → text pasted (FR-D)

1. `audio.rs` stop: replace the fixed 60 ms flush sleep with an adaptive
   drain — a `recv_timeout` loop over `samples_rx` that returns once the
   channel has been quiet for ~15 ms, hard-capped at the old 60 ms. New
   pure-ish helper (channel in, samples out) covered by **R22**.
2. `paste.rs`: `PRE_PASTE_DELAY_MS` 50 → **20**; the 100 ms
   modifier-hold inside `send_paste_keystroke` → **20 ms**. Both remain
   named constants. The stop→paste fixed-sleep budget after this spec is
   **≤ 60 ms total** (drain cap + 20 + 20). The 300 ms deferred
   clipboard restore (R20) is off-path and unchanged.
3. The retained-recording WAV write moves off the hot path: spawn it on
   a background thread (clone of `samples`) so STT starts immediately;
   Retry reads it much later. Retry must never see a partial file — the
   pipeline holds the write's join handle and a Retry waits for it (or
   reads the in-memory samples directly). Format becomes 16-bit PCM
   (`read_wav_16k_mono` already handles Int; halves the file).
4. `record_transcription` keeps the in-memory `history.push` +
   `last_transcription` **before** the focus decision (SPEC7 stands),
   but `history.save`, the `history-changed` emit, and
   `tray::refresh_menu` are deferred until after delivery (spawned or
   post-`deliver_text`).
5. RTF persistence is debounced: `observe_rtf` only marks a dirty flag;
   `rtf.json` is written on return-to-idle / app exit, never per
   observation (live passes currently write the file every 1–4 s).
   Debounce semantics covered by **R23**. R21 (corrupt/missing reads)
   still passes.
6. `enhance.rs` reuses one `reqwest::Client` (`OnceLock`) across warmup
   and enhance, so warmup's connection actually serves the enhance call.

## 2. Engine warm-up (FR-W)

1. The load block inside `AppState::transcribe` is factored into
   `ensure_loaded(&self, app)` (same engine mutex, same
   `model-state-changed` events, same drop-before-load 2× RAM guard).
2. Pre-warm calls `ensure_loaded` on a **background thread** (never the
   pipeline or main thread): (a) at startup when the active model's
   files are on disk, (b) after `set_active_model` completes, (c)
   belt-and-braces at `start_recording` when the engine is `None`. The
   engine mutex already serializes racing loaders; a stop that arrives
   mid-load waits exactly as it would today — just starting seconds
   earlier.
3. Result: the first dictation after launch or model switch no longer
   pays 4.8 s between stop and paste (with any realistic pause before
   dictating; a hotkey inside the load window degrades to today's
   behavior, never worse).

## 3. Audio capture & live mode (FR-A)

1. The capture buffer is kept **at 16 kHz**: `push_chunk` feeds a
   persistent incremental resampler (rubato) as chunks arrive; stop
   flushes the resampler tail. `snapshot_16k` becomes a plain copy (no
   per-pass full-buffer resample; 3× smaller clones) and
   `finish_samples` stops resampling at stop time. **R24** proves the
   incremental path ≡ the whole-buffer `resample_to_16k` within
   tolerance. R10's snapshot semantics unchanged.
2. New `stop_pending: AtomicBool` set at the top of `finish_recording`
   (before `recorder.stop()`); `live_loop` checks it after taking its
   snapshot and before `state.transcribe`, skipping the pass — the stop
   path no longer queues behind a freshly started live pass.
3. `CaptureBuffer::clear()` (and the Cancel path) releases capacity —
   a cancelled 5-min recording must not leave ~58 MB resident (**R25**).
4. The audio worker uses a blocking `recv()` while no stream is active
   (today: 100 wakeups/s forever); the 10 ms `recv_timeout` cadence
   applies only while recording.

## 4. Startup & idle (FR-S)

1. `setup()` order becomes tray → overlay → (settings only if
   onboarding): the menu-bar icon — the only visible launch signal —
   paints before any webview is constructed.
2. The settings window is **lazily created**: `show_settings_window`
   builds it on demand when absent (eager creation remains for first-run
   onboarding). Deep links must still land: `navigate-section` /
   `check-updates` may not be emitted into a webview with no listeners —
   pass the section via the window URL query or replay on a
   frontend-ready signal. Close keeps today's hide-don't-destroy.
   Dock-click Reopen, tray Settings, the no-model flow, ⌘, and
   `ready_activation` (R19) all keep working.
3. The settings webview gates **every** poll on visibility
   (`visibilitychange` + one immediate refresh on show): the 2 s
   capture-dead poll, the 1 s onboarding poll, the 40 ms Appearance
   preview feeder, and the `history-changed` → `get_history` refresh
   all stop while the window is hidden. Hidden window ⇒ zero timers.
4. Onboarding snapshot cost: **one** `getAppInfo` per tick
   (`captureReady` derived from it, not a second call), and the tray
   window-server probe runs only on the menubar step. A pure
   `factsForStep` helper decides what each step reads (**U21**).
5. `tray::set_state` uses cached `Image`s (`OnceLock`) instead of
   re-reading + re-decoding PNGs from disk on every transition, and only
   rebuilds the menu when its contents actually change.
6. The initial `init_capture` call moves off the `setup()` path (after
   Ready); the existing 3 s capture watcher already covers the retry.

## 5. Overlay rendering (FR-R)

1. `EffectEngine`'s tick loop is frame-capped at ~60 fps (30 under
   `reducedMotion`), and the level decay becomes dt-based
   (`level *= pow(0.92, dt·60)`) so the look is identical at any refresh
   rate — today it decays 4× faster at 120 Hz than 30 Hz. **U20** drives
   the loop with fake timestamps: render count capped, decay equal over
   equal simulated time regardless of tick rate.
2. Per-frame allocation hygiene: the engine reuses one mutable `frame`
   object; bar effects index `frame.levels` from an offset instead of
   `slice(-count)`; particle effects compact in place instead of
   `filter()`. `EffectFrame`'s shape is unchanged; still exactly 15
   effects.
3. `fireflies` pre-renders its glow to an offscreen sprite (built from
   `frame.colors`, rebuilt on color change) and `drawImage`s it —
   removing 16 shadow-blurred fills per frame. U8 (no color literals)
   must still pass.
4. `setAsleep(false)` is guarded by a ref so setState fires on the wake
   **transition**, not at 30 Hz while speaking; `transcribe-progress`
   setState is quantized to the filled-bar count (same observable
   `data-progress` values — E19a/E19b untouched).
5. `easter_eggs` joins the overlay `ShowPayload` (Rust already holds the
   settings there), removing the per-recording `get_settings` round-trip
   on the hotkey→pill path. Mirrored in `src/ipc/mock.ts` and
   `types.ts` per the IPC-mirror rule (**E20a**: no `get_settings`
   invoke on show; eggs flag honored from the payload).
6. `applyTheme` memoizes the last applied theme id + css: re-applying
   the same theme is a no-op (no DOM write, no `listUserThemes` IPC —
   **U22**, **E20b**); `EffectEngine.start()` no longer re-runs the
   `initRenderer` that `setEffect()` just did (one canvas backing-store
   allocation per show, not two).
7. Async `listen(...)` registrations get the standard cancelled-flag
   cleanup so a fast unmount can't leak listeners into the
   lives-forever settings webview.

## 6. Build & bundle hygiene (FR-B)

1. App `reqwest` moves to the updater's stack: `0.13`,
   `default-features = false`, `features = ["json", "stream",
   "rustls-tls"]` — one HTTP/TLS stack in the binary. Verify a real
   model download end-to-end manually (rustls vs Security.framework);
   if GitHub downloads break, record in BLOCKERS.md and revert this FR
   alone.
2. `dirs = "5"` → `"6"` (the tree already carries v6).
3. `crate-type` trimmed to `["rlib"]` (staticlib/cdylib return when a
   mobile target does) — one LTO link instead of three, smaller target
   dir; `tauri build` and the R-suite must still link.
4. `main.tsx` lazily imports the window it isn't
   (`React.lazy` + null-fallback Suspense): the always-alive overlay
   webview stops parsing the settings/onboarding/updater code and vice
   versa. Same URL params; e2e and the mock path unchanged.

## 7. Tests (added: R22–R25, U20–U22, E20a–E20b)

- **R22**: the adaptive stop-drain helper returns all queued chunks and,
  with an already-quiet channel, returns well under the old fixed 60 ms
  (bounded by the quiet window), never exceeding the 60 ms cap.
- **R23**: RTF persistence is debounced — `observe` marks dirty without
  invoking the injected writer; a flush writes exactly once and clears
  dirty; no-observation flushes write nothing.
- **R24**: feeding device-rate chunks through the incremental 16 kHz
  capture path equals `resample_to_16k` over the concatenated buffer
  (length within one resampler frame; samples within tolerance), at
  48 kHz and at a non-integral ratio (e.g. 44.1 kHz).
- **R25**: after Cancel/clear, the capture buffer's retained capacity is
  released (≤ a small bound), while stop's `take()` behavior is
  unchanged.
- **U20**: engine frame cap + dt-based decay (fake-timestamp tick loop;
  render count capped at ~60 fps from 120 Hz input; equal decay over
  equal simulated time at 30 Hz vs 120 Hz tick rates).
- **U21**: `factsForStep` — menubar step is the sole tray-probe reader;
  every step's plan implies a single `getAppInfo`.
- **U22**: `applyTheme` re-applied with an unchanged theme performs no
  DOM mutation and no user-theme fetch; a changed id does both.
- **E20a**: show-overlay (recording) carries `easter_eggs` in the
  payload; the overlay issues **no** `get_settings` invoke on show (the
  mock counts invocations); the eggs gate still works from the payload.
- **E20b**: two consecutive show-overlay events with the same `user:`
  theme fetch user themes once; switching themes fetches again.
- validate.mjs step labels update to R1–R25, U1–U22, E1–E20b.
- No existing test is amended. Existing tests may never be weakened,
  stubbed, or deleted (E6, E19a/E19b, R10, R19, R20, R21, U8 named here
  because this spec walks nearest to them).

## 8. Docs

docs/ARCHITECTURE.md: update the pipeline sketch (16 kHz capture,
pre-warmed engine, deferred history/tray/RTF persistence, lazy settings
window, frame-capped overlay) and **re-measure the §7 table** on this
machine — at minimum idle RSS (expect a real drop without the resident
settings webview), the stop→paste fixed-delay budget (≈210 ms → ≤60 ms),
and the perf-probe overlay number, with the `YATYAT_PERF_PROBE` output
shown. Note the reqwest unification under the network-guarantee section
(rustls, still downloads-and-localhost-only). README untouched.

## 9. Definition of Done

1. `npm run validate` exits 0 with complete output — R1–R25, U1–U22,
   E1–E20b, I1–I2, `VALIDATION: ALL PASSED` — printed in the transcript.
2. `npm run tauri build` (signed env) exits 0; app size still < 80 MB,
   path + size printed. macOS first: Windows compile is verified by the
   release pipeline's test-windows job at the next cut, not gated here.
3. The stop→paste fixed-sleep budget is provably ≤ 60 ms: the transcript
   shows the three constants/caps (drain cap, pre-paste, modifier hold)
   at their new values and confirms no new sleeps entered
   `finish_recording`→`deliver`.
4. docs/ARCHITECTURE.md updated per §8 with re-measured numbers;
   `grep -rn ".skip\|.only\|.todo" tests/` prints nothing.
5. Anything infeasible → BLOCKERS.md; never game a check.

Manual checks after green: launch the app, wait ~5 s, dictate
immediately with live transcription OFF — no multi-second stall before
the paste (pre-warm working); paste into TextEdit, VS Code, Slack,
Terminal, and a browser text field — text lands intact with the shorter
delays; dictate 3+ minutes with live ON — CPU/fans stay reasonable and
release-to-paste doesn't hang behind a live pass; Activity Monitor at
idle — RSS well under today's 102 MB, 0.0 % CPU, and (Instruments or
`sample`) no periodic wakeups from a hidden settings window; on a
ProMotion display, recording-time CPU visibly lower than before;
download a fresh model end-to-end (FR-B1's rustls swap); Check for
Updates…, dock-click Reopen, tray Settings, and the no-model deep link
all still open/land correctly with the lazy settings window.
