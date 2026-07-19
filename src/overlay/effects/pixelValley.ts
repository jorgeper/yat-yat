// pixel-valley: a cozy pixel farm. The pill background is the sky; chunky
// clouds drift by, pixel hills roll with your voice history, and little
// crops sprout on strong syllables. Everything snaps to a coarse grid so
// it reads as pixel art at pill size.

import type { EffectFrame, EffectRenderer } from "./types";
import { levelAt, lerp, mulberry } from "./types";

const CELL = 4;
const MAX_CROPS = 9;
const CROP_LIFE = 4.5;

interface Crop {
  col: number;
  height: number; // cells
  age: number; // -1 = dead
}

const snap = (v: number) => Math.floor(v / CELL) * CELL;

export function createPixelValley(): EffectRenderer {
  const rand = mulberry(2016);
  const crops: Crop[] = [];
  for (let i = 0; i < MAX_CROPS; i++) crops.push({ col: 0, height: 1, age: -1 });
  // Cloud shapes: per-cloud row spans (in cells), fixed at init.
  const clouds = [
    { x: 8, y: 1, spans: [3, 5, 4] },
    { x: 90, y: 2, spans: [4, 6, 3] },
    { x: 170, y: 1, spans: [2, 4, 4] },
  ];
  let drift = 0;
  let smooth = 0;
  let prevLevel = 0;
  return {
    init() {
      drift = 0;
      smooth = 0;
      prevLevel = 0;
      for (const c of crops) c.age = -1;
    },
    dispose() {},
    render(ctx, frame: EffectFrame) {
      smooth = lerp(smooth, frame.level, 0.12);
      const cols = Math.max(1, Math.floor(frame.width / CELL));

      // The sun: a chunky corner square with a soft warm halo.
      ctx.fillStyle = frame.colors.accent;
      ctx.globalAlpha = 0.95;
      ctx.shadowColor = frame.colors.glow;
      ctx.shadowBlur = 8 + smooth * 8;
      ctx.fillRect(frame.width - CELL * 4, CELL, CELL * 2, CELL * 2);
      ctx.shadowBlur = 0;

      // Clouds drift lazily; speed barely cares about voice — clouds don't.
      if (!frame.reducedMotion) {
        drift += frame.dt * 3.5;
      }
      ctx.fillStyle = frame.colors.glow;
      for (const c of clouds) {
        const cx = (c.x + drift) % (frame.width + CELL * 8) - CELL * 6;
        ctx.globalAlpha = 0.85;
        for (let r = 0; r < c.spans.length; r++) {
          const span = c.spans[r];
          ctx.fillRect(snap(cx + (r % 2) * CELL), (c.y + r) * CELL, span * CELL, CELL);
        }
      }

      // Rolling hills: two layered strips of field, heights from the level
      // history, snapped to cells so they read as pixel terrain.
      ctx.fillStyle = frame.colors.primary;
      for (let i = 0; i < cols; i++) {
        const lv = levelAt(frame.levels, cols, i);
        const back = 2 + Math.round(lv * 2);
        ctx.globalAlpha = 0.55;
        ctx.fillRect(i * CELL, frame.height - back * CELL - CELL, CELL, back * CELL);
        const front = 1 + Math.round((lv + smooth) * 1.5);
        ctx.globalAlpha = 1;
        ctx.fillRect(i * CELL, frame.height - front * CELL, CELL, front * CELL);
      }

      // Crops sprout on a rising edge and slowly ripen away.
      if (!frame.reducedMotion && frame.level > 0.3 && frame.level - prevLevel > 0.12) {
        for (const c of crops) {
          if (c.age < 0) {
            c.col = Math.floor(rand() * cols);
            c.height = 2 + Math.floor(rand() * 2);
            c.age = 0;
            break;
          }
        }
      }
      prevLevel = frame.level;

      for (const c of crops) {
        if (c.age < 0) continue;
        c.age += frame.reducedMotion ? 0 : frame.dt;
        if (c.age > CROP_LIFE) {
          c.age = -1;
          continue;
        }
        // Stem grows in over the first half-second, cell by cell.
        const grown = Math.min(c.height, 1 + Math.floor(c.age * 4));
        const x = c.col * CELL;
        const fade = c.age > CROP_LIFE - 1 ? CROP_LIFE - c.age : 1;
        ctx.fillStyle = frame.colors.primary;
        ctx.globalAlpha = fade;
        ctx.fillRect(x, frame.height - CELL * (1 + grown), CELL, CELL * grown);
        // The blossom on a fully grown crop.
        if (grown >= c.height) {
          ctx.fillStyle = frame.colors.accent;
          ctx.globalAlpha = 0.95 * fade;
          ctx.fillRect(x, frame.height - CELL * (2 + c.height) + CELL, CELL, CELL);
        }
      }
      ctx.globalAlpha = 1;
    },
  };
}
