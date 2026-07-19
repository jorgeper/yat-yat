# Launching the cinema-modes build with /goal

Run from this directory (`yat-yat/`). docs/specs/SPEC.md–SPEC15.md remain
authoritative; docs/specs/SPEC16.md adds 14 "cinema modes" — named presets
pairing one effect with one theme (13 new effect modules, 14 new built-in
themes → 28 effects / 26 themes) plus the Appearance gallery to browse
them. A mode is derived presentation sugar: picking a card writes the two
existing settings fields; there is NO new settings field, IPC command, or
Rust change. Names are trademark-safe riffs (Yat95 precedent).

The command does not restate the FRs (/goal conditions cap at 4000 chars —
AGENTS.md); the SPEC carries the detail.

## Interactive (paste into a Claude Code session)

```
/goal Implement docs/specs/SPEC16.md in full — 14 cinema modes (paired effect+theme presets), 13 new effects, 14 new themes, and the Appearance gallery (read src/overlay/effects/index.ts, types.ts, engine.ts, src/overlay/themes/index.ts, src/settings/AppearanceSection.tsx, tests/unit/effects.test.ts, themes.test.ts, tests/e2e/appearance.spec.ts first). The SPEC is authoritative for every FR-M/FR-E/FR-T/FR-U requirement — fully local, ZERO new network, ZERO new dependencies, NO settings-schema/IPC/Rust changes (src-tauri/ diff must be empty). Done when: 'npm run validate' exits 0 on macOS with its complete output — R1–R27, U1–U24, E1–E22b, I1–I2, final line 'VALIDATION: ALL PASSED' — printed in the transcript, AND 'npm run tauri build' with the signing env exits 0 with the .app path and size (< 80 MB) printed, AND the transcript shows 'git diff' over tests/ touching ONLY the SPEC16 §5 count literals (effects 15→28 ×3, themes 12→26 ×2) plus the new U24/E22 files, AND U8's color-literal scan passes over all 28 effect modules, AND docs/ARCHITECTURE.md + CLAUDE.md counts are updated with the modes note and README's Looks section mentions modes, AND 'grep -rn ".skip\|.only\|.todo" tests/' prints nothing. Constraints: the SPEC and GOAL files and this condition must not be modified; the mode registry is pure (MODES + modeFor in src/overlay/modes.ts, U24-tested: 14 modes, unique ids, every pair resolves, diverged/unknown pairs → null); mode selection is DERIVED from the two settings fields — no stored mode state; a card click saves effect+theme in ONE set_settings call (E22b asserts the call-count delta); the effect contract (SPEC6 FR-A1 types.ts) stays frozen — effects get level/levels/time/dt/colors/reducedMotion only, emote-face reacts to level alone; every new effect uses frame.colors exclusively (mind U8's banned color words in identifiers), handles reducedMotion and empty levels, and allocates nothing per frame; the existing effect-picker/theme-picker testids and E13 behavior stay byte-identical (segmented control is scroll-anchor sugar, all sections mounted); mode cards are static swatches — no per-card live engines; hover retargets the single existing preview stage without writing settings (revert on leave); yellow-fury is the only light-variant new theme; fonts are system stacks only; no film titles, trademarks, or character names in UI copy; NO existing test is amended beyond the §5 count literals; if something is infeasible, record it in BLOCKERS.md instead of gaming the check; after validate is first green, run one adversarial review pass over the ENTIRE diff hunting SPEC16's risks — a hover-preview that writes settings or leaks after pointer-leave, mode cards running hidden engines, a color literal smuggled past U8 via identifier names, gallery clicks issuing two settings writes (effect then theme racing the pipeline's settings read), scroll-anchor nav breaking E13's direct testid clicks, and reduced-motion leaving any new effect animating — fix findings and re-run validate before declaring done. Stop after 40 turns or 5 hours even if incomplete, and summarize remaining work.
```

## Unattended one-liner

`claude -p "/goal …"` works, but interactive is recommended — the decisive
checks are visual: all 14 cards must read as their inspirations at a
glance, hover-preview must feel instant, and the two Tarantino modes live
or die on taste.

## Notes

- **The §5 sanctioned amendments are exact**: five count literals across
  two test files, nothing else. U8's per-module iteration extends to the
  13 new effects automatically — if one fails the color scan, fix the
  module, never the scan.
- **One save per card click.** Two sequential saves would race the
  pipeline's settings snapshot at recording start and double-write the
  settings file.
- **Preview ≠ persistence.** Hover retargets the preview engine only;
  revert on pointer leave; click is the only path that saves.
- **emote-face is level-driven only** — the frozen effect contract carries
  no pipeline state; do not extend types.ts to sneak it in.
- Watch the DoD grep: avoid the letter-sequences "skip"/"only"/"todo" in
  test names, taglines, and comments (write "left out", not "skipped").
