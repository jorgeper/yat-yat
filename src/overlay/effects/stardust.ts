// stardust: a stream of glowing motes drifting across the pill, colored by
// height. Speech turns the drift into a rush and the brightest ones flare.

import type { EffectFrame, EffectRenderer } from "./types";
import { lerp, mulberry, rampAt } from "./types";

const COUNT = 90;

interface Mote {
  x: number;
  y: number;
  depth: number;
  phase: number;
}

export function createStardust(): EffectRenderer {
  const motes: Mote[] = [];
  const random = mulberry(0x57a2d);
  let smooth = 0;
  return {
    init(_ctx, width, height) {
      smooth = 0;
      motes.length = 0;
      for (let i = 0; i < COUNT; i++) {
        motes.push({
          x: random() * width,
          y: 2 + random() * Math.max(1, height - 4),
          depth: random(),
          phase: random() * Math.PI * 2,
        });
      }
    },
    dispose() {
      motes.length = 0;
    },
    render(ctx, frame: EffectFrame) {
      const w = frame.width;
      const h = frame.height;
      const t = frame.time;
      smooth = lerp(smooth, frame.level, 0.1);

      ctx.globalCompositeOperation = frame.colors.blend;
      ctx.lineCap = "round";
      for (const m of motes) {
        const vx = (10 + smooth * 110) * (0.4 + m.depth);
        const vy = Math.sin(m.x * 0.05 + t * 1.2 + m.phase) * (3 + smooth * 16);
        if (!frame.reducedMotion) {
          m.x += vx * frame.dt;
          m.y += vy * frame.dt;
          if (m.x > w + 4) {
            m.x = -4;
            m.y = 2 + random() * Math.max(1, h - 4);
          }
          m.y = Math.max(1, Math.min(h - 1, m.y));
        }
        const twinkle = frame.reducedMotion
          ? 0.8
          : 0.45 + 0.55 * Math.sin(t * 5 * (0.5 + m.depth) + m.phase);
        const tail = 0.03 + smooth * 0.07;
        ctx.strokeStyle = rampAt(frame.colors, m.y / h);
        ctx.globalAlpha = 0.35 + 0.6 * twinkle;
        ctx.lineWidth = 0.6 + m.depth * 1.3;
        ctx.beginPath();
        ctx.moveTo(m.x, m.y);
        ctx.lineTo(m.x - vx * tail, m.y - vy * tail);
        ctx.stroke();
        if (m.depth > 0.86) {
          const reach = 2 + twinkle * (2 + smooth * 4);
          ctx.globalAlpha = 0.8 * twinkle;
          ctx.lineWidth = 0.7;
          ctx.beginPath();
          ctx.moveTo(m.x - reach, m.y);
          ctx.lineTo(m.x + reach, m.y);
          ctx.moveTo(m.x, m.y - reach);
          ctx.lineTo(m.x, m.y + reach);
          ctx.stroke();
        }
      }
      ctx.globalAlpha = 1;
      ctx.globalCompositeOperation = "source-over";
    },
  };
}
