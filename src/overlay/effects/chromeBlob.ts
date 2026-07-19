// chrome-blob: mercury-like blobs that ripple apart as you speak and pool
// back together in silence. Shading is faked with layered alpha over the
// theme's primary, an accent specular dot, and the glow as ambient light.

import type { EffectFrame, EffectRenderer } from "./types";
import { lerp } from "./types";

const BLOBS = 4;

interface Blob {
  seed: number;
  x: number;
  y: number;
  r: number;
}

export function createChromeBlob(): EffectRenderer {
  const blobs: Blob[] = [];
  for (let i = 0; i < BLOBS; i++) {
    blobs.push({ seed: i * 1.7 + 0.9, x: 0, y: 0, r: 0 });
  }
  let smooth = 0;
  return {
    init() {
      smooth = 0;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      smooth = lerp(smooth, frame.level, 0.12);
      const cx = frame.width / 2;
      const cy = frame.height / 2;
      const spread = smooth * frame.width * 0.28;
      const baseR = frame.height * 0.22 + smooth * frame.height * 0.16;

      // Position the pool: blobs drift out with voice, home to center calm.
      for (let i = 0; i < BLOBS; i++) {
        const b = blobs[i];
        const wob = frame.reducedMotion ? 0 : Math.sin(frame.time * (0.9 + b.seed * 0.31) + b.seed * 5);
        b.x = cx + (i - (BLOBS - 1) / 2) * (spread * 0.8) + wob * (2 + smooth * 8);
        b.y =
          cy +
          (frame.reducedMotion ? 0 : Math.cos(frame.time * (1.1 + b.seed * 0.23) + b.seed * 3)) *
            (1 + smooth * 4);
        b.r = baseR * (0.7 + 0.3 * Math.sin(b.seed * 7 + (frame.reducedMotion ? 0 : frame.time)));
      }

      // Body passes: two stacked alpha layers merge overlapping circles into
      // one silvery mass.
      for (let pass = 0; pass < 2; pass++) {
        ctx.fillStyle = frame.colors.primary;
        ctx.globalAlpha = pass === 0 ? 0.35 : 0.55;
        ctx.shadowColor = frame.colors.glow;
        ctx.shadowBlur = pass === 0 ? 14 : 4;
        for (const b of blobs) {
          ctx.beginPath();
          ctx.arc(b.x, b.y, b.r * (pass === 0 ? 1.15 : 0.9), 0, Math.PI * 2);
          ctx.fill();
        }
      }
      ctx.shadowBlur = 0;

      // Specular highlights: a small hot dot up-left on each blob.
      ctx.fillStyle = frame.colors.accent;
      for (const b of blobs) {
        ctx.globalAlpha = 0.7;
        ctx.beginPath();
        ctx.arc(b.x - b.r * 0.3, b.y - b.r * 0.35, Math.max(0.8, b.r * 0.18), 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.globalAlpha = 1;
    },
  };
}
