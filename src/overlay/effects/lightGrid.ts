// light-grid: a perspective floor grid scrolling toward the viewer, with
// light trails that lift off the grid as you speak.

import type { EffectFrame, EffectRenderer } from "./types";
import { lerp } from "./types";

const H_LINES = 6;
const V_LINES = 9;

export function createLightGrid(): EffectRenderer {
  let scroll = 0;
  let smooth = 0;
  return {
    init() {
      scroll = 0;
      smooth = 0;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      smooth = lerp(smooth, frame.level, 0.15);
      const horizonY = frame.height * 0.28;
      const cx = frame.width / 2;

      if (!frame.reducedMotion) {
        scroll = (scroll + frame.dt * (0.6 + smooth * 2.2)) % 1;
      }

      // Converging verticals.
      ctx.strokeStyle = frame.colors.primary;
      ctx.lineWidth = 1;
      for (let i = 0; i < V_LINES; i++) {
        const f = i / (V_LINES - 1) - 0.5;
        ctx.globalAlpha = 0.28;
        ctx.beginPath();
        ctx.moveTo(cx + f * frame.width * 0.25, horizonY);
        ctx.lineTo(cx + f * frame.width * 1.6, frame.height);
        ctx.stroke();
      }

      // Horizontals rushing toward the viewer (denser near the horizon).
      for (let i = 0; i < H_LINES; i++) {
        const z = ((i + scroll) % H_LINES) / H_LINES;
        const y = horizonY + z * z * (frame.height - horizonY);
        ctx.globalAlpha = 0.15 + z * 0.5;
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(frame.width, y);
        ctx.stroke();
      }

      // The light trail: a bright streak riding the grid, height from voice.
      const trailH = 2 + smooth * (horizonY - 2);
      ctx.fillStyle = frame.colors.accent;
      ctx.globalAlpha = 0.5 + smooth * 0.5;
      ctx.shadowColor = frame.colors.glow;
      ctx.shadowBlur = 8 + smooth * 16;
      const trailX = frame.reducedMotion
        ? cx
        : cx + Math.sin(frame.time * 1.3) * frame.width * 0.3;
      ctx.fillRect(trailX - 1.5, horizonY - trailH, 3, trailH);
      // Horizon glow line.
      ctx.fillStyle = frame.colors.primary;
      ctx.globalAlpha = 0.7;
      ctx.fillRect(0, horizonY - 0.5, frame.width, 1);
      ctx.shadowBlur = 0;
      ctx.globalAlpha = 1;
    },
  };
}
