// constellation: drifting points that link up into a web when they come
// close. Voice speeds them up and lengthens their reach.

import type { EffectFrame, EffectRenderer } from "./types";
import { clamp01, lerp, mulberry, rampAt } from "./types";

const COUNT = 30;
const INSET = 3;

interface Point {
  x: number; // 0..1 across the canvas
  y: number; // 0..1 down the canvas
  vx: number;
  vy: number;
}

export function createConstellation(): EffectRenderer {
  const points: Point[] = [];
  const random = mulberry(0xc0457e);
  let smooth = 0;
  return {
    init() {
      smooth = 0;
      points.length = 0;
      for (let i = 0; i < COUNT; i++) {
        const heading = random() * Math.PI * 2;
        points.push({
          x: random(),
          y: random(),
          vx: Math.cos(heading) * 0.12,
          vy: Math.sin(heading) * 0.3,
        });
      }
    },
    dispose() {
      points.length = 0;
    },
    render(ctx, frame: EffectFrame) {
      const w = frame.width;
      const span = Math.max(1, frame.height - INSET * 2);
      smooth = lerp(smooth, frame.level, 0.1);
      const speed = frame.reducedMotion ? 0 : 0.25 + smooth * 2.6;
      const reach = 20 + smooth * 40;

      for (const p of points) {
        p.x += p.vx * frame.dt * speed;
        p.y += p.vy * frame.dt * speed;
        if (p.x < 0 || p.x > 1) p.vx = -p.vx;
        if (p.y < 0 || p.y > 1) p.vy = -p.vy;
        p.x = clamp01(p.x);
        p.y = clamp01(p.y);
      }

      ctx.lineWidth = 0.9;
      for (let i = 0; i < points.length; i++) {
        for (let j = i + 1; j < points.length; j++) {
          const a = points[i];
          const b = points[j];
          const distance = Math.hypot((a.x - b.x) * w, (a.y - b.y) * span);
          if (distance >= reach) continue;
          ctx.strokeStyle = rampAt(frame.colors, (i + j) / (2 * COUNT));
          ctx.globalAlpha = (1 - distance / reach) * 0.75;
          ctx.beginPath();
          ctx.moveTo(a.x * w, INSET + a.y * span);
          ctx.lineTo(b.x * w, INSET + b.y * span);
          ctx.stroke();
        }
      }
      ctx.globalAlpha = 1;
      for (let i = 0; i < points.length; i++) {
        ctx.fillStyle = rampAt(frame.colors, i / (COUNT - 1));
        ctx.beginPath();
        ctx.arc(points[i].x * w, INSET + points[i].y * span, 1.1 + smooth * 1.5, 0, Math.PI * 2);
        ctx.fill();
      }
    },
  };
}
