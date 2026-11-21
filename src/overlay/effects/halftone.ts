// halftone: a print-screen dot grid. Dots swell where the sound is and the
// pattern scrolls like a tiny LED sign.

import type { EffectFrame, EffectRenderer } from "./types";
import { clamp01, levelAt, rampAt } from "./types";

const CELL = 6;

export function createHalftone(): EffectRenderer {
  return {
    init() {},
    dispose() {},
    render(ctx, frame: EffectFrame) {
      const w = frame.width;
      const h = frame.height;
      const cols = Math.max(1, Math.floor(w / CELL));
      const rows = Math.max(1, Math.floor(h / CELL));
      const ox = (w - cols * CELL) / 2 + CELL / 2;
      const oy = (h - rows * CELL) / 2 + CELL / 2;
      const midRow = (rows - 1) / 2;
      for (let i = 0; i < cols; i++) {
        const level = levelAt(frame.levels, cols, i);
        ctx.fillStyle = rampAt(frame.colors, cols > 1 ? i / (cols - 1) : 0);
        for (let j = 0; j < rows; j++) {
          const offCenter = midRow > 0 ? Math.abs(j - midRow) / midRow : 0;
          const v = clamp01(level * 1.7 - offCenter * 0.85);
          ctx.globalAlpha = 0.22 + v * 0.78;
          ctx.beginPath();
          ctx.arc(ox + i * CELL, oy + j * CELL, 0.55 + v * 2.1, 0, Math.PI * 2);
          ctx.fill();
        }
      }
      ctx.globalAlpha = 1;
    },
  };
}
