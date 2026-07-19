// phosphor-terminal: blocky characters typing themselves across a CRT grid,
// cadence driven by voice, with a fat blinking cursor. Wraps and clears
// like a terminal screen.

import type { EffectFrame, EffectRenderer } from "./types";
import { mulberry } from "./types";

const CELL_W = 7;
const CELL_H = 9;
const GLYPH_W = 5;
const GLYPH_H = 7;

export function createPhosphorTerminal(): EffectRenderer {
  let typed = 0; // fractional cells typed
  const rand = mulberry(1983);
  // Deterministic per-cell glyph shapes, generated once (no per-frame alloc).
  const shapes: number[] = [];
  for (let i = 0; i < 512; i++) shapes.push(Math.floor(rand() * 0x7fff));
  return {
    init() {
      typed = 0;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      const cols = Math.max(1, Math.floor((frame.width - 4) / CELL_W));
      const rows = Math.max(1, Math.floor(frame.height / CELL_H));
      const capacity = cols * rows;

      if (!frame.reducedMotion) {
        typed += frame.dt * (2 + frame.level * 26);
        if (typed >= capacity) typed = 0; // screen full — clear, start over
      } else {
        typed = Math.min(capacity - 1, frame.level * capacity);
      }
      const shown = Math.floor(typed);

      for (let i = 0; i < shown; i++) {
        const cx = 2 + (i % cols) * CELL_W;
        const cy = (Math.floor(i / cols) % rows) * CELL_H + 1;
        const bits = shapes[i % shapes.length];
        ctx.fillStyle = frame.colors.primary;
        // Older characters dim a little, like fading phosphor.
        ctx.globalAlpha = 0.35 + 0.6 * (i / Math.max(1, shown));
        // A 3x3 sub-block pattern reads as a character at pill size.
        for (let b = 0; b < 9; b++) {
          if ((bits >> b) & 1) {
            const bx = cx + (b % 3) * (GLYPH_W / 3);
            const by = cy + Math.floor(b / 3) * (GLYPH_H / 3);
            ctx.fillRect(bx, by, GLYPH_W / 3, GLYPH_H / 3);
          }
        }
      }

      // The cursor: fat block at the write position, blinking on wall time.
      const blinkOn = frame.reducedMotion || frame.time % 0.9 < 0.55;
      if (blinkOn) {
        const cx = 2 + (shown % cols) * CELL_W;
        const cy = (Math.floor(shown / cols) % rows) * CELL_H + 1;
        ctx.fillStyle = frame.colors.accent;
        ctx.globalAlpha = 0.95;
        ctx.shadowColor = frame.colors.glow;
        ctx.shadowBlur = 8;
        ctx.fillRect(cx, cy, GLYPH_W, GLYPH_H);
        ctx.shadowBlur = 0;
      }
      ctx.globalAlpha = 1;
    },
  };
}
