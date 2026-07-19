// tracker-sweep: a hand-held motion tracker — range rings, a sweeping
// wedge, and contact blips that ping inward when you speak.

import type { EffectFrame, EffectRenderer } from "./types";
import { mulberry } from "./types";

const MAX_BLIPS = 8;
const RINGS = 3;

interface Blip {
  angle: number;
  dist: number; // 0..1 of radius
  age: number; // seconds, -1 = dead
}

export function createTrackerSweep(): EffectRenderer {
  const rand = mulberry(1986);
  const blips: Blip[] = [];
  for (let i = 0; i < MAX_BLIPS; i++) blips.push({ angle: 0, dist: 0, age: -1 });
  let sweep = 0;
  let prevLevel = 0;
  return {
    init() {
      sweep = 0;
      prevLevel = 0;
      for (const b of blips) b.age = -1;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      const cx = frame.width / 2;
      const cy = frame.height * 0.95;
      const radius = frame.height * 0.88;

      // Range rings (upper half only — the pill is wide and short).
      ctx.strokeStyle = frame.colors.primary;
      ctx.lineWidth = 1;
      for (let r = 1; r <= RINGS; r++) {
        ctx.globalAlpha = 0.25;
        ctx.beginPath();
        ctx.arc(cx, cy, (radius * r) / RINGS, Math.PI, Math.PI * 2);
        ctx.stroke();
      }

      // Spawn a blip on a rising edge of the level.
      if (frame.level > 0.35 && frame.level - prevLevel > 0.12) {
        for (const b of blips) {
          if (b.age < 0) {
            b.angle = Math.PI + rand() * Math.PI;
            b.dist = 0.45 + rand() * 0.5;
            b.age = 0;
            break;
          }
        }
      }
      prevLevel = frame.level;

      // Blips creep inward and fade.
      for (const b of blips) {
        if (b.age < 0) continue;
        b.age += frame.reducedMotion ? 0 : frame.dt;
        if (b.age > 1.6) {
          b.age = -1;
          continue;
        }
        const d = b.dist * (1 - b.age * 0.25) * radius;
        const bx = cx + Math.cos(b.angle) * d;
        const by = cy + Math.sin(b.angle) * d;
        ctx.fillStyle = frame.colors.accent;
        ctx.globalAlpha = 0.95 * (1 - b.age / 1.6);
        ctx.shadowColor = frame.colors.glow;
        ctx.shadowBlur = 8;
        ctx.beginPath();
        ctx.arc(bx, by, 2.2, 0, Math.PI * 2);
        ctx.fill();
        ctx.shadowBlur = 0;
      }

      // The sweep wedge.
      if (!frame.reducedMotion) {
        sweep = (sweep + frame.dt * (1.1 + frame.level * 1.6)) % 1;
      }
      const a = Math.PI + sweep * Math.PI;
      ctx.strokeStyle = frame.colors.primary;
      ctx.globalAlpha = 0.9;
      ctx.lineWidth = 1.5;
      ctx.shadowColor = frame.colors.glow;
      ctx.shadowBlur = 10;
      ctx.beginPath();
      ctx.moveTo(cx, cy);
      ctx.lineTo(cx + Math.cos(a) * radius, cy + Math.sin(a) * radius);
      ctx.stroke();
      // Fading wedge behind the sweep line.
      for (let i = 1; i <= 5; i++) {
        const wa = a - i * 0.07;
        if (wa < Math.PI) break;
        ctx.globalAlpha = 0.18 * (1 - i / 6);
        ctx.beginPath();
        ctx.moveTo(cx, cy);
        ctx.lineTo(cx + Math.cos(wa) * radius, cy + Math.sin(wa) * radius);
        ctx.stroke();
      }
      ctx.shadowBlur = 0;
      ctx.globalAlpha = 1;
    },
  };
}
