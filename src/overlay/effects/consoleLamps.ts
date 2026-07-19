// console-lamps: two rows of candy-colored console indicator lamps that
// twinkle lazily in silence and blink double-time when you talk.

import type { EffectFrame, EffectRenderer } from "./types";
import { lerp, mulberry } from "./types";

const LAMP_W = 9;
const GAP = 4;
const ROWS = 2;

export function createConsoleLamps(): EffectRenderer {
  const rand = mulberry(1966);
  const jitter: number[] = [];
  for (let i = 0; i < 96; i++) jitter.push(rand() * 10);
  let smooth = 0;
  return {
    init() {
      smooth = 0;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      smooth = lerp(smooth, frame.level, 0.15);
      const cols = Math.max(1, Math.floor((frame.width - 4) / (LAMP_W + GAP)));
      const lampH = Math.max(4, (frame.height - 10) / ROWS);
      const rate = 0.8 + smooth * 7;

      for (let r = 0; r < ROWS; r++) {
        for (let c = 0; c < cols; c++) {
          const i = r * cols + c;
          const j = jitter[i % jitter.length];
          // Deterministic on/off schedule per lamp; static pattern under
          // reduced motion, brightness still tied to voice.
          const on = frame.reducedMotion
            ? (i * 2654435761) % 3 !== 0
            : Math.sin(frame.time * rate + j * 2.4) > -0.2 - smooth * 0.5;
          const style =
            i % 3 === 0
              ? frame.colors.primary
              : i % 3 === 1
                ? frame.colors.accent
                : frame.colors.glow;
          const x = 2 + c * (LAMP_W + GAP);
          const y = 3 + r * (lampH + 4);
          ctx.fillStyle = style;
          ctx.globalAlpha = on ? 0.55 + smooth * 0.45 : 0.1;
          if (on) {
            ctx.shadowColor = frame.colors.glow;
            ctx.shadowBlur = 5 + smooth * 8;
          }
          ctx.fillRect(x, y, LAMP_W, lampH);
          // A little bezel gleam on lit lamps.
          if (on) {
            ctx.globalAlpha *= 0.5;
            ctx.fillStyle = frame.colors.accent;
            ctx.fillRect(x + 1, y + 1, LAMP_W - 2, 1);
          }
          ctx.shadowBlur = 0;
        }
      }
      ctx.globalAlpha = 1;
    },
  };
}
