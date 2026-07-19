// led-circuits: three stacked segmented-LED rows. The top row tracks the
// live level, the middle a short average, the bottom a slow average — three
// readouts of the same voice, like a dashboard of time displays.

import type { EffectFrame, EffectRenderer } from "./types";
import { levelAt, lerp } from "./types";

const SEG_W = 6;
const GAP = 2;

export function createLedCircuits(): EffectRenderer {
  let mid = 0;
  let slow = 0;
  return {
    init() {
      mid = 0;
      slow = 0;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      mid = lerp(mid, frame.level, 0.06);
      slow = lerp(slow, frame.level, 0.015);
      const count = Math.max(1, Math.floor((frame.width - 4) / (SEG_W + GAP)));
      const rowH = Math.max(3, (frame.height - 8) / 3);
      const rows = 3;

      for (let r = 0; r < rows; r++) {
        const value = r === 0 ? frame.level : r === 1 ? mid : slow;
        const lit = Math.round(value * count);
        const y = 2 + r * (rowH + 2);
        const style =
          r === 0 ? frame.colors.primary : r === 1 ? frame.colors.accent : frame.colors.glow;
        for (let i = 0; i < count; i++) {
          const x = 2 + i * (SEG_W + GAP);
          const on = i < lit;
          // A segment flicker on the live row when loud, digital-clock style.
          const flicker =
            !frame.reducedMotion &&
            r === 0 &&
            on &&
            Math.sin(frame.time * 40 + i * 7) > 0.92;
          ctx.fillStyle = style;
          ctx.globalAlpha = on ? (flicker ? 0.45 : 0.9) : 0.12;
          if (on) {
            ctx.shadowColor = frame.colors.glow;
            ctx.shadowBlur = 6;
          }
          ctx.fillRect(x, y, SEG_W, rowH);
          ctx.shadowBlur = 0;
        }
      }
      // History tick marks along the bottom edge from the level buffer.
      ctx.fillStyle = frame.colors.accent;
      for (let i = 0; i < count; i++) {
        const h = levelAt(frame.levels, count, i) * 2;
        if (h <= 0) continue;
        ctx.globalAlpha = 0.5;
        ctx.fillRect(2 + i * (SEG_W + GAP), frame.height - 2, SEG_W, -h);
      }
      ctx.globalAlpha = 1;
    },
  };
}
