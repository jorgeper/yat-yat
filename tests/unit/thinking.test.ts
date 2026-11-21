// U27 (owner-picked 2026-11-20): the Writing figure — the cursor's travel
// and where each spark rests.

import { describe, expect, it } from "vitest";
import { cursorAt, SPARKS, writingSpark, type Spark } from "../../src/overlay/thinking";

const W = 200;
const H = 30;
const at = (i: number, fraction: number, done = false, blinkOn = true): Spark =>
  writingSpark(i, SPARKS, fraction, done, blinkOn, W, H, 0.5, { x: 0, y: 0, radius: 0, alpha: 0, tone: 0 });
const onLine = (s: Spark) => Math.abs(Math.abs(s.y - H / 2) - H * 0.15) < 1e-6;

describe("U27: writing figure", () => {
  it("u27 without progress the cursor sweeps, rests, and repeats", () => {
    expect(cursorAt(0, 0)).toEqual({ fraction: 0, done: false });
    expect(cursorAt(1, 0).fraction).toBeCloseTo(0.5, 5);
    expect(cursorAt(2.2, 0)).toEqual({ fraction: 1, done: true });
    // 2.6 s later it is writing again from the start.
    expect(cursorAt(2.7, 0).fraction).toBeLessThan(0.1);
    expect(cursorAt(-5, 0)).toEqual({ fraction: 0, done: false });
  });

  it("u27 with progress the cursor IS the progress, done at completion and never before", () => {
    expect(cursorAt(99, 0.25)).toEqual({ fraction: 0.25, done: false });
    expect(cursorAt(0, 0.999)).toEqual({ fraction: 0.999, done: false });
    expect(cursorAt(0, 1)).toEqual({ fraction: 1, done: true });
    expect(cursorAt(0, 7).fraction).toBe(1);
  });

  it("u27 nothing is written before the cursor moves; more is written as it goes", () => {
    const written = (fraction: number) =>
      Array.from({ length: SPARKS }, (_, i) => at(i, fraction)).filter(onLine).length;
    expect(written(0)).toBe(0);
    const half = written(0.5);
    const full = written(1);
    expect(half).toBeGreaterThan(10);
    expect(full).toBeGreaterThan(half);
    // Word gaps: some sparks stay in the cursor even when everything is written.
    expect(full).toBeLessThan(SPARKS);
  });

  it("u27 written sparks sit on two lines behind the cursor; the rest stack in it", () => {
    const fraction = 0.6;
    const cursorX = W * 0.05 + fraction * W * 0.88;
    const lines = new Set<number>();
    for (let i = 0; i < SPARKS; i++) {
      const s = at(i, fraction);
      expect(s.y).toBeGreaterThanOrEqual(0);
      expect(s.y).toBeLessThanOrEqual(H);
      if (onLine(s)) {
        expect(s.x).toBeLessThanOrEqual(cursorX - 2);
        lines.add(Math.sign(s.y - H / 2));
      } else {
        expect(Math.abs(s.x - cursorX)).toBeLessThan(1);
      }
    }
    expect(lines.size).toBe(2);
  });

  it("u27 the cursor blinks when the writing is done, and at no other time", () => {
    const cursorSpark = Array.from({ length: SPARKS }, (_, i) => i).find((i) => !onLine(at(i, 1)))!;
    expect(at(cursorSpark, 1, true, false).alpha).toBeLessThan(0.3);
    expect(at(cursorSpark, 1, true, true).alpha).toBeGreaterThan(0.9);
    expect(at(cursorSpark, 0.4, false, false).alpha).toBeGreaterThan(0.9);
  });
});
