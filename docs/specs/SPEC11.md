# SPEC11: Yat Yat — Uninstall, sound-cues default, and three easter eggs

An increment over docs/specs/SPEC.md–SPEC10.md (authoritative elsewhere).
Two halves. First: deleting the .app leaves everything the app ever created
— settings, history, **the downloaded models (up to ~2 GB)**, TCC grants,
preferences, WebKit storage, caches, saved state, the login item.
`scripts/deep-clean.sh` scrubs all of it for developers; end users get
nothing. This spec ships a user-facing **Uninstall Yat Yat…** that removes
everything removable, itemized and consented, then puts the app itself in
the Trash. Second: **delight** — sound cues turn on by default, and three
easter eggs land. Iron rule for every egg: **cosmetic only** — none may
ever touch recording, transcription, cleanup, or paste behavior, and all
motion respects prefers-reduced-motion.

Out of scope: uninstall telemetry/surveys (never), removing the two things
macOS offers no API for (the Menu Bar allowance toggle and a user's Dock
pin — the dialog names them honestly), Linux.

## 1. The uninstall plan (FR-P) — pure and testable

1. **`uninstall_plan(data_dir, home, keep_data) -> Vec<PlanItem>`** in a new
   `src-tauri/src/uninstall.rs`: a pure function returning the exact items
   an uninstall would remove — `{ path, kind, bytes }` where `kind` ∈
   `app_data | models | preferences | webkit | caches | saved_state`.
   With `keep_data = true`, `app_data` (settings/history) is excluded but
   `models` still goes (they are re-downloadable and huge). Paths mirror
   `scripts/deep-clean.sh` (the doc comment cross-references it; drift
   between the two is a bug). Sizes are computed by directory walk;
   missing paths are simply omitted.
2. **R15** table-tests the plan against temp dirs: items found with correct
   byte counts, `keep_data` exclusion, missing-path omission, and that
   models under `<data_dir>/models` are classified `models`, not
   `app_data`.
3. Commands: `get_uninstall_plan(keep_data) -> Vec<PlanItem>` (drives the
   dialog) and `uninstall_app(keep_data)` (executes §2). Both mirrored in
   the e2e mock.

## 2. Execution (FR-X)

On confirm, in order, continuing past individual failures (best-effort,
each failure logged — an uninstall must never strand the user half-way):

1. Stop dictation if active; disable launch-at-login via the autostart
   plugin.
2. Delete every plan item.
3. Reset the app's own TCC grants: spawn `/usr/bin/tccutil reset
   Accessibility com.yatyat.app` and `… reset Microphone com.yatyat.app`
   (needs no privileges for the caller's own entries; if it fails, the
   dialog's completion note tells the user the System Settings path).
4. Move the app bundle itself to the **Trash** (NSWorkspace recycle —
   never a hard delete of the running bundle), then exit(0).
5. **Windows**: the in-app button instead launches the NSIS uninstaller
   (`uninstall.exe` beside the installed binary) and exits — Add/Remove
   Programs remains the canonical path. `deleteAppDataOnUninstall` (or the
   current tauri-NSIS equivalent) is enabled in tauri.conf so the NSIS
   uninstaller also clears app data; if no such option exists in our tauri
   version, record it in BLOCKERS.md and scope the Windows button to
   launching the uninstaller.

## 3. UI (FR-U)

1. Settings → General, bottom, beneath Clear history: a **Danger zone**
   row — "Uninstall Yat Yat… Removes the app and everything it stored."
   with an `uninstall-open` button.
2. Clicking opens a modal (`uninstall-dialog`, reuses the `.modal` styles):
   the itemized plan with human-readable sizes (`uninstall-item` rows, e.g.
   "Models — 1.9 GB"), a **keep my settings & history** checkbox
   (`uninstall-keep-data`, re-fetches the plan when toggled), the honest
   un-scrubbables note (Menu Bar allowance, Dock pin), and
   `uninstall-confirm` / `uninstall-cancel` buttons. Confirm shows a brief
   "Removing…" state, then the app quits (macOS) or hands off to the NSIS
   uninstaller (Windows).
3. `formatBytes` (pure, `src/lib/format.ts`) renders sizes — **U15** covers
   it (B/KB/MB/GB boundaries, 0, rounding).

## 4. Sound cues on by default (FR-S)

`sound_cues` defaults to **true** (Settings::default, defaultSettings(),
and the mock). Users who already persisted `false` keep it — serde defaults
only fill missing fields; no migration. Sanctioned test amendments for this
(and ONLY this): `r13_legacy_json_defaults_new_fields` flips its
sound_cues assertion to default-ON, and **E14b** asserts the sound-cues
toggle starts checked. Nothing else about either test changes.

## 5. Easter eggs (FR-E)

1. **"Yat yat" wiggle.** While recording with live transcription, if the
   RAW stream text (pre-cleanup — the collapse hasn't eaten it yet at this
   stage, which is the joke) matches `/yat[\s,.!?]*yat/i`, the pill plays a
   ~600 ms bounce/wiggle (class `pill-wiggle`, removed on animationend).
   At most one wiggle per new match count per recording. Reduced motion: no
   wiggle. Detection is a pure helper (`src/lib/eggs.ts: yatYatMatches`).
2. **Konami code → hidden 13th theme.** In the settings window,
   ↑↑↓↓←→←→BA (pure detector `konamiProgress` in eggs.ts) toggles the
   secret theme **"Yat95"** — Windows-95 gray: `#c0c0c0` pill, navy text,
   beveled-border look, system-ish font. It lives OUTSIDE the built-in
   registry (id `secret:yat95`, its CSS in `src/overlay/secretTheme.ts`)
   so U9's exactly-12 contract is untouched and it never appears in the
   Appearance picker; `applyTheme`/the preview resolve the secret id
   before the built-in fallback. Activating persists
   `overlay_theme: "secret:yat95"` (with a small unlock toast in whatever
   section is open, testid `egg-toast`); the same code toggles back to the
   previous theme. Picking any normal theme simply replaces it.
3. **Sleepy waveform.** If the recording pill has shown no speech for
   **20 s** (level below 0.06 continuously — tracked by a pure
   `SleepTracker` in eggs.ts, threshold/duration exported), the pill gains
   `pill-asleep`: the effect canvas slumps (CSS squash/dim — the engine
   keeps running untouched) and a 💤 (`sleep-zzz`) drifts upward on loop.
   ANY level above threshold wakes it instantly. Recording, live passes,
   timer, and the stop path are completely unaffected — asleep is CSS
   only. Reduced motion: 💤 static, no drift.

## 6. Tests (added: R15, U15–U17, E16a–E16b, E17a–E17c)

- **R15**: the plan function (§1.2).
- **U15**: `formatBytes`.
- **U16**: eggs — `yatYatMatches` (matches with space/comma/period between,
  case-insensitive, no false positive on single "yat"), `konamiProgress`
  (full sequence fires, wrong key resets, prefix re-entry), and the Yat95
  CSS defines the full 11-variable theme contract.
- **U17**: `SleepTracker` — sleeps only after the full window below
  threshold, any loud sample resets, wake is instant, tracker is pure
  (injected clock).
- **E16a**: General shows the Danger-zone row; opening lists the mocked
  plan items with sizes; toggling keep-data refetches and drops the
  app-data row; confirm calls `uninstall_app` with the flag.
- **E16b**: cancel closes the dialog with NO `uninstall_app` call and
  settings remain fully functional.
- **E17a**: overlay in live recording; stream-text containing "yat yat" →
  `pill-wiggle` appears (and clears); plain text never wiggles.
- **E17b**: typing the Konami sequence in settings persists
  `overlay_theme: "secret:yat95"` via set_settings and shows `egg-toast`;
  typing it again restores the prior theme; the Appearance picker still
  shows exactly 12 swatches throughout.
- **E17c**: with Playwright's clock API, a recording overlay fed silence
  for 20 virtual seconds gains `pill-asleep` + `sleep-zzz`; one loud
  mic-level removes it immediately.
- Sanctioned amendments: ONLY the two named in §4. Nothing else may be
  modified, weakened, or deleted.

## 7. Docs

README: an **Uninstalling** section (the in-app path, what's removed, the
two manual leftovers, Windows = Add/Remove Programs); the "Full reset"
dev section points at it; the sound-cues mention flips to "on by default".
Easter eggs are documented in **docs/EASTER-EGGS.md** (what each egg is
and how to trigger it, with the same wink the features have), linked from
README with a single teaser line ("Yat Yat has secrets — spoilers in
docs/EASTER-EGGS.md"); the APP itself never documents or hints at them.
docs/ARCHITECTURE.md: one paragraph — plan purity, best-effort execution
order, trash-not-delete, deep-clean.sh parity; one line noting eggs are
cosmetic-only with pure helpers in src/lib/eggs.ts.

## 8. Definition of Done

1. `npm run validate` exits 0 with complete output — R1–R15, U1–U17,
   E1–E17c, I1–I2, `VALIDATION: ALL PASSED` — printed in the transcript.
2. `npm run tauri build` (signed env) exits 0; app size still < 80 MB —
   **macOS first**: this spec's DoD is fully local for a fast test-iterate
   loop; the Windows compile is verified by the release pipeline's
   test-windows job at the next cut, not gated here.
3. README + docs/EASTER-EGGS.md + ARCHITECTURE updated per §7;
   `npm run licenses` green (the
   trash crate, if one is added, must pass the allowlist);
   `grep -rn ".skip\|.only\|.todo" tests/` prints nothing.
4. Anything infeasible → BLOCKERS.md; never game a check.

Manual checks after green (a scratch install, not your daily one): run the
in-app uninstall with models downloaded — app lands in Trash, app-support
gone, `tccutil` grants cleared (System Settings shows no Yat Yat rows),
login item gone; keep-data run preserves settings.json/history.json;
Windows: the button opens the NSIS uninstaller and Add/Remove works.
