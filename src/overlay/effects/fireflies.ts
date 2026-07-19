// fireflies: drifting glow-bugs that brighten and quicken as you talk.
//
// SPEC14 FR-R3: the glow is a sprite pre-rendered once per color set —
// setting shadowBlur per particle made every frame pay 16 gaussian passes,
// the most expensive per-frame pattern in the effect set. The sprite is
// built exclusively from frame.colors (the U8 rule holds).

import type { EffectColors, EffectFrame, EffectRenderer } from "./types";
import { mulberry } from "./types";

interface Fly {
  x: number;
  y: number;
  phase: number;
  drift: number;
  accent: boolean;
}

const COUNT = 16;
/** Sprite canvas size; the dot is drawn centered with room for its glow. */
const SPRITE = 32;
/** Radius of the dot baked into the sprite (glow extends beyond it). */
const SPRITE_R = 6;

function buildSprite(fill: string, glow: string): HTMLCanvasElement | null {
  const canvas = document.createElement("canvas");
  canvas.width = SPRITE;
  canvas.height = SPRITE;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null; // no offscreen 2d (test DOM): draw directly instead
  ctx.fillStyle = fill;
  ctx.shadowColor = glow;
  ctx.shadowBlur = SPRITE_R * 1.5;
  ctx.beginPath();
  ctx.arc(SPRITE / 2, SPRITE / 2, SPRITE_R, 0, Math.PI * 2);
  ctx.fill();
  return canvas;
}

export function createFireflies(): EffectRenderer {
  const rand = mulberry(99);
  let flies: Fly[] = [];
  let primarySprite: HTMLCanvasElement | null = null;
  let accentSprite: HTMLCanvasElement | null = null;
  let spriteKey = "";

  function ensureSprites(colors: EffectColors) {
    const key = `${colors.primary}|${colors.accent}|${colors.glow}`;
    if (key === spriteKey) return;
    spriteKey = key;
    primarySprite = buildSprite(colors.primary, colors.glow);
    accentSprite = buildSprite(colors.accent, colors.glow);
  }

  return {
    init(_, width, height) {
      flies = Array.from({ length: COUNT }, () => ({
        x: rand() * width,
        y: rand() * height,
        phase: rand() * Math.PI * 2,
        drift: 0.5 + rand(),
        accent: rand() > 0.65,
      }));
    },
    dispose() {
      flies = [];
      primarySprite = null;
      accentSprite = null;
      spriteKey = "";
    },
    render(ctx, frame: EffectFrame) {
      ensureSprites(frame.colors);
      const speed = frame.reducedMotion ? 3 : 9 + frame.level * 30;
      for (const fly of flies) {
        fly.phase += frame.dt * fly.drift * (frame.reducedMotion ? 0.4 : 1.2);
        fly.x += Math.cos(fly.phase) * speed * frame.dt * fly.drift;
        fly.y += Math.sin(fly.phase * 1.3) * speed * frame.dt * 0.6;
        if (fly.x < -6) fly.x = frame.width + 6;
        if (fly.x > frame.width + 6) fly.x = -6;
        if (fly.y < -6) fly.y = frame.height + 6;
        if (fly.y > frame.height + 6) fly.y = -6;

        const blink = 0.5 + Math.sin(fly.phase * 2.2) * 0.5;
        const bright = blink * (0.3 + frame.level * 0.7);
        const radius = 1.4 + bright * 1.8;
        ctx.globalAlpha = 0.15 + bright * 0.85;
        const sprite = fly.accent ? accentSprite : primarySprite;
        if (sprite) {
          // Scale the sprite so its baked dot matches the per-fly radius.
          const draw = (radius / SPRITE_R) * SPRITE;
          ctx.drawImage(sprite, fly.x - draw / 2, fly.y - draw / 2, draw, draw);
        } else {
          // Direct-draw fallback for contexts without offscreen canvases.
          ctx.fillStyle = fly.accent ? frame.colors.accent : frame.colors.primary;
          ctx.shadowColor = frame.colors.glow;
          ctx.shadowBlur = 4 + bright * 10;
          ctx.beginPath();
          ctx.arc(fly.x, fly.y, radius, 0, Math.PI * 2);
          ctx.fill();
          ctx.shadowBlur = 0;
        }
      }
      ctx.globalAlpha = 1;
    },
  };
}
