# SPEC12: Yat Yat — the dance egg, tray wiggle, and an eggs switch

An increment over docs/specs/SPEC.md–SPEC11.md (authoritative elsewhere).
SPEC11 §5.1 shipped a wiggle egg triggered by "yat yat" in the raw stream.
Field-testing killed it: Whisper tiny **never** transcribes "yat yat" —
measured against synthesized speech in four voices it produced "that you
add", "that yet", and "you're at", none of which any sane pattern can
match without constant false fires. Only the documented "yeah yeah"
fallback ever worked. This spec supersedes the trigger with the word
**"dance"** (measured: exact in 3/4 voices in sentence context), extends
the wiggle to the **tray icon**, and adds a user-facing **Easter eggs**
setting that gates all three eggs. SPEC11's iron rule stands: every egg
is **cosmetic only** — none may ever touch recording, transcription,
cleanup, or paste behavior, and all motion respects
prefers-reduced-motion.

Out of scope: changing the Konami/Yat95 egg or the sleepy waveform beyond
gating them; any in-app documentation or hints for eggs (docs only);
Windows-specific tray work beyond compiling (mac-first, as SPEC11 §8).

## 1. The dance trigger (FR-D)

1. `src/lib/eggs.ts`: `yatYatMatches` is **replaced** by
   `danceMatches(text) -> number`, counting `/\bdance\b/gi` matches in the
   RAW stream text (pre-cleanup, as before). Single word, deliberately
   loose — "dance class tonight" wiggles too; it's cosmetic, that's fine.
2. Overlay behavior is unchanged in shape: at most one wiggle per new
   match count per recording, `pill-wiggle` (~600 ms, removed on
   animationend), reset on each new recording, suppressed entirely by
   reduced motion.
3. SPEC11 §5.1's trigger is superseded — note the divergence (and the
   measured Whisper evidence, one line) in docs/ARCHITECTURE.md. SPEC11
   itself is not edited.

## 2. Tray wiggle (FR-T)

1. When the dance egg fires (same detection instant as the pill wiggle),
   the overlay also invokes a new command `wiggle_tray()`. Reduced motion
   suppresses the call along with the pill class; eggs off (§3) suppresses
   both at the source.
2. `scripts/gen-icons.mjs` deterministically generates wiggle frames —
   the recording tray glyph at a few rotations (e.g. −8°, +8°, −4°, 0°) as
   `tray-wiggle-<n>.png`, bundled with the existing tray resources.
3. Rust (`tray.rs`): `wiggle_tray` steps the tray icon through the frames
   ~70 ms apart on a short-lived thread, then restores the icon for the
   **current** `TrayState`, template mode throughout (the macOS 26 privacy
   capsule keeps rendering our alpha — see the SPEC7 FR-T1 divergence
   note). Re-entrant calls during a wiggle are ignored (a simple in-flight
   flag). The command also re-checks `easter_eggs` server-side and no-ops
   when false (defense in depth).
4. Mirror the command in `src/ipc/api.ts`, `src/ipc/types.ts`, and
   `src/ipc/mock.ts`; the mock records invocations so e2e can assert them.
5. Escape hatch: if rapid `set_icon` swaps flicker or are throttled by
   macOS, fall back to a single tilted frame held ~600 ms and record the
   observation in BLOCKERS.md.

## 3. The Easter-eggs setting (FR-S)

1. `Settings.easter_eggs: bool`, default **true** (struct-level
   `#[serde(default)]` + the `Default` impl — legacy JSON loads as ON).
   UI-owned field: ordinary whole-object writes, no server-owned
   preservation needed. Mirrored in `types.ts` and the mock's
   `defaultSettings()`.
2. Settings → General: a checkbox **"Easter eggs"** with hint
   "Yat Yat has secrets", testid `eggs-toggle`, placed above the Danger
   zone row.
3. When false, ALL eggs are inert: no dance wiggle (pill or tray), the
   Konami sequence does nothing (no theme change, no `egg-toast`), and
   the sleepy waveform never engages. The overlay reads the flag from
   settings at each recording start; the settings window reads its live
   state.
4. If Yat95 is active when eggs are switched off, the theme **stays** —
   no surprise theme swap; the user escapes by picking any normal theme
   in Appearance. (The Konami toggle-back is unavailable while off; this
   is intended.)

## 4. Tests (added: R17, U18, E18a–E18b; amended: U16, E17a)

- **R17**: `easter_eggs` defaults true; legacy JSON without the field
  loads true; a persisted false round-trips.
- **U18**: `danceMatches` — counts single and multiple matches, case-
  insensitive, word-boundary (no match inside "dancer"/"abundance"),
  empty string.
- **E18a**: General shows `eggs-toggle` checked by default and persists
  an uncheck via set_settings; with eggs off, a dance stream produces NO
  `pill-wiggle` and NO `wiggle_tray` call, and the Konami sequence
  changes nothing (no toast, theme persisted unchanged).
- **E18b**: with eggs off and Playwright's clock, 20 virtual seconds of
  silence never gains `pill-asleep`; flipping eggs back on restores the
  sleepy behavior in the same session.
- Sanctioned amendments (superseded behavior, SPEC11 §5.1 — and ONLY
  these): **U16**'s yat-yat detector block is removed (replaced by U18;
  its Konami and Yat95-CSS assertions are untouched), and **E17a**'s
  trigger text becomes a "dance" stream, now also asserting the mock
  recorded exactly one `wiggle_tray` call (its plain-text-never-wiggles
  half is unchanged). Nothing else may be modified, weakened, or deleted.

## 5. Docs

docs/EASTER-EGGS.md: §1 rewritten for **"dance!"** — same wink, now
mentioning both the pill and the menu-bar icon doing the wiggle, and the
new General-section switch for people who want zero surprises (one line,
all three eggs). docs/ARCHITECTURE.md: the SPEC11 §5.1 divergence note
(§1.3 above) plus one line each for `wiggle_tray` (frame-swap, template
mode, in-flight guard) and the `easter_eggs` gate. README untouched — the
teaser line already points at EASTER-EGGS.md.

## 6. Definition of Done

1. `npm run validate` exits 0 with complete output — R1–R17, U1–U18,
   E1–E18b, I1–I2, `VALIDATION: ALL PASSED` — printed in the transcript.
2. `npm run tauri build` (signed env) exits 0; app size still < 80 MB.
   macOS first: Windows compile is verified by the release pipeline's
   test-windows job at the next cut, not gated here.
3. docs/EASTER-EGGS.md + docs/ARCHITECTURE.md updated per §5;
   `node scripts/gen-icons.mjs` re-run cleanly (deterministic output);
   `grep -rn ".skip\|.only\|.todo" tests/` prints nothing.
4. Anything infeasible → BLOCKERS.md; never game a check.

Manual checks after green: dictate "dance" with live transcription on —
pill and menu-bar icon both wiggle once; dictate it with Easter eggs off —
nothing moves; System Settings → Accessibility → Reduce Motion on — the
pill stays still and the tray never swaps.
