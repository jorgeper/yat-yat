// holo-arcs: concentric holographic arcs spinning at different speeds,
// expanding and brightening as the assistant hears you.

import type { EffectFrame, EffectRenderer } from "./types";
import { lerp } from "./types";

const ARCS = 4;

export function createHoloArcs(): EffectRenderer {
  const phases: number[] = [0, 1.7, 3.9, 5.2];
  let smooth = 0;
  return {
    init() {
      smooth = 0;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      smooth = lerp(smooth, frame.level, 0.15);
      const cx = frame.width / 2;
      const cy = frame.height / 2;
      const maxR = frame.height / 2 - 2 + smooth * 4;

      for (let i = 0; i < ARCS; i++) {
        const dir = i % 2 === 0 ? 1 : -1;
        if (!frame.reducedMotion) {
          phases[i] += frame.dt * dir * (0.8 + i * 0.35) * (0.6 + smooth * 1.8);
        }
        const r = maxR * ((i + 1.2) / (ARCS + 0.5));
        const span = frame.reducedMotion ? Math.PI * 2 : 0.9 + smooth * 2.2 - i * 0.12;
        ctx.strokeStyle = i % 2 === 0 ? frame.colors.primary : frame.colors.accent;
        ctx.globalAlpha = frame.reducedMotion ? 0.3 : 0.45 + smooth * 0.5;
        ctx.lineWidth = i === 0 ? 2 : 1.2;
        ctx.shadowColor = frame.colors.glow;
        ctx.shadowBlur = 6 + smooth * 10;
        ctx.beginPath();
        ctx.arc(cx, cy, r, phases[i], phases[i] + span);
        ctx.stroke();
        // A counterweight tick opposite each arc, instrument-style.
        ctx.globalAlpha *= 0.5;
        ctx.beginPath();
        ctx.arc(cx, cy, r, phases[i] + Math.PI, phases[i] + Math.PI + 0.25);
        ctx.stroke();
      }
      // Reactor core.
      ctx.fillStyle = frame.colors.accent;
      ctx.globalAlpha = 0.75 + smooth * 0.25;
      ctx.shadowBlur = 10 + smooth * 14;
      ctx.beginPath();
      ctx.arc(cx, cy, 2.2 + smooth * 2.5, 0, Math.PI * 2);
      ctx.fill();
      ctx.shadowBlur = 0;
      ctx.globalAlpha = 1;
    },
  };
}
