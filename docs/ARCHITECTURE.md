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
| `chunking.rs` | Pure pause-finding + overlap stitching for chunked transcription | R28–R30, R32–R33 |
| `stt.rs` | Engine abstraction (Whisper/Parakeet), `load_path` for CLI | exercised by I1 |
| `downloader.rs` | Resume-able downloads, SHA-256 verify, tar.gz extract, multi-file models | sha/cancel tests, R34–R35 |
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

Post-SPEC3 divergence: `live_transcription` defaults **OFF** as of
0.1.0-alpha.10 (owner decision, 2026-07-18 — SPEC3 FR-L1 shipped it on by
default; E10c was updated to match). The feature itself is unchanged —
users opt in from Settings → General.

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
29 built-ins live in src/overlay/effects/, one module each (13 arrived with
SPEC16's cinema modes, one more with the post-SPEC16 valley swap); the
registry falls back to `classic-bars` on unknown ids. (One post-SPEC6 swap: the
`vu-needle` analog meter read poorly in the wide, short pill — radial shapes
waste the horizontal canvas — so `heartbeat`, an EKG-style scrolling trace,
replaced it; saved `vu-needle` selections fall back to the default.) Rendering pauses whenever the
overlay is hidden (engine.stop on state change) and every renderer receives
`reducedMotion` to calm itself.

Colors are the theme's job: renderers draw exclusively with
`frame.colors.{primary,accent,glow}`, resolved from the `--nh-fx-*` CSS
variables on the pill root — U8 enforces "no hardcoded draw colors" at the
source level. A theme is the 11-variable contract in THEMES.md applied via
the `.nh-theme` class; 26 built-ins ship in src/overlay/themes/, and user
themes are drop-in .css files (size-capped, remote url() rejected —
src-tauri/src/themes.rs, R11) loaded through `list_user_themes`.

Post-SPEC16 swap (owner decision, same pattern as vu-needle→heartbeat): the
`helix` mode (dna-helix + the `isla-amber` theme) was replaced by
`the-valley` — a cozy pixel-farm daytime look pairing the new
`pixel-valley` effect (chunky clouds over the sky-colored pill background,
pixel hills rolling with the voice history, crops sprouting on strong
syllables) with the new light `the-valley` theme. That took the effect
count to **29** (owner-sanctioned count amendment; themes hold at 26). The
`dna-helix` effect stays in the catalog; saved `isla-amber`/`valley-night`
selections fall back to the default theme.

**Cinema modes (SPEC16)**: 14 riff-named presets in `src/overlay/modes.ts`,
each a designed pair of one effect and one theme. A mode is derived
presentation sugar — clicking a gallery card writes `overlay_effect` +
`overlay_theme` in a single save, and the pure `modeFor(effect, theme)`
derives selection (diverging via either à-la-carte picker deselects the
card). No `overlay_mode` field exists anywhere; the effect contract above
carried no changes (mode effects are level-driven, recording-time
renderers like every other). The Appearance page stays one scrollable
surface: a sticky Modes/Effects/Themes segmented control scroll-anchors
the sections, cards are static swatches, and the single real-engine
preview retargets on card hover (preview-only; settings write on click).

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

## Post-update recovery (SPEC15)

Release builds are **ad-hoc signed** (`signingIdentity: "-"`), so every
release carries a different code signature — and macOS keys the
Accessibility (TCC) grant to that signature. The failure chain after an
in-app update relaunches: new signature → the old TCC grant no longer
matches (System Settings still shows a stale checked entry) →
`check_accessibility()` is false → `init_capture` defers forever → the
hotkey never arms, silently. SPEC15's answer is detection, not repair (TCC
is user-only by design): `launch_recovery` in `src-tauri/src/lib.rs` — a
pure decision function (plain bools in, `None | Recover { after_update }`
out; R26 drives the matrix) — runs **once per process** at
`RunEvent::Ready`, right after the FR-S6 `init_capture` attempt, so a
healthy grant arms first and decides `None`. It recovers only when macOS ∧
onboarding complete ∧ grant dead ∧ the accessibility gate wasn't
deliberately deferred in `onboarding_skips`; the 3 s capture watcher never
re-fires it (no nag loop). On `Recover` it posts a best-effort notification
(`after_update` distinguishes "the update re-keyed Accessibility" from
generic missing-permission wording) and shows the settings window, whose
capture-dead banner → "Fix in setup" → wizard accessibility gate is the
existing recovery UI. `after_update` comes from `last_run_version`, a
server-owned settings field (preserved across whole-object UI saves via
`preserve_server_owned`, R27) stamped by Rust each launch *after* the
decision reads the previous value. The update dialog is honest about all
this up front: no dismissing mid-download, a `ready`-phase macOS warning
about the re-key, restart failures surfaced in the error phase (U23,
E21a–E21b). The definitive structural fix — Developer ID signing +
notarization, whose TCC grant survives updates — is out of scope here and
tracked for a future spec.

## Model sources: the `files` kind (post-SPEC16)

Owner-requested directly, without a SPEC/GOAL pair (2026-11-19). The
registry gained a third `kind`, `files`: `url` is a base ending in `/`,
and `files` lists each file's name, SHA-256 and size (R36 validates names
as plain file names — they become paths). The entry's own `sha256` is the
digest of that list (`registry::files_digest`), so the entry still pins
its exact contents. The downloader fetches each file to `<name>.partial`
(each resumable), verifies all of them, and only then renames them into
place; `is_downloaded` for this kind means every listed file exists
(R34–R35). Progress events cover the whole model, so the UI and
`src/ipc/mock.ts` flow are unchanged.

Parakeet V3 is the first user: it now downloads from
`istupakov/parakeet-tdt-0.6b-v3-onnx` on Hugging Face at a pinned commit
instead of the Handy project's `.tar.gz` of the same files (verified
byte-identical, so folders installed from the archive stay valid). The
download is 671 MB instead of 479 MB — the archive was compressed, the
repository's files are not. The `archive` kind stays for future use.

## Full-precision Parakeet (post-SPEC16)

Owner-reported (2026-11-19): a 20 s dictation came back as one word. The
int8 Parakeet encoder skips whole sentences on some recordings, and which
ones depends on how much audio it is handed: prefixes of that recording
(8, 10, 12, 14, 16, 18, 20 s) gave 15, 0, 36, 25, 53, 15 and 45 words.
The same windows through the unquantized (FP32) files gave 24, 24, 36,
45, 53, 56 and 56. Swapping halves showed the encoder is responsible
(int8 encoder + FP32 decoder still drops; FP32 encoder + int8 decoder
mostly does not). The plain whole-recording path is affected too — 45
words where FP32 hears 56 — and anything that makes more, shorter passes
(chunking, live previews) is affected more.

The catalog therefore offers `parakeet-tdt-0.6b-v3-full`: the FP32 files
from the same pinned repository commit (R44). `ModelEntry.precision`
(`int8` when absent) tells the loader which file names to expect. Cost
on an M5: a 2.5 GB download instead of 0.67 GB, about 0.9 s instead of
0.8 s for 30 s of audio in one pass, and roughly 0.8 GB more memory. The
int8 model stays the default download; making the full one the default
is an owner decision.

## Chunked transcription while recording (post-SPEC16)

Owner-requested directly, without a SPEC/GOAL pair (2026-11-19) — this
section is its record. It amends the SPEC3/SPEC14 statement that the stop
path "takes the full buffer": with Parakeet and live mode off, it now takes
only the tail.

**Why.** The stop path handed the whole recording to the engine after the
user stopped, and Parakeet's cost grows faster than the audio (M5, release,
int8 ONNX on CPU): 35 s → 0.95 s, 70 s → 2.1 s, 140 s → 5.1 s. A field
log confirmed the app matched the CLI: 28 s of speech, `stop -> delivered
in 817 ms`. The wait under the progress fill was that single pass.

**What.** While recording, `pipeline.rs::chunk_loop` polls every 250 ms,
asks the pure `chunking::next_chunk` for a cut, and transcribes the
finished stretch through the warm engine. At stop, `finish_recording`
stops the chunker (waiting out a pass in flight — that audio is owed
either way), transcribes only the audio after the last cut, and runs the
unchanged cleanup → enhancement → focus guard → paste path. A recording
with no cut takes the old whole-buffer path, byte-for-byte.

**Cut rules** (`find_cut`, R28–R29): never under 4 s; the longest pause of
≥ 500 ms (≥ 250 ms once 15 s are pending), cut mid-pause; past 30 s of
unbroken speech, the quietest 200 ms. Silence is relative to the stretch's
own loud level, so no calibration and no VAD model.

**Why overlap + stitching** (R32–R33). A chunk transcribed alone gets a
capital on its first word and a period on its last regardless of the
sentence (measured on real dictation: a capital and a full stop appeared
mid-sentence at every cut). So each pass also gets 2 s of audio before its chunk and 1.5 s
after, and consecutive passes are stitched by aligning the words both
heard in the overlap (LCS on normalized text + time proximity); a word
only one pass heard is kept if that pass owns its side of the cut. Two
engine behaviors shaped this, both measured: word **end** times are
useless (a word "lasts" until the next one starts, so the word before a
4 s pause ends 4 s later — position is judged by start time only), and a
pass often **hallucinates at a truncated window end** (several invented
words, all stamped with the window's last instant), which is exactly the region the next pass overrides.

**Scope.** Parakeet only: Whisper pays a fixed ~30 s encoder window per
pass (≈ 0.85 s on the same machine for a 2.5 s clip), so extra passes
would cost more than they save. Off when `live_transcription` is on —
live passes already own the engine during recording.

**Measured** (same machine, `yat-yat transcribe --chunked`, which replays
the live rules over a WAV):

| Recording | Whole-buffer pass (old wait) | Tail after stop (new wait) | Passes during recording |
| --- | --- | --- | --- |
| 30.5 s real dictation | 0.91–1.13 s | 0.16–0.19 s | 5 |
| 35 s synthesized | 0.92–1.55 s | 0.27–0.29 s | 3 |
| 140 s synthesized | 5.2–7.7 s | 0.27–0.28 s | 15 |

Background passes cost about 5 % of recording time in CPU (7.2 s over
140 s). Text versus the whole-buffer pass: same words on the synthesized
clips (punctuation and hyphenation vary the way the engine's own repeats
do); on the real dictation no dropped or duplicated words, two word
choices changed (both arguably more correct than the whole-buffer
pass) and one comma became a period at a 1.2 s
pause. One real recording is a thin sample — treat join quality as
field-unproven.

## Parakeet noise floor (post-SPEC16)

Every Parakeet pass reads the audio through `stt::with_noise_floor`, which
adds a faint fixed-seed noise (peak 0.0025 of full scale, about -60 dB RMS).

Why: the quantized (int8) encoder skips whole sentences on some clean
recordings, and what it skips changes with the length of audio it is given.
On one 20 s dictation (56 words spoken) it returned 45 words for the whole
recording, 0 for the first 10 s and 15 for the first 18 s. Whole-recording
transcription lost a fifth of the text; chunked transcription, which runs
many shorter passes, delivered 6 words. With the noise floor the same passes
return 56, 30 and 56 words and the chunked path returns 56 — the same as
the full-precision weights give without it.

Measured on the failing recording and five others. On the five others the
whole-recording word count moved by at most one word (four were unchanged
word for word) and the chunked count by at most two. A third of the noise did not fix the failing recording; ten
times as much did no harm. Why the encoder fails on that recording is not
established — the fix is empirical. Test: R45.

## Live transcription on the chunker (post-SPEC16)

Owner-requested directly (2026-11-19). The SPEC3 live loop re-reads the
whole recording on every pass, so on Parakeet a pass grew from 0.26 s at
10 s of speech to 5.2 s at 140 s — the on-screen text fell seconds
behind, and the stop path could queue behind a pass in flight. Live mode
also switched chunking off, so stop paid a whole-recording pass again.

On Parakeet the chunker thread now does both jobs (`pipeline::chunk_loop`
with `live`): it commits finished stretches as before, and between cuts
it previews the audio since the last cut — one pass over that window,
stitched onto the committed words for display only
(`chunking::preview_text`, R42). Previews rest `live::preview_gap` after
each pass (R41: 150 ms floor, never less than the pass itself), and the
loop polls every 100 ms in live mode. Whisper keeps the
SPEC3 loop: its cost per pass is a fixed window, not the audio length.

Shown raw, previews flicker: a window that ends mid-phrase makes the
engine invent an ending ("…we can't be able to do it.") that the next
preview takes back. `chunking::PreviewFilter` (R43) therefore sends a
preview only up to the last word the previous preview also had; committed
words are always sent. On the overlay side, `liveText.ts` realigns the
tentative tail when a pass re-words stable text ("third party" →
"third-party"), which used to repeat or swallow a word (U25).

Replay (`transcribe --live`, M5): the app's loop in audio time, a preview
every 0.5 s, each one timed against re-reading the whole recording so far.

| Recording | Whole so far, worst | Since last cut, worst | Engine time, old → new |
|---|---|---|---|
| 30.5 s real dictation | 835 ms | 401 ms | 25.5 s → 11.2 s |
| 35 s, few pauses | 1404 ms | 597 ms | 34.2 s → 20.7 s |

How far the screen trails the voice (replay: time since the newest
shown word was spoken, median): 1.8 s / 1.5 s on the two recordings with
a 400 ms floor and 250 ms poll, 1.6 s / 1.2 s with 150 ms and 100 ms —
for about 70% more engine time during recording. The rest is structural:
the model does not settle on a word until it has heard what follows, and
a word is shown only once a second preview confirms it.

The gap widens with length: the old cost grows with the whole recording
(5.2 s per pass at 140 s), the new one only with the time since the last
pause. Feeding the replayed previews through the overlay's stabilizer,
the text on screen ends within two or three words of the final
transcript on both recordings.

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
