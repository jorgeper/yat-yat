// heartbeat: a hospital-monitor trace scrolling right to left — flat and calm
// in silence, sharp EKG-style spikes that grow with your voice. Replaces the
// old vu-needle (radial meters read poorly in the wide, short pill).

import type { EffectFrame, EffectRenderer } from "./types";
import { clamp01 } from "./types";

const STEP_PX = 2; // one sample every 2 px
const BEAT_PERIOD = 9; // samples between QRS kicks while speaking

export function createHeartbeat(): EffectRenderer {
  let samples: number[] = [];
  let acc = 0;
  let tick = 0;
  let smoothed = 0;

  const ensureCapacity = (width: number) => {
    const cols = Math.max(8, Math.ceil(width / STEP_PX) + 1);
    if (samples.length < cols) {
      samples = new Array(cols - samples.length).fill(0).concat(samples);
    } else if (samples.length > cols) {
      samples = samples.slice(samples.length - cols);
    }
  };

  return {
    init(_ctx, width) {
      samples = [];
      acc = 0;
      tick = 0;
      smoothed = 0;
      ensureCapacity(width);
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      const { width, height, colors, reducedMotion } = frame;
      ensureCapacity(width);

      // Fast attack, gentle release — spikes land within one beat of speech.
      const target = clamp01(frame.level);
      smoothed = target > smoothed ? smoothed + (target - smoothed) * 0.5 : smoothed * 0.94;

      // Advance the trace; reduced motion scrolls gently instead of racing.
      const rate = reducedMotion ? 24 : 110; // samples per second
      acc += frame.dt * rate;
      while (acc >= 1) {
        acc -= 1;
        tick += 1;
        // A compact QRS-like complex: sharp rise, deep overshoot, settle.
        const k = tick % BEAT_PERIOD;
        const kick = k === 0 ? 1 : k === 1 ? -0.4 : k === 2 ? 0.14 : 0;
        const idleWander = Math.sin(tick * 0.7) * 0.04;
        samples.push(kick * smoothed + idleWander * (1 - smoothed));
        samples.shift();
      }

      const mid = height * 0.55;
      const amp = height * 0.42;
      const x0 = width - (samples.length - 1) * STEP_PX;

      // Faint baseline, like the monitor's grid line.
      ctx.strokeStyle = colors.primary;
      ctx.globalAlpha = 0.18;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(0, mid);
      ctx.lineTo(width, mid);
      ctx.stroke();

      // The trace — newest sample at the right edge, tail fading left.
      ctx.lineWidth = 1.8;
      ctx.lineJoin = "round";
      const fadeSpan = Math.max(1, samples.length - 1);
      for (let i = 1; i < samples.length; i++) {
        const t = i / fadeSpan; // 0 oldest .. 1 newest
        ctx.strokeStyle = colors.primary;
        ctx.globalAlpha = 0.15 + 0.75 * t * t;
        ctx.beginPath();
        ctx.moveTo(x0 + (i - 1) * STEP_PX, mid - samples[i - 1] * amp);
        ctx.lineTo(x0 + i * STEP_PX, mid - samples[i] * amp);
        ctx.stroke();
      }

      // Bright leading dot with a voice-sized glow.
      const headY = mid - samples[samples.length - 1] * amp;
      ctx.fillStyle = colors.accent;
      ctx.globalAlpha = 0.95;
      if (!reducedMotion) {
        ctx.shadowColor = colors.glow;
        ctx.shadowBlur = 4 + smoothed * 12;
      }
      ctx.beginPath();
      ctx.arc(width - 1.5, headY, 2.2 + smoothed * 1.4, 0, Math.PI * 2);
      ctx.fill();
      ctx.shadowBlur = 0;
      ctx.globalAlpha = 1;
    },
  };
}
