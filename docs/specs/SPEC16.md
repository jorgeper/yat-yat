# SPEC16: Yat Yat — cinema modes: 14 paired effect+theme presets & the Appearance gallery

An increment over docs/specs/SPEC.md–SPEC15.md (authoritative elsewhere).
The overlay's look is built from two orthogonal pieces — 15 effects
(SPEC6 FR-A2) and 12 built-in themes (FR-A3) — picked à la carte. This spec
adds **modes**: named presets inspired by film-and-TV talking gizmos, each a
*designed pair* of one effect and one theme, plus the Appearance UI to browse
what is now a much larger catalog. A mode is presentation-layer sugar only:
picking one writes the two existing settings fields. **No settings-schema
change, no new IPC, no Rust changes, zero network, zero new dependencies.**

Naming is riff-based (house precedent: Yat95) — evocative names, no
trademarks, no character names, no film-title strings in UI copy. This spec
file may name inspirations; the shipped UI may not.

---

## 1. The mode registry (FR-M)

1. **Pure registry.** `src/overlay/modes.ts` exports
   `MODES: ModeDef[]` — `{ id, name, tagline, effect, theme }` — and the
   pure helper `modeFor(effect: string, theme: string): ModeDef | null`
   (the mode whose pair matches exactly, else null). Selection state is
   **derived**: a mode is selected iff both settings fields match its pair.
   No `overlay_mode` setting exists.
2. **Exactly 14 modes**, ids/pairs fixed:

   | mode id | effect | theme | inspiration (spec-only) |
   |---|---|---|---|
   | `night-scanner` | `larson-bar` (new) | `night-scanner` | Knight Rider '82 |
   | `shall-we-play` | `phosphor-terminal` (new) | `war-room` | WarGames '83 |
   | `grid-rider` | `light-grid` (new) | `grid-rider` | Tron '82 |
   | `time-circuits` | `led-circuits` (new) | `time-circuits` | Back to the Future '85 |
   | `motion-tracker` | `tracker-sweep` (new) | `motion-tracker` | Aliens '86 |
   | `liquid-metal` | `chrome-blob` (new) | `liquid-metal` | Terminator 2 '91 |
   | `glyph-rain` | `glyph-rain` (new) | `glyph-rain` | The Matrix '99 |
   | `helix` | `dna-helix` (existing) | `isla-amber` | Jurassic Park '93 |
   | `arc-hud` | `holo-arcs` (new) | `arc-hud` | Iron Man '08 |
   | `companion` | `emote-face` (new) | `companion` | Moon '09 |
   | `bridge-66` | `console-lamps` (new) | `bridge-66` | Star Trek TOS |
   | `engage` | `elbow-panel` (new) | `engage` | Star Trek TNG |
   | `yellow-fury` | `blade-slash` (new) | `yellow-fury` | Kill Bill '03 |
   | `royale` | `neon-diner` (new) | `royale` | Pulp Fiction '94 |

3. Taglines are one-line winks (e.g. `shall-we-play`: "Shall we play a
   game?"), subject to the naming rule in the preamble — and written
   without the DoD grep's letter-sequences, since e2e copy assertions
   would drag them into tests/.

## 2. Effects (FR-E)

1. **13 new renderer modules** (the table's "new" column; `helix` reuses
   `dna-helix`) in `src/overlay/effects/`, registered in `EFFECTS` —
   **28 total**. Every module honors the frozen SPEC6 FR-A1 contract
   unchanged: level/levels/time/dt/colors/reducedMotion in, canvas out.
   Effects are recording-time, level-driven visualizers — no pipeline-state
   awareness (`emote-face` reacts to voice level: idle blink, mouth and
   eyes track `level`; nothing more).
2. **Theme colors only** (U8's source-level scan, which auto-extends to the
   new modules): all shading via `globalAlpha`/shadow over
   `frame.colors` — the chrome of `chrome-blob` and the LED trio of
   `led-circuits` are expressed through the paired theme's
   primary/accent/glow, never module-local color literals. Mind the scan's
   word list when naming identifiers.
3. **SPEC14 discipline**: no per-frame allocations (use the existing
   `levelAt`/`compactInPlace`/`mulberry` helpers), sane static rendering
   under `reducedMotion`, and the existing engine frame cap untouched.
4. Sketches (normative for character, not pixels): `larson-bar` sweeping
   eye whose width/speed follow level; `phosphor-terminal` blocky character
   cascade + fat cursor; `light-grid` perspective floor grid with
   level-lifted light trails; `led-circuits` three segmented-LED rows;
   `tracker-sweep` radar sweep with inbound blips; `chrome-blob` merging
   metaballs rippling with voice; `glyph-rain` falling glyph columns,
   density from level; `holo-arcs` concentric rotating arcs expanding with
   level; `emote-face` per FR-E1; `console-lamps` rows of blinking console
   lamps, cadence from level; `elbow-panel` rounded elbow frame +
   segmented data bars filling with level; `blade-slash` diagonal slash
   streaks per speech burst, edge shimmer on sustained voice; `neon-diner`
   flickering neon tube outline + inner glow welling with level.

## 3. Themes (FR-T)

1. **14 new built-in themes** (the table's theme column) in
   `src/overlay/themes/index.ts` — **26 total**. Each defines the full
   `.nh-theme` variable contract like the existing twelve; fonts are
   system stacks only (mono for `war-room`/`motion-tracker`, condensed
   sans for `engage`, defaults elsewhere as fits).
2. `yellow-fury` is the set's one `light` variant (taxi yellow, black
   stripe accent, red rec-dot); all other new themes are `dark`.
3. User themes, the secret Yat95 path, and theme resolution/fallback
   behavior are untouched.

## 4. Appearance UI (FR-U)

1. **One scrollable page, three anchored sections.** A pinned segmented
   control — testid `appearance-nav`, items `appearance-nav-modes` /
   `-effects` / `-themes` — sits under the preview stage and scroll-anchors
   to the sections. All sections stay mounted; the existing
   `effect-picker`/`theme-picker` grids and their testids are unchanged
   (E13 must pass byte-identical).
2. **Modes gallery** first: testid `mode-gallery`, one card per mode
   (testid `mode-<id>`) — static swatch built from the paired theme's
   swatch colors + name + tagline. **No per-card live engines** (SPEC14
   FR-S3 sensibilities): the single existing real-engine preview stage is
   the only live surface.
3. Clicking a card saves **both** `overlay_effect` and `overlay_theme` in
   one `save()` (one settings write). Card selected-state comes from
   `modeFor(...)`; diverging via either à-la-carte picker afterwards
   deselects the card by derivation — no stored mode state anywhere.
4. Hovering (pointer) or selecting a mode card retargets the existing
   preview stage to that mode's pair — preview-only on hover, reverting to
   the saved pair on pointer leave; no settings write until click.
5. Effect/theme picker grids stay functionally as they are (grid layout
   polish allowed; behavior and testids identical).

## 5. Sanctioned test amendments

The count assertions are the *only* permitted edits to existing tests:
`tests/unit/effects.test.ts` count literals 15 → 28 (three occurrences:
registry length, unique-id set size, module-file count) and
`tests/unit/themes.test.ts` count literals 12 → 26 (two occurrences).
Every other line of every existing test is untouched; U8's per-effect
interface/draw smoke and color-literal scan extend to the new modules by
iteration, and that extension must pass — never be special-cased.

## 6. Tests (added: U24, E22a–E22b)

1. **U24 (mode registry):** exactly 14 modes with unique ids; every mode's
   `effect` resolves in `EFFECTS` and `theme` in `THEMES`; `modeFor` round-
   trips every pair and returns null for a diverged pair and for unknown
   ids.
2. **E22a (gallery drives both fields):** click `mode-glyph-rain` → the
   card is selected, the effect picker shows `glyph-rain` selected and the
   theme picker `glyph-rain` selected, and both persist across reload;
   then click a different à-la-carte effect → the card deselects while the
   theme stays.
3. **E22b (anchors + one-write save):** the segmented control scrolls each
   section into view; selecting a mode issues exactly one `set_settings`
   call (mock `calls()` delta).
4. Test IDs continue the sequences (no Rust changes ⇒ R stays at R27);
   function/title names embed the IDs. Mind the DoD grep's letter-sequence
   trap in names and copy.

## 7. Docs

1. **docs/ARCHITECTURE.md**: update the appearance section counts and add
   a short "modes" note (derived pairs, no schema field, registry +
   `modeFor`).
2. **CLAUDE.md**: the appearance bullet's counts ("exactly 15 effects / 12
   built-in themes") become 28/26 with modes mentioned.
3. **README** "Looks" section: one paragraph on modes.
4. Existing SPEC/GOAL files untouched (immutable history).

## 8. Definition of Done

1. `npm run validate` exits 0 with complete output — R1–R27, U1–U24,
   E1–E22b, I1–I2, `VALIDATION: ALL PASSED` — printed in the transcript.
2. `npm run tauri build` (signing env per docs/RELEASING.md) exits 0 with
   the `.app` path and size (< 80 MB) printed.
3. The transcript shows: `git diff` over `tests/` limited to the §5 count
   literals plus the new U24/E22 files, and U8's color-literal scan
   passing over all 28 modules.
4. `grep -rn ".skip\|.only\|.todo" tests/` prints nothing.
5. Docs per §7; no settings-schema, IPC-surface, or Rust changes
   (`src-tauri/` diff empty); no new dependencies; every existing
   invariant (one pipeline thread, non-activating overlay, IPC mirror,
   server-owned settings, localhost-only enhancement) holds.
6. Anything infeasible → BLOCKERS.md; never game a check.

Manual checks after green (human at the machine): flip through all 14 mode
cards in Settings → Appearance — each retargets the live preview on hover
and sticks on click; dictate once in `night-scanner` and once in
`yellow-fury` (the light theme) over a bright and a dark desktop; confirm
`companion` blinks at idle and mouths while speaking; confirm reduced-motion
(System Settings → Accessibility) stills every new effect.
