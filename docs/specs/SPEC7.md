# SPEC7: Yat Yat — focus guard, personal dictionary, sound cues, red tray dot

An increment over SPEC.md–SPEC6.md (authoritative elsewhere). Four features:
a **focus-change paste guard** (never paste into the wrong app silently), a
**personal dictionary** (user-defined always-apply replacements), **sound
cues** (audible start/delivered confirmation), and a **red recording dot** in
the menu bar. All fully local; no new network access, no new permissions.

## 1. Focus-change paste guard (FR-G)

The problem: dictation starts with focus in Terminal, the user switches to
Slack while talking (or while a slow enhancement pass runs), and the
transcript pastes into Slack. The guard detects the switch and asks first.

- **FR-G1 (capture).** When recording starts, capture the frontmost
  application — bundle id + localized name — via `NSWorkspace`
  (objc2/objc2-app-kit). The overlay is a non-activating panel, so the app
  frontmost at hotkey time IS the paste target. Keep the capture behind the
  platform-abstraction boundary (ARCHITECTURE.md): a `focus` module with a
  `frontmost_app() -> Option<FrontmostApp>` signature; the non-macOS stub
  returns `None`. `None` at either end disables the guard for that dictation
  (fail open — never block a paste on an API hiccup).
- **FR-G2 (check).** Immediately before delivering — i.e. after STT, cleanup,
  AND enhancement, at the top of the paste step — re-read the frontmost app
  and compare **bundle ids** (same app, different window/tab = no prompt).
  Paste proceeds unprompted when: ids match, either read returned `None`,
  the effective output method is clipboard-only, or the setting is off.
- **FR-G3 (prompt).** On mismatch, do NOT paste. Show a new overlay state
  `FOCUS_CHANGED`: "Started in **{start}** — paste into **{now}**?" with two
  buttons, **Paste** and **Copy only**, plus the existing Esc-cancels
  convention. Clicks must not steal focus (non-activating panel — clicking
  Paste must land the text in the app the user is looking at). Pressing the
  dictation hotkey while the prompt is up = confirm Paste. The decision
  logic (given start-app, current-app, output method, setting → Deliver |
  Ask) MUST be a pure function so R13 can table-test it.
- **FR-G4 (never lose text).** Whatever the user picks — including Esc and
  the timeout — the transcript is stored as the last transcription and in
  history exactly as today. **Timeout:** if the prompt sits unanswered for
  10 s, fall back to copy-to-clipboard, notify ("Focus changed — copied to
  clipboard instead"), and hide. A new dictation started while a prompt is
  pending resolves the pending one as the timeout does (copy + notify).
- **FR-G5 (pipeline discipline).** The pipeline thread must not block
  waiting on the human. Hold the pending text in `AppState`, return the
  pipeline to Idle, and let the overlay's answer (an IPC command) trigger
  delivery on the main thread through the existing `Paster` path. The
  pending paste re-checks nothing — the user just confirmed the target.
- **FR-G6 (setting).** `focus_guard: bool`, default **true**, toggle in
  Settings → General ("Ask before pasting into a different app").

## 2. Personal dictionary (FR-D)

User-defined replacements applied to every transcript — names, jargon,
products that STT reliably mangles ("jorge pereira" → "Jorge Pereira",
"yat yat" → "Yat Yat").

- **FR-D1 (data).** Settings field `dictionary: Vec<DictionaryEntry>` where
  `DictionaryEntry { from: String, to: String }`. `#[serde(default)]`-safe
  (legacy settings.json loads with an empty list). Caps: 200 entries,
  `from`/`to` ≤ 100 chars each; entries beyond the caps are ignored with a
  log warning, never a crash.
- **FR-D2 (matching).** Applied inside `cleanup::clean()` after filler
  stripping and repeat collapsing, so enhancement (when enabled) sees the
  corrected text. Case-insensitive, whole-word-boundary match on the full
  `from` phrase (multi-word phrases supported); replacement text is inserted
  literally. Entries apply in list order, each over the whole text; an empty
  or whitespace-only `from` is skipped. No regex syntax — `from` is a
  literal phrase (escape it before matching).
- **FR-D3 (UI).** Settings → Cleanup grows a "Personal dictionary" table:
  two-column rows (Heard → Replace with), add row, delete row, edits persist
  via the existing set_settings flow. A row with an empty "Heard" cell is
  not saved. Keep the visual language of the existing Cleanup section.
- **FR-D4 (reach).** Because it lives in `clean()`, it automatically covers
  live-final divergence rules, Retry Last Transcription, and both output
  methods. The live overlay preview text is NOT required to apply it.

## 3. Sound cues (FR-C)

Ambient audio confirmation for eyes-off dictation: a soft tick when
recording actually starts, a soft click when text is delivered (pasted or
copied). No sound on cancel, errors, or the nothing-heard path.

- **FR-C1 (sounds).** Two short bundled sounds (≤ 0.3 s, ≤ 50 KB total,
  quiet by design — generated/CC0, committed as app resources). Playback on
  the default output device; implementation is the implementer's choice
  (NSSound via objc2, rodio, or spawning `afplay` are all acceptable) but it
  MUST be non-blocking and MUST NOT add measurable latency to the
  hotkey→overlay or stop→paste paths (play async, never `sleep`).
- **FR-C2 (placement).** Start cue fires when the recorder actually starts
  (after `recorder.start()` succeeds, not on hotkey press); delivered cue
  fires after `Paster::deliver` returns, for both paste and clipboard-only
  outcomes, including the focus-guard "Copy only"/timeout resolutions.
- **FR-C3 (setting).** `sound_cues: bool`, default **false**, toggle in
  Settings → General ("Play sounds when recording starts and text is
  delivered"). Behind the platform boundary; non-macOS stub is a no-op.

## 4. Red recording dot in the menu bar (FR-T)

The tray already swaps icons per state (Idle/Recording/Processing) — but all
three are monochrome templates, so "recording" is easy to miss. Make it an
unmistakable ambient indicator.

- **FR-T1.** The Recording tray icon renders its dot in **red** (system red,
  ≈ #FF3B30). This requires the recording icon to be a non-template (color)
  image; Idle and Processing stay template so they keep adapting to menu-bar
  appearance. The mic glyph in the recording icon must stay legible on both
  light and dark menu bars (mid-gray glyph + red dot is acceptable;
  implementer has latitude on the exact art).
- **FR-T2.** State swaps continue to ride the existing `tray::set_state`
  path — recording red appears when the recorder starts and clears when
  processing begins. No new states.

## 5. Settings summary (FR-S)

New fields, all `#[serde(default)]`-safe on `Settings` (legacy files load
without loss): `focus_guard: bool = true`, `sound_cues: bool = false`,
`dictionary: Vec<DictionaryEntry> = []`. Existing fields are untouched;
saving any setting must not disturb `active_model` (regression covered by
existing R-suite).

## 6. Tests

- **R12 (dictionary):** `clean()` with dictionary entries — case-insensitive
  whole-word replacement; multi-word phrase; no partial-word match ("cat"
  never fires inside "catalog"); literal (non-regex) `from` containing `.`
  or `(` is treated literally; list-order application; empty `from` skipped;
  caps enforced (201st entry ignored); empty dictionary is a byte-identical
  no-op on the input.
- **R13 (focus-guard decision):** table-test the pure decision function —
  match → Deliver; mismatch → Ask; `None` on either side → Deliver;
  clipboard-only → Deliver; setting off → Deliver. Plus settings: legacy
  JSON without the new fields loads with documented defaults.
- **U10 (dictionary UI):** the Cleanup table renders existing entries;
  add/edit/delete rows round-trip through the mocked set_settings; a row
  with empty "Heard" is not persisted.
- **U11 (focus overlay state):** the `FOCUS_CHANGED` overlay state renders
  both app names and both buttons; Paste and Copy only invoke the right IPC
  commands through the mock.
- **E14a (guard flow):** mocked-IPC e2e — overlay driven to `FOCUS_CHANGED`
  with app names in the payload shows the prompt; clicking Paste emits the
  confirm command; Esc emits cancel; the recording UI is fully restored for
  the next dictation.
- **E14b (settings toggles):** Settings → General shows the focus-guard and
  sound-cues toggles with the specified defaults; flipping each persists
  via the mock and survives reload.
- **E14c (dictionary e2e):** add two dictionary rows in Settings → Cleanup,
  reload, both persist; delete one, the other survives.
- Frontmost-app capture, actual audio playback, and the red tray dot are
  macOS-runtime behaviors — covered by the manual checklist in the DoD, not
  faked in unit tests. The decision function and all UI around them are
  covered above. Existing suites must keep passing unweakened.

## 7. Definition of Done

1. `npm run validate` exits 0 with complete output — all existing suites
   plus R12–R13, U10–U11, E14a–c — ending `VALIDATION: ALL PASSED`, printed
   in the transcript.
2. `npm run tauri build` exits 0; .app path + size (< 80 MB) printed.
3. `grep -rn ".skip\|.only\|.todo" tests/` prints nothing.
4. README: a "Personal dictionary" paragraph under cleanup, the focus guard
   described in "How it works", sound cues + both new toggles named in the
   settings coverage, and the red recording dot mentioned where the menu-bar
   icon is described.
5. ARCHITECTURE.md: a short "Focus guard" note — where the capture lives on
   the platform boundary, the pure decision function, and the pending-paste
   handoff (pipeline never blocks on the prompt).
6. Anything infeasible → BLOCKERS.md; never game a check.

Manual checks after green: start dictation in Terminal, ⌘-tab to another
app mid-sentence, stop — the prompt names both apps; Paste lands in the
front app without focus flicker; Copy only + timeout paths leave the text
on the clipboard and in history. Add "yat yat → Yat Yat" to the dictionary
and hear it corrected in a real dictation. Toggle sound cues on and confirm
tick/click with no added lag. Watch the menu bar turn red while recording.
