// spectrum-blocks: a chunky grid of cells lighting bottom-up with the voice.

import type { EffectFrame, EffectRenderer } from "./types";
import { levelAt } from "./types";

const COLS = 24;
const ROWS = 5;

export function createSpectrumBlocks(): EffectRenderer {
  return {
    init() {},
    dispose() {},
    render(ctx, frame: EffectFrame) {
      const cw = frame.width / COLS;
      const ch = frame.height / ROWS;
      for (let c = 0; c < COLS; c++) {
        const level = levelAt(frame.levels, COLS, c);
        const lit = Math.round(level * ROWS);
        for (let r = 0; r < ROWS; r++) {
          const on = r < lit;
          // Top row of a loud column gets the accent, VU-meter style.
          ctx.fillStyle = on && r >= ROWS - 2 ? frame.colors.accent : frame.colors.primary;
          ctx.globalAlpha = on ? 0.85 : 0.12;
          ctx.fillRect(
            c * cw + 1,
            frame.height - (r + 1) * ch + 1,
            cw - 2,
            ch - 2,
          );
        }
      }
      ctx.globalAlpha = 1;
    },
  };
}
