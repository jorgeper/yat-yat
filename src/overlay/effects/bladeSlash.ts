// blade-slash: a revenge flick in a pill. The racing stripe rides the
// middle and pulses with your voice; each speech burst draws a fast
// SWEEPING cut across the frame that ends in a spray burst; sustained loud
// passages add manga speed-lines; every cut flashes the stripe hot for a
// beat. Palette contract: primary carries the stripe/blade, accent the
// spray and flashes, glow the shine.

import type { EffectFrame, EffectRenderer } from "./types";
import { clamp01, lerp, mulberry } from "./types";

const MAX_SLASHES = 5;
const MAX_DROPS = 24;
const MAX_STREAKS = 8;
const SWEEP_TIME = 0.09; // seconds for the cut to cross
const LINGER = 0.4; // afterimage fade after the sweep completes

interface Slash {
  x: number;
  tilt: number; // dx per unit height
  dir: number; // sweep direction: 1 = down-left to up-right
  age: number; // -1 = dead
}

interface Drop {
  x: number;
  y: number;
  vx: number;
  vy: number;
  age: number; // -1 = dead
}

interface Streak {
  y: number;
  x: number;
  len: number;
  age: number; // -1 = dead
}

export function createBladeSlash(): EffectRenderer {
  const rand = mulberry(2004);
  const slashes: Slash[] = [];
  for (let i = 0; i < MAX_SLASHES; i++) slashes.push({ x: 0, tilt: 0, dir: 1, age: -1 });
  const drops: Drop[] = [];
  for (let i = 0; i < MAX_DROPS; i++) drops.push({ x: 0, y: 0, vx: 0, vy: 0, age: -1 });
  const streaks: Streak[] = [];
  for (let i = 0; i < MAX_STREAKS; i++) streaks.push({ y: 0, x: 0, len: 0, age: -1 });
  let prevLevel = 0;
  let smooth = 0;
  let flash = 0;

  function spawnSpray(x: number, y: number, count: number) {
    let spawned = 0;
    for (const d of drops) {
      if (spawned >= count) break;
      if (d.age >= 0) continue;
      d.x = x;
      d.y = y;
      d.vx = (rand() - 0.5) * 90;
      d.vy = -20 - rand() * 55;
      d.age = 0;
      spawned++;
    }
  }

  return {
    init() {
      prevLevel = 0;
      smooth = 0;
      flash = 0;
      for (const s of slashes) s.age = -1;
      for (const d of drops) d.age = -1;
      for (const st of streaks) st.age = -1;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      smooth = lerp(smooth, frame.level, 0.22);
      const midY = frame.height / 2;

      // ---- The racing stripe: always on, breathing with the voice. ----
      const stripeH = 3 + smooth * (frame.height * 0.3);
      const hot = clamp01(flash / 0.12);
      ctx.fillStyle = hot > 0.02 ? frame.colors.accent : frame.colors.primary;
      ctx.globalAlpha = 0.75 + smooth * 0.25;
      ctx.shadowColor = frame.colors.glow;
      ctx.shadowBlur = 4 + smooth * 8 + hot * 14;
      ctx.fillRect(0, midY - stripeH / 2, frame.width, stripeH);
      // Twin pinstripes above and below, the suit's trim.
      ctx.shadowBlur = 0;
      ctx.globalAlpha = 0.5 + smooth * 0.3;
      ctx.fillStyle = frame.colors.primary;
      ctx.fillRect(0, midY - stripeH / 2 - 3, frame.width, 1.2);
      ctx.fillRect(0, midY + stripeH / 2 + 1.8, frame.width, 1.2);
      if (!frame.reducedMotion) flash = Math.max(0, flash - frame.dt);

      // ---- Speech burst = a cut. ----
      if (!frame.reducedMotion && frame.level > 0.28 && frame.level - prevLevel > 0.12) {
        for (const s of slashes) {
          if (s.age < 0) {
            s.x = frame.width * (0.12 + rand() * 0.76);
            s.tilt = 0.5 + rand() * 0.5;
            s.dir = rand() < 0.5 ? 1 : -1;
            s.age = 0;
            flash = 0.12;
            break;
          }
        }
      }
      prevLevel = frame.level;

      for (const s of slashes) {
        if (s.age < 0) continue;
        s.age += frame.reducedMotion ? 0 : frame.dt;
        if (s.age > SWEEP_TIME + LINGER) {
          s.age = -1;
          continue;
        }
        const dx = frame.height * s.tilt + 10;
        const x0 = s.x - dx * s.dir;
        const y0 = frame.height + 3;
        const x1 = s.x + dx * s.dir;
        const y1 = -3;
        // Sweep phase: the tip races along the cut line; after that the
        // full line lingers and fades like an afterimage.
        const t = Math.min(1, s.age / SWEEP_TIME);
        const fade = s.age <= SWEEP_TIME ? 1 : 1 - (s.age - SWEEP_TIME) / LINGER;
        const tipX = x0 + (x1 - x0) * t;
        const tipY = y0 + (y1 - y0) * t;

        // Wide soft wake.
        ctx.strokeStyle = frame.colors.primary;
        ctx.lineWidth = 4;
        ctx.globalAlpha = 0.22 * fade;
        ctx.beginPath();
        ctx.moveTo(x0, y0);
        ctx.lineTo(tipX, tipY);
        ctx.stroke();
        // Hot core.
        ctx.strokeStyle = frame.colors.accent;
        ctx.lineWidth = 1.6;
        ctx.globalAlpha = fade;
        ctx.shadowColor = frame.colors.glow;
        ctx.shadowBlur = 8;
        ctx.beginPath();
        ctx.moveTo(x0, y0);
        ctx.lineTo(tipX, tipY);
        ctx.stroke();
        ctx.shadowBlur = 0;
        // The blade tip gleam while sweeping, and the spray at completion.
        if (s.age <= SWEEP_TIME) {
          ctx.fillStyle = frame.colors.accent;
          ctx.globalAlpha = 1;
          ctx.beginPath();
          ctx.arc(tipX, tipY, 2.4, 0, Math.PI * 2);
          ctx.fill();
        } else if (s.age - (frame.reducedMotion ? 0 : frame.dt) <= SWEEP_TIME) {
          spawnSpray(s.x, midY, 7);
        }
      }

      // ---- Spray drops: tiny squares under gravity. ----
      for (const d of drops) {
        if (d.age < 0) continue;
        d.age += frame.reducedMotion ? 0 : frame.dt;
        if (d.age > 0.7) {
          d.age = -1;
          continue;
        }
        d.x += d.vx * frame.dt;
        d.y += d.vy * frame.dt;
        d.vy += 320 * frame.dt;
        ctx.fillStyle = frame.colors.accent;
        ctx.globalAlpha = 0.9 * (1 - d.age / 0.7);
        ctx.fillRect(d.x, d.y, 2.2, 2.2);
      }

      // ---- Manga speed-lines while the voice runs hot. ----
      if (!frame.reducedMotion && smooth > 0.45 && rand() < 0.35) {
        for (const st of streaks) {
          if (st.age < 0) {
            st.y = rand() * frame.height;
            st.x = frame.width * (0.1 + rand() * 0.6);
            st.len = 20 + rand() * 40;
            st.age = 0;
            break;
          }
        }
      }
      for (const st of streaks) {
        if (st.age < 0) continue;
        st.age += frame.reducedMotion ? 0 : frame.dt;
        if (st.age > 0.22) {
          st.age = -1;
          continue;
        }
        st.x += frame.dt * 700;
        ctx.strokeStyle = frame.colors.primary;
        ctx.lineWidth = 1;
        ctx.globalAlpha = 0.45 * (1 - st.age / 0.22);
        ctx.beginPath();
        ctx.moveTo(st.x, st.y);
        ctx.lineTo(st.x + st.len, st.y);
        ctx.stroke();
      }

      // Reduced motion: one proud static crossed pair over the stripe.
      if (frame.reducedMotion) {
        const dx = frame.height * 0.55 + 10;
        const cx = frame.width / 2;
        ctx.strokeStyle = frame.colors.primary;
        ctx.lineWidth = 1.6;
        ctx.globalAlpha = 0.5 + smooth * 0.5;
        ctx.beginPath();
        ctx.moveTo(cx - dx, frame.height + 2);
        ctx.lineTo(cx + dx, -2);
        ctx.moveTo(cx + dx, frame.height + 2);
        ctx.lineTo(cx - dx, -2);
        ctx.stroke();
      }
      ctx.globalAlpha = 1;
    },
  };
}
