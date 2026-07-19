// emote-face: a small companion face. Level-driven ONLY (the effect
// contract carries no pipeline state — SPEC16 FR-E1): it blinks while idle
// and its mouth opens with your voice; eyes lift a touch when you're loud.

import type { EffectFrame, EffectRenderer } from "./types";
import { lerp } from "./types";

export function createEmoteFace(): EffectRenderer {
  let smooth = 0;
  return {
    init() {
      smooth = 0;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      smooth = lerp(smooth, frame.level, 0.25);
      const cx = frame.width / 2;
      const cy = frame.height / 2;
      const eyeDx = 11;
      const eyeR = 3.2;

      // Blink: a periodic squeeze on wall time, suppressed while talking
      // and under reduced motion.
      const blinkT = frame.time % 3.7;
      const blinking =
        !frame.reducedMotion && smooth < 0.15 && blinkT > 3.45 && blinkT < 3.62;
      const eyeH = blinking ? 0.15 : 1;
      const eyeLift = smooth * 2.5;

      ctx.fillStyle = frame.colors.accent;
      ctx.globalAlpha = 0.95;
      for (const side of [-1, 1]) {
        ctx.beginPath();
        ctx.ellipse(
          cx + side * eyeDx,
          cy - 4 - eyeLift,
          eyeR,
          eyeR * eyeH,
          0,
          0,
          Math.PI * 2,
        );
        ctx.fill();
      }

      // Mouth: a resting line that opens into a rounded O with voice.
      const mouthW = 8 + smooth * 6;
      const mouthH = 0.8 + smooth * 9;
      ctx.fillStyle = frame.colors.primary;
      ctx.shadowColor = frame.colors.glow;
      ctx.shadowBlur = 4 + smooth * 10;
      ctx.beginPath();
      ctx.ellipse(cx, cy + 5, mouthW / 2, mouthH / 2, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.shadowBlur = 0;

      // Faint cheek dots when the face is "pleased" (sustained voice).
      if (smooth > 0.45) {
        ctx.fillStyle = frame.colors.glow;
        ctx.globalAlpha = (smooth - 0.45) * 0.8;
        for (const side of [-1, 1]) {
          ctx.beginPath();
          ctx.arc(cx + side * (eyeDx + 7), cy + 2, 1.8, 0, Math.PI * 2);
          ctx.fill();
        }
      }
      ctx.globalAlpha = 1;
    },
  };
}
