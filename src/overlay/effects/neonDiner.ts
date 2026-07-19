// neon-diner: a flickering neon tube traces the pill while a warm glow
// wells up from inside as you speak. You know what's in the case.

import type { EffectFrame, EffectRenderer } from "./types";
import { lerp, mulberry } from "./types";

export function createNeonDiner(): EffectRenderer {
  const rand = mulberry(1994);
  // A fixed schedule of flicker dips (seconds within a repeating minute).
  const dips: number[] = [];
  for (let i = 0; i < 12; i++) dips.push(rand() * 60);
  let smooth = 0;
  return {
    init() {
      smooth = 0;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      smooth = lerp(smooth, frame.level, 0.12);
      const inset = 3;
      const w = frame.width - inset * 2;
      const h = frame.height - inset * 2;
      const r = Math.min(h / 2, 12);

      // Flicker: brief alpha dips at scheduled moments (steady under
      // reduced motion — a neon sign that finally got fixed).
      let tube = 1;
      if (!frame.reducedMotion) {
        const t = frame.time % 60;
        for (const d of dips) {
          const dd = Math.abs(t - d);
          if (dd < 0.09) tube = Math.min(tube, 0.25 + dd * 6);
        }
      }

      // Inner welling glow — the case light.
      ctx.fillStyle = frame.colors.accent;
      ctx.globalAlpha = smooth * 0.5;
      ctx.shadowColor = frame.colors.glow;
      ctx.shadowBlur = 18 + smooth * 22;
      ctx.beginPath();
      ctx.ellipse(
        frame.width / 2,
        frame.height * 0.72,
        w * 0.28 + smooth * w * 0.1,
        h * 0.3,
        0,
        0,
        Math.PI * 2,
      );
      ctx.fill();
      ctx.shadowBlur = 0;

      // The tube: rounded-rect outline via lines + corner arcs (hand-rolled
      // so older WebKit canvases stay happy).
      ctx.strokeStyle = frame.colors.primary;
      ctx.lineWidth = 2;
      ctx.globalAlpha = tube * (0.65 + smooth * 0.35);
      ctx.shadowColor = frame.colors.glow;
      ctx.shadowBlur = 8 + smooth * 10;
      ctx.beginPath();
      ctx.moveTo(inset + r, inset);
      ctx.lineTo(inset + w - r, inset);
      ctx.arc(inset + w - r, inset + r, r, -Math.PI / 2, 0);
      ctx.lineTo(inset + w, inset + h - r);
      ctx.arc(inset + w - r, inset + h - r, r, 0, Math.PI / 2);
      ctx.lineTo(inset + r, inset + h);
      ctx.arc(inset + r, inset + h - r, r, Math.PI / 2, Math.PI);
      ctx.lineTo(inset, inset + r);
      ctx.arc(inset + r, inset + r, r, Math.PI, Math.PI * 1.5);
      ctx.stroke();
      // A second, tighter pass makes the tube read as glass.
      ctx.lineWidth = 0.8;
      ctx.strokeStyle = frame.colors.accent;
      ctx.globalAlpha = tube * 0.8;
      ctx.shadowBlur = 0;
      ctx.stroke();
      ctx.globalAlpha = 1;
    },
  };
}
