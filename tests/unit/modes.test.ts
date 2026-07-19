// U24 (SPEC16 §6.1): the cinema-mode registry — 14 pure presets whose
// pairs resolve against the real effect and theme registries, and the
// derived-selection helper.

import { describe, expect, it } from "vitest";
import { EFFECTS } from "../../src/overlay/effects";
import { MODES, modeFor } from "../../src/overlay/modes";
import { THEMES } from "../../src/overlay/themes";

describe("U24: cinema-mode registry", () => {
  it("u24 has exactly 14 modes with unique ids", () => {
    expect(MODES).toHaveLength(14);
    expect(new Set(MODES.map((m) => m.id)).size).toBe(14);
  });

  it("u24 every mode pairs an existing effect with an existing theme", () => {
    const effectIds = new Set(EFFECTS.map((e) => e.id));
    const themeIds = new Set(THEMES.map((t) => t.id));
    for (const m of MODES) {
      expect(effectIds.has(m.effect), `${m.id}: effect '${m.effect}' exists`).toBe(true);
      expect(themeIds.has(m.theme), `${m.id}: theme '${m.theme}' exists`).toBe(true);
      expect(m.name).toBeTruthy();
      expect(m.tagline).toBeTruthy();
    }
  });

  it("u24 modeFor round-trips every pair", () => {
    for (const m of MODES) {
      expect(modeFor(m.effect, m.theme)?.id).toBe(m.id);
    }
  });

  it("u24 modeFor returns null for diverged and unknown pairs", () => {
    // Diverged: a real mode's effect with a different real theme.
    expect(modeFor(MODES[0].effect, "indigo")).toBeNull();
    expect(modeFor("classic-bars", MODES[0].theme)).toBeNull();
    // Unknown ids on either side.
    expect(modeFor("does-not-exist", MODES[0].theme)).toBeNull();
    expect(modeFor(MODES[0].effect, "does-not-exist")).toBeNull();
    expect(modeFor("", "")).toBeNull();
  });
});
