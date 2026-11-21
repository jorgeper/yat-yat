// mesh: a wireframe plane seen in perspective, like a projected terrain.
// Each sound sends a ripple out from the front edge across the grid.

import type { EffectFrame, EffectRenderer } from "./types";
import { rampAt } from "./types";

const ROWS = 7;
const COLS = 44;
const RIB_EVERY = 4;

export function createMesh(): EffectRenderer {
  // Projected grid points, row-major; allocated once.
  const xs = new Float32Array(ROWS * (COLS + 1));
  const ys = new Float32Array(ROWS * (COLS + 1));
  return {
    init() {},
    dispose() {},
    render(ctx, frame: EffectFrame) {
      const w = frame.width;
      const h = frame.height;
      const cx = w / 2;
      const history = frame.levels.length;

      for (let r = 0; r < ROWS; r++) {
        const z = r / (ROWS - 1);
        const scale = 1 - 0.42 * z;
        const base = h * (0.93 - 0.66 * Math.pow(z, 0.85));
        for (let i = 0; i <= COLS; i++) {
          const u = i / COLS;
          const distance = Math.hypot((u - 0.5) * 2, z * 0.9);
          const index = history - 1 - Math.round(distance * 34);
          const level = index >= 0 ? frame.levels[index] : 0;
          const lift = level * h * 0.46 * scale * Math.max(0, 1 - distance * 0.55);
          xs[r * (COLS + 1) + i] = cx + (u - 0.5) * w * 1.04 * scale;
          ys[r * (COLS + 1) + i] = base - lift;
        }
      }

      ctx.globalCompositeOperation = frame.colors.blend;
      ctx.lineJoin = "round";
      // Rows, far to near.
      for (let r = ROWS - 1; r >= 0; r--) {
        const z = r / (ROWS - 1);
        ctx.strokeStyle = rampAt(frame.colors, z);
        ctx.globalAlpha = 1 - z * 0.55;
        ctx.lineWidth = 1.2 - z * 0.5;
        ctx.beginPath();
        for (let i = 0; i <= COLS; i++) ctx.lineTo(xs[r * (COLS + 1) + i], ys[r * (COLS + 1) + i]);
        ctx.stroke();
      }
      // Ribs running into the distance.
      ctx.lineWidth = 0.6;
      ctx.globalAlpha = 0.4;
      for (let i = 0; i <= COLS; i += RIB_EVERY) {
        ctx.strokeStyle = rampAt(frame.colors, i / COLS);
        ctx.beginPath();
        for (let r = 0; r < ROWS; r++) ctx.lineTo(xs[r * (COLS + 1) + i], ys[r * (COLS + 1) + i]);
        ctx.stroke();
      }
      ctx.globalAlpha = 1;
      ctx.globalCompositeOperation = "source-over";
    },
  };
}
