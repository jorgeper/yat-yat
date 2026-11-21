// ribbon: four translucent ribbons of silk that twist through each other
// and swell with your voice.

import type { EffectFrame, EffectRenderer } from "./types";
import { lerp, rampAt } from "./types";

const LAYERS = 4;
const STEP = 3;

function ribbonY(x: number, w: number, k: number, phase: number, amp: number): number {
  const envelope = Math.sin((Math.PI * x) / w);
  return Math.sin(x * k + phase) * amp * envelope * envelope;
}

export function createRibbon(): EffectRenderer {
  let smooth = 0;
  return {
    init() {
      smooth = 0;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      const w = frame.width;
      const h = frame.height;
      const cy = h / 2;
      const t = frame.reducedMotion ? 0 : frame.time;
      smooth = lerp(smooth, frame.level, 0.12);

      ctx.globalCompositeOperation = frame.colors.blend;
      ctx.globalAlpha = 0.6;
      for (let layer = 0; layer < LAYERS; layer++) {
        const amp = (0.1 + smooth * 0.95) * (h / 2) * (1 - layer * 0.14);
        const k = ((1.2 + layer * 0.55) * Math.PI * 2) / w;
        const speed = (1.1 + layer * 0.5) * (layer % 2 ? -1 : 1);
        const phase = t * speed * 2 + layer * 1.7;
        ctx.beginPath();
        for (let x = 0; x <= w; x += STEP) ctx.lineTo(x, cy - ribbonY(x, w, k, phase, amp));
        for (let x = w; x >= 0; x -= STEP) ctx.lineTo(x, cy + ribbonY(x, w, k, phase, amp) * 0.4);
        ctx.fillStyle = rampAt(frame.colors, layer / (LAYERS - 1));
        ctx.fill();
      }
      ctx.globalAlpha = 1;
      ctx.globalCompositeOperation = "source-over";
    },
  };
}
