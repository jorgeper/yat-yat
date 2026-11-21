// trace: a seismograph pen. Your voice is drawn as one fine line that
// scrolls away to the left and fades.

import type { EffectFrame, EffectRenderer } from "./types";
import { rampAt } from "./types";

const CAPACITY = 512;
const SAMPLE_RATE = 90;
const DX = 1.4;
const FADE_BANDS = 6;
const PEN_INSET = 8;

export function createTrace(): EffectRenderer {
  const samples = new Float32Array(CAPACITY);
  let head = 0; // index of the next write
  let count = 0;
  let pending = 0;
  let phase = 0;
  return {
    init() {
      head = 0;
      count = 0;
      pending = 0;
      phase = 0;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      const w = frame.width;
      const h = frame.height;
      const cy = h / 2;
      const visible = Math.min(CAPACITY, Math.max(2, Math.ceil((w - PEN_INSET - 2) / DX)));

      pending += frame.dt;
      while (pending > 1 / SAMPLE_RATE) {
        pending -= 1 / SAMPLE_RATE;
        phase += (18 + frame.level * 26) / SAMPLE_RATE;
        samples[head] = frame.level * Math.sin(phase) * (0.75 + 0.25 * Math.sin(phase * 0.37));
        head = (head + 1) % CAPACITY;
        count = Math.min(CAPACITY, count + 1);
      }

      // Baseline ticks.
      ctx.globalAlpha = 0.22;
      ctx.fillStyle = rampAt(frame.colors, 0);
      for (let x = 4; x < w - PEN_INSET; x += 8) ctx.fillRect(x, cy - 0.5, 3, 1);

      // The line, oldest band first, each band a little more opaque.
      const shown = Math.min(count, visible);
      const penX = w - PEN_INSET;
      let penY = cy;
      ctx.lineWidth = 1.6;
      ctx.lineJoin = "round";
      ctx.lineCap = "round";
      const perBand = Math.max(1, Math.ceil(shown / FADE_BANDS));
      for (let band = 0; band < FADE_BANDS; band++) {
        const from = band * perBand;
        if (from >= shown) break;
        const to = Math.min(shown - 1, from + perBand);
        const v = (band + 1) / FADE_BANDS;
        ctx.globalAlpha = v * v;
        ctx.strokeStyle = rampAt(frame.colors, v);
        ctx.beginPath();
        for (let i = from; i <= to; i++) {
          const sample = samples[(head - shown + i + CAPACITY * 2) % CAPACITY];
          penY = cy - sample * h * 0.46;
          ctx.lineTo(penX - (shown - 1 - i) * DX, penY);
        }
        ctx.stroke();
      }

      // The pen head.
      ctx.globalAlpha = 1;
      ctx.fillStyle = rampAt(frame.colors, 1);
      ctx.shadowColor = frame.colors.glow;
      ctx.shadowBlur = 6;
      ctx.beginPath();
      ctx.arc(penX, penY, 2.4, 0, Math.PI * 2);
      ctx.fill();
      ctx.shadowBlur = 0;
    },
  };
}
