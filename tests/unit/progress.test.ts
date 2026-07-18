// U19: transcription-progress helpers (SPEC13 FR-O1) — the fill maps a
// fraction to filled shimmer bars and never moves backward.

import { describe, expect, it } from "vitest";
import { advance, filledBars } from "../../src/overlay/progress";

describe("U19: transcription progress helpers", () => {
  it("advance clamps to 0..1", () => {
    expect(advance(0, -0.5)).toBe(0);
    expect(advance(0, 1.7)).toBe(1);
    expect(advance(0, 0.4)).toBe(0.4);
  });

  it("advance never regresses", () => {
    expect(advance(0.6, 0.3)).toBe(0.6);
    expect(advance(0.6, 0.61)).toBe(0.61);
    expect(advance(1, 0)).toBe(1);
  });

  it("advance treats junk input as no movement", () => {
    expect(advance(0.5, Number.NaN)).toBe(0.5);
  });

  it("filledBars is empty at 0 and full at or past 1", () => {
    expect(filledBars(0, 12)).toBe(0);
    expect(filledBars(1, 12)).toBe(12);
    expect(filledBars(1.5, 12)).toBe(12);
  });

  it("filledBars stays partial below 1 and never overflows", () => {
    // Full is reserved for completion (SPEC13 FR-O1): even 0.999 keeps
    // at least one bar unfilled.
    expect(filledBars(0.999, 12)).toBe(11);
    expect(filledBars(0.95, 12)).toBeLessThan(12);
    expect(filledBars(-0.2, 12)).toBe(0);
  });

  it("filledBars is monotonic across the range", () => {
    let prev = -1;
    for (let f = 0; f <= 1.0001; f += 0.01) {
      const n = filledBars(f, 12);
      expect(n).toBeGreaterThanOrEqual(prev);
      expect(n).toBeGreaterThanOrEqual(0);
      expect(n).toBeLessThanOrEqual(12);
      prev = n;
    }
  });
});
