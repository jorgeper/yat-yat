// U26 (owner-requested 2026-11-20): color ramps — parsing a theme's
// `--nh-fx-ramp`, expanding it into the smooth gradient effects index, and
// the built-in catalog's ramp contract.

import { describe, expect, it } from "vitest";
import { rampAt } from "../../src/overlay/effects/types";
import { expandRamp, parseColor, RAMP_STEPS, splitRamp } from "../../src/overlay/ramp";
import { THEMES } from "../../src/overlay/themes";

describe("U26: color ramps", () => {
  it("u26 parses hex and rgb()/rgba() colors, rejects the rest", () => {
    expect(parseColor("#fff")).toEqual([255, 255, 255, 1]);
    expect(parseColor(" #7AA8FF ")).toEqual([122, 168, 255, 1]);
    expect(parseColor("#00000080")?.[3]).toBeCloseTo(0.5, 2);
    expect(parseColor("rgb(1, 2, 3)")).toEqual([1, 2, 3, 1]);
    expect(parseColor("rgba(10, 20, 30, 0.4)")).toEqual([10, 20, 30, 0.4]);
    expect(parseColor("rebeccapurple")).toBeNull();
    expect(parseColor("")).toBeNull();
  });

  it("u26 splits a ramp on top-level commas", () => {
    expect(splitRamp("#111, #222,#333")).toEqual(["#111", "#222", "#333"]);
    expect(splitRamp("rgb(1, 2, 3), #fff")).toEqual(["rgb(1, 2, 3)", "#fff"]);
    expect(splitRamp("")).toEqual([]);
    expect(splitRamp("  ")).toEqual([]);
  });

  it("u26 expands stops into a smooth gradient that hits every stop in order", () => {
    const ramp = expandRamp(["#000000", "#ff0000", "#ffffff"], 5);
    expect(ramp).toEqual([
      "rgb(0, 0, 0)",
      "rgb(128, 0, 0)",
      "rgb(255, 0, 0)",
      "rgb(255, 128, 128)",
      "rgb(255, 255, 255)",
    ]);
    expect(expandRamp(["#000000", "#ffffff"])).toHaveLength(RAMP_STEPS);
    expect(expandRamp([])).toEqual([]);
  });

  it("u26 never blends a color it cannot parse — it bands instead", () => {
    expect(expandRamp(["tomato", "#ffffff"], 4)).toEqual([
      "tomato",
      "tomato",
      "#ffffff",
      "#ffffff",
    ]);
    expect(expandRamp(["#123456"], 3)).toEqual(["#123456", "#123456", "#123456"]);
  });

  it("u26 rampAt indexes the ramp and falls back to the primary without one", () => {
    const colors = {
      primary: "p",
      accent: "a",
      glow: "g",
      ramp: ["r0", "r1", "r2"],
      blend: "source-over" as const,
    };
    expect(rampAt(colors, 0)).toBe("r0");
    expect(rampAt(colors, 0.5)).toBe("r1");
    expect(rampAt(colors, 1)).toBe("r2");
    expect(rampAt(colors, 7)).toBe("r2");
    expect(rampAt(colors, -1)).toBe("r0");
    expect(rampAt({ ...colors, ramp: [] }, 0.5)).toBe("p");
  });

  for (const theme of THEMES) {
    it(`u26 ${theme.id}: declares a blendable ramp of 3 to 6 colors and a blend mode`, () => {
      expect(theme.ramp.length).toBeGreaterThanOrEqual(3);
      expect(theme.ramp.length).toBeLessThanOrEqual(6);
      for (const color of theme.ramp) expect(parseColor(color)).not.toBeNull();
      expect(theme.css).toContain(`--nh-fx-ramp: ${theme.ramp.join(", ")};`);
      expect(theme.css).toContain(
        `--nh-fx-blend: ${theme.variant === "dark" ? "lighter" : "multiply"};`,
      );
    });
  }
});
