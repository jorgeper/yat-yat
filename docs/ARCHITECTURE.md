# Yat Yat Architecture

## Pipeline

```
Right ⌘ (handy-keys event tap)
   └─> pipeline.rs   Idle → Recording → Processing state machine (1 thread, serialized)
         ├─ audio.rs        cpal @ device native rate → mono downmix → RMS levels (~30 Hz)
         │                  → incremental rubato resample to 16 kHz AS CHUNKS ARRIVE (SPEC14);
         │                    stop = adaptive drain (quiet 15 ms, cap 60 ms) + tail flush
         ├─ overlay.rs      non-activating pill (NSPanel), waveform from level events
         ├─ stt.rs          transcribe-rs: Parakeet (ONNX) | Whisper GGUF (whisper.cpp+Metal)
         │                  engine pre-warmed in the background (SPEC14) — never cold on stop
         ├─ cleanup.rs      pure filter: fillers, [artifacts], repeats, whitespace  ← R1–R4
         ├─ enhance.rs      optional localhost-only Ollama rewrite, 5 s timeout → fallback
         └─ paste.rs        clipboard save → write (20 ms settle) → ⌘V (enigo, 20 ms hold)
                            → deferred restore (~300 ms, off-thread)
```

## Module map

**Rust host (`src-tauri/src/`)**

| Module | One purpose | Key tests |
| --- | --- | --- |
| `cleanup.rs` | Pure transcript filter (`clean(text, opts)`) + personal dictionary | R1–R4, R12 |
| `settings.rs` | Settings JSON + localhost endpoint guard | R5, R8, R13 |
| `focus.rs` | Frontmost-app capture + pure focus-guard decision | R13 |
| `sounds.rs` | Sound cues: async afplay, never on the timing path | manual |
| `registry.rs` | Data-driven model catalog (`models.json`) | R7 |
| `history.rs` | 20-entry ring buffer + retained last WAV | R6 |
| `audio.rs` | cpal capture worker thread, resampling, WAV IO | resample/WAV tests |
| `stt.rs` | Engine abstraction (Whisper/Parakeet), `load_path` for CLI | exercised by I1 |
| `downloader.rs` | Resume-able downloads, SHA-256 verify, tar.gz extract | sha/cancel tests |
| `enhance.rs` | Ollama client, warmup, timeout fallback | localhost-guard tests |
| `hotkey.rs` | handy-keys manager thread, capture mode for the recorder UI | validated via binding parse |
| `overlay.rs` | Pill window: create/show/hide/position, level throttling | E6/E8 (UI side) |
| `pipeline.rs` | The dictation state machine and delivery | manual + E-suite |
| `paste.rs` | Clipboard write + synthesized ⌘V + restore | manual (needs AX) |
| `tray.rs` | Tray icon states + menu, hotkey labels | label test |
| `commands.rs` | Tauri IPC surface (mirrored by `src/ipc/mock.ts`) | E1–E8 |

**Frontend (`src/`)**: `ipc/` (typed API + browser mock), `settings/` (four
sections + onboarding), `overlay/` (the pill), `lib/` (pure helpers — U2–U4),
`store/` (U1).

The CLI (`yat-yat transcribe|clean|bench-clean`) exposes the same engine +
cleanup code paths headlessly; the validation harness (I1/I2) drives it.

## Windows platform-abstraction boundary

Everything below compiles cross-platform today (cpal, rubato, transcribe-rs,
enigo, handy-keys, arboard, reqwest are all Windows-capable). The
platform-specific seams, each isolated in one place:

| Concern | macOS implementation | Windows port point |
| --- | --- | --- |
| Overlay window | `overlay.rs::platform` (mac): tauri-nspanel non-activating panel | `overlay.rs::platform` (non-mac) already provides the `focusable(false)` + always-on-top window; add a `SetWindowPos(HWND_TOPMOST)` reassert after show if z-order flickers |
| Paste keystroke | `paste.rs::send_paste_keystroke`: ⌘ + kVK 9 | same fn, `cfg(windows)` arm: Ctrl + VK 0x56 (already written) |
| Hotkey backend | handy-keys event tap (Accessibility) | handy-keys `WH_KEYBOARD_LL` hook (no permission needed) — same API |
| Permissions | `tauri-plugin-macos-permissions` + onboarding step | step is skipped on non-mac (`Onboarding.tsx` builds the step list per platform) |
| Dock + tray app | regular activation (LSUIElement dropped post-SPEC8 by owner preference; Dock click → Reopen → Settings) | standard taskbar presence + tray |
| Frontmost app (focus guard) | `focus.rs`: NSWorkspace via main thread | **implemented (SPEC10)**: `GetForegroundWindow` → exe path, compared lowercased (`windows_exe_key`, R14); every failure is `None` — guard fails open |
| Sound cues | `sounds.rs`: spawn `afplay` | **implemented (SPEC10)**: `PlaySoundW` (`SND_SYNC` on a worker thread so the buffer outlives playback) |
| Whisper accel | `whisper-metal` feature | CPU for now — `whisper-vulkan` blocked on an upstream ggml release-profile build bug (BLOCKERS.md §3); re-enabling is a one-line feature swap |

Remaining Windows work is packaging (MSI/NSIS via `tauri build`), not code.

## Measured performance (SPEC §7, re-measured after SPEC14)

Machine: Apple Silicon Mac (Darwin 25.5), release build, 2026-07-18.

| Target | Requirement | Measured |
| --- | --- | --- |
| Hotkey → overlay visible | < 150 ms | `show_state` → visible: **0.24–0.61 ms** (perf probe, three runs; tray icons now come from a decode-once cache, so `set_state` on this path stopped reading disk) |
| Stop → text pasted (Whisper tiny) | < 1 s | inference 97–431 ms (warm/cold Metal, unchanged) + cleanup 0.06 ms + **fixed sleeps ≤ 55 ms typical** (was ~210 ms — see the budget table below) ⇒ **≈ 0.2–0.5 s** |
| Idle RSS (tray-only launch, engine not yet warm) | < 400 MB | **95 MB** main process (was 102 MB *with* the settings webview always built; the webview's WebContent process — no longer spawned at launch — lives out-of-process, so the main-process delta understates the saving) |
| Idle RSS (engine pre-warmed) | — | active model resident from launch by design — **1,875 MB with Parakeet v3** (0.6 B ONNX). Not a regression: SPEC §3 keeps the engine resident forever, so this was always the post-first-dictation steady state; SPEC14's pre-warm just moves the load (553 ms measured for Parakeet) to launch so it never lands between stop and paste |
| Idle CPU | < 1 % | **0.0 %** — and the audio worker now blocks on its channel when idle (was 100 wakeups/s) and a hidden settings webview runs zero timers |
| Cleanup filter, 1,000 words | < 1 ms | 59.6 µs per run (bench-clean; unchanged code) |

Stop→paste fixed-delay budget (SPEC14 FR-D — was ≈210 ms of
unconditional sleeps every dictation):

| Delay | Was | Now |
| --- | --- | --- |
| Audio flush at stop | fixed 60 ms sleep | adaptive drain: returns at 15 ms of channel quiet (typical ≤ 15 ms), hard cap 60 ms (R22) |
| Clipboard settle | 50 ms | **20 ms** (`PRE_PASTE_DELAY_MS`) |
| Paste-modifier hold | 100 ms | **20 ms** (`PASTE_MODIFIER_HOLD_MS`) |
| Clipboard restore | 300 ms, off-thread | unchanged (R20 — never on the path) |

Fixed sleeps on the path: **40 ms** + the adaptive drain (≤ 15 ms
typical) ⇒ ≈ 55 ms typical, 100 ms absolute worst case. If a
timing-sensitive app ever drops pastes, bump the named constant and log
the measured floor in BLOCKERS.md — never revert silently.

Model load (Whisper tiny): 4.8 s cold file cache, 154 ms warm; Parakeet
v3: 553 ms (measured at pre-warm). The active engine is loaded once and
kept resident (`state.rs`) — and since SPEC14 the load is pre-warmed in
the background (launch / model switch / recording start), so no
dictation's stop path ever pays it.

## Onboarding gates (SPEC2)

The wizard never stores a step index. A snapshot of verified facts —
`{ microphone, accessibility, captureReady, trayVisible, modelReady,
platform }` — is polled live, and the pure function
`firstUnmetStep(snapshot, skips)` (src/lib/onboarding.ts, U5-tested) decides
what to show; the wizard resumes and auto-advances from that alone. Who
verifies each fact:

| Fact | Verifier |
| --- | --- |
| `microphone` / `accessibility` | tauri-plugin-macos-permissions checks, polled 1 s |
| `captureReady` | the hotkey service is actually armed (`get_app_info` + `capture-ready` event from the lib.rs capture watcher) — accessibility is a two-fact gate |
| `trayVisible` | `tray_item_visible` (src-tauri/src/tray_probe.rs): in-process AppKit probe of the app's NSStatusBarWindow — frame width + occlusion state, on the main thread. (CGWindowList is unreliable here: macOS 26's Control Center hosts visible third-party items, so the app's own server-side window list shows a zero-size placeholder when suppressed and nothing when visible.) The wizard also offers an explicit "I can see the icon" attestation since Apple ships no public API for this state |
| `modelReady` | registry: active model set AND its files on disk |

Explicit skips (accessibility, menubar) persist in
`settings.onboarding_skips` and count as met on resume.

## Live transcription loop (SPEC3)

While recording with `live_transcription` on, a dedicated thread re-runs the
warm engine over a non-destructive snapshot of the audio so far and emits the
raw text to the overlay (`stream-text`). Cadence starts at 1 s and is governed
by `live::LiveInterval` (R10-tested): a pass costing > 60% of the interval
stretches it ×1.5 (capped at 4 s, logged once); fast passes recover it toward
1 s. Slow hardware therefore degrades to fewer updates — never to a broken
recording. **The stop path is authoritative and unchanged**: it takes the full
buffer, waits for the engine lock like any other caller, and runs the same
transcribe → cleanup → enhancement → paste pipeline; live passes can only ever
have been read-only spectators.

Measured live-pass duration on this machine (release, Metal): Whisper tiny
over the 11 s jfk.wav fixture = **97–211 ms** warm (the same numbers as I1) —
comfortably inside the 1 s base cadence; the 60% guard would not engage until
a pass exceeded ~600 ms, i.e. roughly a minute of accumulated audio at tiny's
throughput.

Flicker control lives in the frontend: `src/lib/liveText.ts` (U6-tested)
promotes words that two consecutive passes agree on (case/punctuation-
insensitive) to `stable` — stable text never shrinks and never rewrites; the
disagreeing tail renders dimmed as `tentative`.

## Overlay appearance (SPEC6)

The recording visualization is a canvas driven by `EffectEngine`
(src/overlay/effects/engine.ts) through a deliberately small renderer
interface — `init(ctx, w, h)` / `render(ctx, frame)` / `dispose()` with
`frame = { level, levels, time, dt, colors, reducedMotion, width, height }`.
15 built-ins live in src/overlay/effects/, one module each; the registry
falls back to `classic-bars` on unknown ids. (One post-SPEC6 swap: the
`vu-needle` analog meter read poorly in the wide, short pill — radial shapes
waste the horizontal canvas — so `heartbeat`, an EKG-style scrolling trace,
replaced it; saved `vu-needle` selections fall back to the default.) Rendering pauses whenever the
overlay is hidden (engine.stop on state change) and every renderer receives
`reducedMotion` to calm itself.

Colors are the theme's job: renderers draw exclusively with
`frame.colors.{primary,accent,glow}`, resolved from the `--nh-fx-*` CSS
variables on the pill root — U8 enforces "no hardcoded draw colors" at the
source level. A theme is the 11-variable contract in THEMES.md applied via
the `.nh-theme` class; 12 built-ins ship in src/overlay/themes/, and user
themes are drop-in .css files (size-capped, remote url() rejected —
src-tauri/src/themes.rs, R11) loaded through `list_user_themes`.

**User JS/TS effects are deferred deliberately**: user code in the overlay
webview needs an IPC-less sandbox and a frozen API. The renderer interface
above IS that API surface — if plugins ever land, they implement the same
contract inside a sandboxed frame, and nothing here changes.

## Focus guard (SPEC7)

Never paste into the wrong app silently. `focus.rs` is the platform boundary
for frontmost-application queries: on macOS it asks NSWorkspace (raw
`msg_send!`, marshalled to the main thread like tray_probe); the non-mac stub
returns `None`, which disables the guard entirely. Since SPEC10 the Windows
branch is real: `GetForegroundWindow` → `QueryFullProcessImageNameW`, with
the lowercased executable path as the comparison key (`windows_exe_key`,
pure and R14-tested); every failure path stays `None`.

The flow: `start_recording` captures the frontmost app (the overlay panel is
non-activating, so frontmost-at-hotkey IS the paste target) into
`AppState::dictation_start_app`. After STT + cleanup + enhancement —
immediately before delivery, exactly the window where users ⌘-tab away — the
pipeline re-reads the frontmost app and feeds both into the **pure decision
function** `focus::decide(enabled, output_method, started, current)` (R13
table-tests it). Same bundle id, any `None`, clipboard-only output, or the
setting off → deliver as always. Mismatch → the transcript parks in
`AppState::pending_paste`, the overlay shows the `focus-changed` prompt, and
**the pipeline thread returns to its message loop** (`AwaitFocusConfirm`
stage) — it never blocks on the human. Resolution arrives as ordinary
pipeline messages: the overlay buttons (`resolve_focus_prompt` IPC), the
dictation hotkey (= paste), Esc (= dismiss), a 10 s timeout or a preempting
new dictation (= copy + notify). History and last-transcription are recorded
*before* the decision, so every outcome keeps the text. The confirmed paste
re-checks nothing — the user just pointed at the target.

Sound cues (`sounds.rs`, same platform-boundary pattern) and the recording
tray dot are the other SPEC7 surfaces; cue playback is spawn-and-forget
(`afplay`), never on the dictation path's critical timing. (Post-SPEC7
divergence: FR-T1's red non-template dot is gone — macOS 26 wraps the
recording app's status item in its own orange privacy capsule and
substitutes a generic glyph for non-template icons, so all tray states are
template now; the dot survives in the alpha and renders white inside the
capsule.)

## Release pipeline (SPEC8)

The app version lives in exactly three files — `package.json`,
`src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` — moved only in
lock-step by `npm run release:prepare -- <semver>` (pure rewrite transforms,
U12-tested; pre-release ids like `-alpha.1` are never stripped; lockfiles
refresh alongside). Tags mirror the files.

`.github/workflows/release.yml` topology: a tag push (`v*`) or a
`workflow_dispatch` dry-run → **test** (macos-latest: version-drift check,
then the full `npm run validate` gate, with the I1 model cached via
actions/cache) → **build-macos** (Apple Silicon dmg) → **release** (checksum
file + one **draft** GitHub Release, exactly two assets, 80 MB guard).
Publishing is always a human flipping `--draft=false` after smoke-testing —
see docs/RELEASING.md.

Licensing guard: `npm run licenses` regenerates THIRD-PARTY-NOTICES.md from
`package-lock.json` + `cargo metadata` and fails on any license outside the
permissive allowlist (U13-tested, including the tauri-nspanel
missing-metadata override) — copyleft can't enter the bundle unnoticed.

Updater (SPEC9): **Check for Updates…** (app menu + tray) shows the
settings window and emits `check-updates`; the dialog drives the
`src/ipc/updates.ts` seam — `tauri-plugin-updater`/`-process` on desktop
(all network Rust-side, user-initiated, minisign-verified against the
pubkey in tauri.conf.json), a `window.__yyUpdate` mock in the browser
(E15a/E15b). Release builds emit signed `.app.tar.gz` updater artifacts and
a `latest.json` composed by `scripts/updater-manifest.mjs` (pure core,
U14); publishing a release advances the rolling `updater` release that the
fixed endpoint points at (`updater-manifest.yml`).

## Uninstall & eggs (SPEC11)

`uninstall.rs` splits uninstall into a pure, R15-tested plan
(`uninstall_plan` — the exact items with sizes, mirroring
scripts/deep-clean.sh; drift between the two is a bug) and best-effort
execution: autostart off → delete plan items → `tccutil reset` for our own
Accessibility/Microphone entries → the bundle goes to the **Trash** (never
a hard delete of a running app) → exit. Windows hands off to the NSIS
uninstaller, whose `windows/hooks.nsh` POSTUNINSTALL hook clears app data.

The easter eggs (dance wiggle, Konami → Yat95, sleepy waveform) are
cosmetic-only by contract: pure helpers in `src/lib/eggs.ts`
(U16–U18-tested), CSS animations that respect prefers-reduced-motion, and
zero contact with the recording/cleanup/paste paths. The Yat95 theme lives
outside the built-in registry (`secret:yat95` in
`src/overlay/secretTheme.ts`) so the 12-theme picker contract stands.

SPEC12 divergence from SPEC11 §5.1: the wiggle egg's trigger is **"dance"**,
not "yat yat" — Whisper never transcribes the non-word "yat" (measured with
the release CLI on synthesized speech: it hears "that you add" / "that yet" /
"you're at"), so the documented trigger could never fire. The wiggle also
reaches the menu-bar icon now: `wiggle_tray` (mirrored in the mock) steps
the tray through pre-rotated template frames from `scripts/gen-icons.mjs`
and restores the pipeline's current state icon, ignoring re-entrant calls.
A single `easter_eggs` setting (default ON, R17) gates all three eggs; the
overlay re-reads it at each recording start, and the command re-checks it
server-side.

## Performance pass (SPEC14)

A code-level review (2026-07-18) found the app slower and heavier than its
measured components justified; SPEC14 removed the costs with zero
user-visible behavior change. What moved, and where it lives now:

**Stop→paste fixed-sleep budget: ≈210 ms → ≤60 ms.** The old path slept a
fixed 60 ms to flush the audio callback, 50 ms for the clipboard, and
100 ms holding the paste modifier. Now: `audio.rs::drain_pending` (R22)
stops the stream first, then drains until the channel is quiet 15 ms
(hard cap 60 ms — typically returns in ≤15 ms); `paste.rs` uses 20 ms
settle + 20 ms hold (named constants — bump per-app in BLOCKERS.md if a
timing-sensitive target ever drops pastes, never silently).

**Capture is 16 kHz end-to-end** (`audio.rs::StreamResampler`, R24):
chunks feed a persistent rubato resampler as they arrive, in the same
1024-frame batches the old whole-buffer path used, with the tail
zero-pad-flushed at stop. Live-mode snapshots became plain copies — the
old path cloned the whole device-rate buffer AND re-resampled all of it
every 1–4 s (O(n²) over a recording; a 5-min 48 kHz recording cloned
~58 MB per pass). `AppState::stop_pending` makes an about-to-start live
pass yield the engine to the stop path instead of queueing a full
re-transcription ahead of it. Cancel releases the capture allocation
(R25).

**The engine pre-warms** (`AppState::ensure_loaded`): background load at
Ready when the active model is on disk, after `set_active_model`, and
belt-and-braces at recording start — the 4.8 s cold load can no longer
land between "stop" and "paste". The loader holds the engine mutex, so a
stop mid-load waits exactly as the lazy path always did, just with a
head start.

**Nothing writes or rebuilds on the delivery path**: the retained WAV
(now 16-bit PCM, half the size) is written on a background thread whose
join handle Retry awaits — never a partial file; history.json,
`history-changed`, and the tray-menu rebuild are deferred to a
background thread; rtf.json persistence is debounced to return-to-idle /
exit (`progress.rs::RtfStore`, R23 — live passes used to write it every
1–4 s); tray icons are decoded once into a cache and the menu rebuilds
on change instead of on every transition.

**Idle is actually idle**: the audio worker blocks on its channel when no
stream is active (was 100 wakeups/s forever); the settings webview is
created lazily on first use (`show_settings_window`) instead of at every
launch — deep links ride the window URL query (`?section=…`,
`?updates=1`) because an emit into a just-created webview races listener
registration — and every poll in it gates on `visibilitychange`
(capture-dead 2 s, onboarding 1 s, Appearance preview 25 Hz, history
refresh); onboarding ticks re-verify only the displayed step's facts
(`factsForStep`, U21 — the tray window-server probe runs on the menubar
step alone).

**The overlay renders what the data justifies**: the effect loop is
frame-capped at ~60 fps (30 under reduced motion) with a dt-based decay
(U20 — the look is refresh-rate-independent now; it used to decay 4×
faster on ProMotion), reuses one frame object, indexes level history
instead of slicing per frame, compacts particles in place, and fireflies
draws pre-rendered glow sprites instead of 16 shadowBlur passes per
frame. `easter_eggs` rides the show-overlay payload (E20a — no
get_settings round-trip per recording) and `applyTheme` memoizes the
applied theme id (U22/E20b — no style recalc or user-theme fetch on a
same-theme show).

**One HTTP/TLS stack**: app reqwest moved to 0.13 + rustls
(`rustls-platform-verifier` — still the OS trust store), the exact stack
tauri-plugin-updater links, dropping the duplicate 0.12/native-tls
tower. The network guarantee is untouched: user-initiated model
downloads and the localhost-only enhancement endpoint remain the ONLY
network paths. `crate-type` is `["rlib"]` (the mobile template's
staticlib/cdylib each paid a full extra LTO link); each webview
lazy-loads only its own app bundle chunk.

## Transcription progress (SPEC13)

The transcribing overlay shows an **estimated** progress fill — transcribe-rs
exposes no progress callback for either engine (revisit if the crate ever
grows one), so `src-tauri/src/progress.rs` (pure, R18-tested) estimates:
expected STT time = audio seconds × a per-model real-time factor kept as an
EMA (`AppState::rtf`, seed 0.5, alpha 0.3, observed from raw engine wall
time only — never model load, cleanup, or enhancement). Three divergences
from SPEC13 §1 (field-measured: the 0.5 seed is ~50× slow for Apple-Silicon
Metal, so the fill crawled to ~10% and snapped): the first real measurement
**replaces** the seed instead of blending (R18); the map is **disk-backed**
(`<data dir>/rtf.json`, R21 — corrupt/missing reads as empty) so relaunches
start calibrated; and the observation lives inside `AppState::transcribe`,
so **live-transcription passes calibrate the estimate mid-recording** —
even a first-ever dictation gets an accurate fill when live mode is on.
While the blocking
STT call runs, `finish_recording` spawns a ticker thread emitting
`transcribe-progress` (bare fraction, like `mic-level`) every 100 ms; its
stop flag is cleared on every exit path before the overlay changes state,
and only completion emits 1.0 — the curve itself tops out at 0.95. The
overlay sweeps a `fill` class across the existing thinking-shimmer bars
(forward-only via `src/overlay/progress.ts`, U19; session-scoped; theme
variables only; a class change, not an animation, so it survives
reduced motion). With enhancement enabled the bar crawls near the cap
during the LLM round-trip — expected, not a bug.

## Design notes

- **One pipeline thread** serializes Idle→Recording→Processing, so double
  hotkey presses, Esc, menu clicks, and the 5-minute watchdog can't race.
- **The overlay never takes focus**: NSPanel with `nonactivating_panel` +
  `can_become_key_window: false`; levels are throttled and skipped entirely
  while hidden (hidden WebKit views still pay for every event).
- **Paste runs on the main thread** (macOS requirement), but the previous-
  clipboard restore does NOT: `restore_clipboard_later` (R20) fires ~300 ms
  after ⌘V on its own thread and only if the clipboard still holds the
  pasted text. A blocking restore starved our own webview of the queued ⌘V
  (the onboarding try-box pasted the OLD clipboard — the target app must
  get to process the keystroke before the restore lands). Missing
  Accessibility degrades to clipboard-only plus a notification.
- **Ready-time activation is launch-shape-aware** (`ready_activation`, R19):
  the windowless tray-only launch gets the dock-tile nudge's
  activate-and-hand-back (macOS 26 hidden-flag workaround), but a first-run
  launch with the visible wizard activates and KEEPS focus — the hand-back
  used to deactivate the app ~1 s in, making the wizard's first button need
  an extra click just to re-activate the window.
- **The mock IPC shim** (`src/ipc/mock.ts`) mirrors `commands.rs` event-for-
  event, which is what lets Playwright drive the real settings/overlay UI in a
  plain browser.
- Patterns for the hard parts (bare-modifier hotkeys, non-activating overlay,
  clipboard restore timing, download state machine) were adapted from
  [Handy](https://github.com/cjpais/Handy) (MIT), with thanks.
