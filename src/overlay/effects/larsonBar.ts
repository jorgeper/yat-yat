// larson-bar: a lone scanning eye sweeping the pill; voice widens the head
// and quickens the sweep. Trailing segments fade behind the scan direction.

import type { EffectFrame, EffectRenderer } from "./types";
import { lerp } from "./types";

const TRAIL = 10;
const TRAIL_STEP = 7;

export function createLarsonBar(): EffectRenderer {
  let phase = 0; // 0..2 — one full out-and-back sweep
  let smooth = 0;
  return {
    init() {
      phase = 0;
      smooth = 0;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      smooth = lerp(smooth, frame.level, 0.2);
      const cy = frame.height / 2;
      const headW = 10 + smooth * 26;
      const headH = Math.max(4, frame.height * 0.35);
      const span = Math.max(1, frame.width - headW);

      if (!frame.reducedMotion) {
        phase = (phase + frame.dt * (0.55 + smooth * 1.2)) % 2;
      }
      // Triangle wave: 0..1..0 across the sweep, static center when calm.
      const t = frame.reducedMotion ? 0.5 : phase < 1 ? phase : 2 - phase;
      const dir = frame.reducedMotion || phase < 1 ? 1 : -1;
      const headX = t * span;

      // Fading trail behind the head.
      for (let i = TRAIL; i >= 1; i--) {
        const x = headX - dir * i * TRAIL_STEP;
        if (x < -headW || x > frame.width) continue;
        ctx.fillStyle = frame.colors.primary;
        ctx.globalAlpha = 0.35 * (1 - i / (TRAIL + 1)) * (0.4 + smooth * 0.6);
        ctx.fillRect(x, cy - headH / 2, headW * 0.7, headH);
      }

      // The eye.
      ctx.fillStyle = frame.colors.primary;
      ctx.globalAlpha = 0.95;
      ctx.shadowColor = frame.colors.glow;
      ctx.shadowBlur = 10 + smooth * 18;
      ctx.fillRect(headX, cy - headH / 2, headW, headH);
      // Hot core.
      ctx.fillStyle = frame.colors.accent;
      ctx.globalAlpha = 0.8;
      ctx.fillRect(headX + headW * 0.3, cy - headH / 4, headW * 0.4, headH / 2);
      ctx.shadowBlur = 0;
      ctx.globalAlpha = 1;
    },
  };
}
