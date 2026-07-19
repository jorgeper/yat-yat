// elbow-panel: a rounded elbow console frame with segmented data bars
// filling from the left as you speak, and a ticking numeric readout block.

import type { EffectFrame, EffectRenderer } from "./types";
import { levelAt, lerp } from "./types";

const SEG_W = 5;
const SEG_GAP = 2;

export function createElbowPanel(): EffectRenderer {
  let smooth = 0;
  return {
    init() {
      smooth = 0;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      smooth = lerp(smooth, frame.level, 0.15);
      const elbowW = 16;
      const railH = 4;

      // The elbow: a fat left column joining a thin top rail — the classic
      // console frame silhouette, drawn as two rects and a joining arc.
      ctx.fillStyle = frame.colors.primary;
      ctx.globalAlpha = 0.85;
      ctx.fillRect(0, 0, elbowW * 0.55, frame.height);
      ctx.fillRect(elbowW * 0.5, 0, frame.width * 0.45, railH);
      ctx.beginPath();
      ctx.arc(elbowW * 0.55 + 6, railH + 6, 6, Math.PI, Math.PI * 1.5);
      ctx.lineWidth = railH;
      ctx.strokeStyle = frame.colors.primary;
      ctx.stroke();
      // Rail end cap in the second panel color.
      ctx.fillStyle = frame.colors.accent;
      ctx.fillRect(frame.width * 0.45 + elbowW * 0.5 + 3, 0, 14, railH);

      // Segmented data bars: recent level history as chunky fills.
      const barX = elbowW + 4;
      const barW = frame.width - barX - 30;
      const count = Math.max(1, Math.floor(barW / (SEG_W + SEG_GAP)));
      const barY = railH + 5;
      const barH = frame.height - barY - 3;
      for (let i = 0; i < count; i++) {
        const lv = levelAt(frame.levels, count, i);
        const h = Math.max(1.5, lv * barH);
        ctx.fillStyle = i % 4 === 3 ? frame.colors.accent : frame.colors.primary;
        ctx.globalAlpha = 0.35 + lv * 0.6;
        ctx.fillRect(barX + i * (SEG_W + SEG_GAP), barY + (barH - h), SEG_W, h);
      }

      // Readout block: little data cells whose alpha ticks over time.
      const roX = frame.width - 24;
      for (let r = 0; r < 3; r++) {
        const tick = frame.reducedMotion
          ? 0.6
          : 0.35 + 0.55 * Math.abs(Math.sin(frame.time * (2 + r) + r * 2));
        ctx.fillStyle = r === 1 ? frame.colors.accent : frame.colors.primary;
        ctx.globalAlpha = tick * (0.5 + smooth * 0.5);
        ctx.fillRect(roX, railH + 4 + r * 8, 20, 5);
      }
      ctx.globalAlpha = 1;
    },
  };
}
