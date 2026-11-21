// filaments: hair-fine threads, one per hue, running the length of the
// pill. They gather into knots that travel along as you speak, then comb
// back out.

import type { EffectFrame, EffectRenderer } from "./types";
import { lerp, rampAt } from "./types";

export function createFilaments(): EffectRenderer {
  let smooth = 0;
  return {
    init() {
      smooth = 0;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      const w = frame.width;
      const h = frame.height;
      const cy = h / 2;
      const t = frame.reducedMotion ? 1 : frame.time;
      const threads = Math.max(14, Math.floor(h / 1.6));
      smooth = lerp(smooth, frame.level, 0.1);

      ctx.lineWidth = 0.8;
      ctx.lineJoin = "round";
      ctx.globalAlpha = 0.9;
      const drift = Math.sin(t * 0.4) * 2;
      for (let k = 0; k < threads; k++) {
        const v = k / (threads - 1);
        const home = 1.5 + v * (h - 3);
        const gathered = cy + (v - 0.5) * h * 0.12;
        ctx.strokeStyle = rampAt(frame.colors, v);
        ctx.beginPath();
        for (let x = 0; x <= w; x += 3) {
          const swell = 0.5 + 0.5 * Math.sin(x * 0.04 - t * 1.6 + drift);
          const pinch = (0.25 + 0.72 * smooth) * swell * swell;
          const ripple = Math.sin(x * 0.07 + t * 2.2 + k * 0.6) * (0.4 + smooth * 3.2);
          ctx.lineTo(x, lerp(home, gathered, pinch) + ripple * (1 - pinch));
        }
        ctx.stroke();
      }
      ctx.globalAlpha = 1;
    },
  };
}
