// glyph-rain: falling columns of terminal glyphs; your voice drives the
// density and speed of the downpour.

import type { EffectFrame, EffectRenderer } from "./types";
import { mulberry } from "./types";

const COL_W = 7;
const ROW_H = 8;
const CHARS = "01<>+*=|:#";

export function createGlyphRain(): EffectRenderer {
  const rand = mulberry(1999);
  // Per-column head position (in rows, fractional) and speed factor —
  // sized generously once so render never allocates.
  const heads: number[] = [];
  const speeds: number[] = [];
  for (let i = 0; i < 96; i++) {
    heads.push(rand() * 12 - 12);
    speeds.push(0.6 + rand() * 0.9);
  }
  return {
    init() {},
    dispose() {},
    render(ctx, frame: EffectFrame) {
      const cols = Math.min(heads.length, Math.max(1, Math.floor(frame.width / COL_W)));
      const rows = Math.ceil(frame.height / ROW_H) + 1;
      ctx.font = "9px monospace";

      for (let c = 0; c < cols; c++) {
        if (!frame.reducedMotion) {
          heads[c] += frame.dt * speeds[c] * (2.5 + frame.level * 12);
          // Recycle a finished column below a voice-dependent chance so the
          // rain thins in silence.
          if (heads[c] > rows + 4) {
            heads[c] = -2 - ((c * 2654435761) % 7);
          }
        }
        const head = frame.reducedMotion ? rows * 0.5 + (c % 5) : heads[c];
        // Trail above the head, fading with distance.
        for (let r = 0; r < rows; r++) {
          const d = head - r;
          if (d < 0 || d > 7) continue;
          const isHead = d < 1;
          // Deterministic churn: the character at a cell changes over time.
          const tick = frame.reducedMotion ? 0 : Math.floor(frame.time * 6);
          const idx = (c * 31 + r * 17 + tick * 7) % CHARS.length;
          ctx.fillStyle = isHead ? frame.colors.accent : frame.colors.primary;
          ctx.globalAlpha = isHead ? 0.95 : 0.6 * (1 - d / 7);
          if (isHead) {
            ctx.shadowColor = frame.colors.glow;
            ctx.shadowBlur = 6;
          }
          ctx.fillText(CHARS[idx], c * COL_W + 1, r * ROW_H + ROW_H - 1);
          ctx.shadowBlur = 0;
        }
      }
      ctx.globalAlpha = 1;
    },
  };
}
