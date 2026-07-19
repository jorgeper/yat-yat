// U20: EffectEngine frame cap + dt-based level decay (SPEC14 FR-R1).
// stepFrame is driven directly with fake timestamps: render count stays
// capped near 60 fps when ticks arrive at 120 Hz, and decay over equal
// simulated wall time is identical at any tick rate.

import { describe, expect, it } from "vitest";
import { EffectEngine } from "../../src/overlay/effects/engine";

function engineAtRate(tickHz: number, seconds: number): { engine: EffectEngine; rendered: number } {
  const engine = new EffectEngine(document.createElement("canvas"));
  engine.feed(1.0);
  let rendered = 0;
  const ticks = tickHz * seconds;
  for (let i = 0; i <= ticks; i++) {
    if (engine.stepFrame(i * (1000 / tickHz))) rendered++;
  }
  return { engine, rendered };
}

describe("U20: engine frame cap and rate-independent decay", () => {
  it("caps rendering near 60 fps when ticks arrive at 120 Hz", () => {
    const { rendered } = engineAtRate(120, 1);
    expect(rendered).toBeLessThanOrEqual(62);
    expect(rendered).toBeGreaterThanOrEqual(55);
  });

  it("renders every tick at 60 Hz and below — the cap never starves a normal display", () => {
    expect(engineAtRate(60, 1).rendered).toBe(61);
    expect(engineAtRate(30, 1).rendered).toBe(31);
  });

  it("rejects a tick that arrives sooner than the frame budget", () => {
    const engine = new EffectEngine(document.createElement("canvas"));
    expect(engine.stepFrame(0)).toBe(true);
    expect(engine.stepFrame(4)).toBe(false); // 4 ms after a render: dropped
    expect(engine.stepFrame(17)).toBe(true);
  });

  it("decays the level identically over equal wall time at 120 Hz and 30 Hz ticks", () => {
    const at120 = engineAtRate(120, 1).engine.currentLevel;
    const at30 = engineAtRate(30, 1).engine.currentLevel;
    // Both must equal 0.92^60 (one second at the 60 fps reference decay) —
    // the old per-frame *0.92 decayed 4x faster at 120 Hz than at 30 Hz.
    const expected = Math.pow(0.92, 60);
    expect(at120).toBeCloseTo(expected, 4);
    expect(at30).toBeCloseTo(expected, 4);
    expect(at120).toBeCloseTo(at30, 4);
  });

  it("the first frame carries dt 0 — no decay before time has passed", () => {
    const engine = new EffectEngine(document.createElement("canvas"));
    engine.feed(1.0);
    engine.stepFrame(500);
    expect(engine.currentLevel).toBe(1.0);
  });
});
