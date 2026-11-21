// The "Writing" figure (owner-picked 2026-11-20): what the box shows while a
// dictation is transcribed. The effect's last frame breaks into sparks, the
// sparks gather into a text cursor, and the cursor runs across the box
// laying down two lines of "words". There is no progress bar — when the
// pipeline reports progress, the cursor's position IS the progress;
// otherwise it writes the lines over and over.
//
// Colors come from the theme exactly as for effects (resolveColors + the
// ramp); the geometry is pure and unit-tested (U27).

import { resolveColors } from "./effects/engine";
import type { EffectColors } from "./effects/types";
import { clamp01, lerp, mulberry, rampAt } from "./effects/types";

export const SPARKS = 150;
/** Seconds the sparks take to fly from the effect into the figure. */
const MORPH_S = 0.6;
const MAX_FPS = 60;
const REDUCED_FPS = 30;

export interface Spark {
  x: number;
  y: number;
  radius: number;
  alpha: number;
  /** Position along the color ramp, 0..1. */
  tone: number;
}

/**
 * Where the cursor is. With reported progress the cursor tracks it (and is
 * `done` only at completion); without, it sweeps the box every 2.6 s and
 * rests, blinking, for the last 0.6 s of each sweep.
 */
export function cursorAt(seconds: number, progress: number): { fraction: number; done: boolean } {
  if (progress > 0) return { fraction: clamp01(progress), done: progress >= 1 };
  const loop = (Math.max(0, seconds) * 0.5) % 1.3;
  return { fraction: Math.min(1, loop), done: loop > 1 };
}

/**
 * The resting place of spark `i` of `count`: on one of two text lines if the
 * cursor has already passed it, otherwise stacked in the cursor itself. A
 * fifth of the sparks never leave the cursor — those are the gaps between
 * words. `jitter` is the spark's own 0..1 random number.
 */
export function writingSpark(
  i: number,
  count: number,
  fraction: number,
  done: boolean,
  blinkOn: boolean,
  width: number,
  height: number,
  jitter: number,
  out: Spark,
): Spark {
  const u = i / count;
  const line = i % 2;
  const left = width * 0.05;
  const span = width * 0.88;
  const cursorX = left + clamp01(fraction) * span;
  const homeX = left + u * span;
  const isGap = (u * 6 + line * 0.37) % 1 > 0.8;
  if (!isGap && homeX <= cursorX - 2) {
    out.x = homeX;
    out.y = height / 2 + (line ? 1 : -1) * height * 0.15;
    out.radius = 1.05;
    out.alpha = 0.9;
    out.tone = u;
  } else {
    out.x = cursorX + (jitter - 0.5) * 1.2;
    out.y = height / 2 + (((i * 0.618) % 1) - 0.5) * height * 0.62;
    out.radius = 0.9;
    out.alpha = done && !blinkOn ? 0.15 : 0.95;
    out.tone = 1;
  }
  return out;
}

const easeOut = (t: number) => (t <= 0 ? 0 : t >= 1 ? 1 : 1 - Math.pow(1 - t, 3));

/** Copy a canvas so its last frame survives the element being unmounted. */
export function snapshotCanvas(source: HTMLCanvasElement): HTMLCanvasElement | null {
  if (!source.width || !source.height) return null;
  const copy = document.createElement("canvas");
  copy.width = source.width;
  copy.height = source.height;
  const ctx = copy.getContext("2d");
  if (!ctx) return null;
  ctx.drawImage(source, 0, 0);
  return copy;
}

export class ThinkingFigure {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D | null;
  private colors: EffectColors | null = null;
  private snapshot: HTMLCanvasElement | null = null;
  private startX = new Float32Array(SPARKS);
  private startY = new Float32Array(SPARKS);
  private jitter = new Float32Array(SPARKS);
  private spark: Spark = { x: 0, y: 0, radius: 1, alpha: 1, tone: 0 };
  private width = 0;
  private height = 0;
  private raf = 0;
  private running = false;
  private startTs = -1;
  private lastTs = -1;
  private progress = 0;
  private reducedMotion: boolean;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d");
    this.reducedMotion =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  setProgress(progress: number) {
    this.progress = progress;
  }

  /** `snapshot` is the effect's last frame, or null when there was none. */
  start(snapshot: HTMLCanvasElement | null) {
    if (this.running || !this.ctx) return;
    const rect = this.canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    this.width = Math.max(1, Math.round(rect.width));
    this.height = Math.max(1, Math.round(rect.height));
    this.canvas.width = this.width * dpr;
    this.canvas.height = this.height * dpr;
    this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.colors = resolveColors(this.canvas, this.ctx);
    this.snapshot = snapshot;
    this.seed(snapshot);
    this.running = true;
    this.startTs = -1;
    this.lastTs = -1;
    const tick = (ts: number) => {
      if (!this.running) return;
      this.raf = requestAnimationFrame(tick);
      this.step(ts);
    };
    this.raf = requestAnimationFrame(tick);
  }

  /** Start each spark on a pixel the effect had drawn, when there is one. */
  private seed(snapshot: HTMLCanvasElement | null) {
    const random = mulberry(0x5eed);
    let data: Uint8ClampedArray | null = null;
    let sw = 0;
    let sh = 0;
    if (snapshot) {
      try {
        sw = snapshot.width;
        sh = snapshot.height;
        data = snapshot.getContext("2d")?.getImageData(0, 0, sw, sh).data ?? null;
      } catch {
        data = null;
      }
    }
    for (let i = 0; i < SPARKS; i++) {
      let x = random() * this.width;
      let y = this.height / 2 + (random() - 0.5) * 4;
      if (data) {
        for (let attempt = 0; attempt < 40; attempt++) {
          const px = Math.floor(random() * sw);
          const py = Math.floor(random() * sh);
          if (data[(py * sw + px) * 4 + 3] > 50) {
            x = (px / sw) * this.width;
            y = (py / sh) * this.height;
            break;
          }
        }
      }
      this.startX[i] = x;
      this.startY[i] = y;
      this.jitter[i] = random();
    }
  }

  private step(ts: number) {
    const ctx = this.ctx;
    const colors = this.colors;
    if (!ctx || !colors) return;
    const minFrameMs = 1000 / (this.reducedMotion ? REDUCED_FPS : MAX_FPS) - 1;
    if (this.lastTs >= 0 && ts - this.lastTs < minFrameMs) return;
    if (this.startTs < 0) this.startTs = ts;
    this.lastTs = ts;
    const seconds = (ts - this.startTs) / 1000;
    const w = this.width;
    const h = this.height;

    // Reduced motion: no flight, no sweep, no blink — the finished lines.
    const morph = this.reducedMotion ? 1 : easeOut(seconds / MORPH_S);
    const cursor = this.reducedMotion
      ? { fraction: this.progress > 0 ? clamp01(this.progress) : 1, done: false }
      : cursorAt(Math.max(0, seconds - MORPH_S * 0.5), this.progress);
    const blinkOn = Math.sin(seconds * 12) > 0;

    // Sparks grow with the box so the lines stay solid in the regular size.
    const scale = Math.max(1, h / 30);

    ctx.clearRect(0, 0, w, h);
    const fade = (1 - morph) * (1 - morph);
    if (this.snapshot && fade > 0.01) {
      ctx.globalAlpha = fade;
      ctx.drawImage(this.snapshot, 0, 0, w, h);
    }
    ctx.globalCompositeOperation = colors.blend === "lighter" ? "lighter" : "source-over";
    for (let i = 0; i < SPARKS; i++) {
      const s = writingSpark(i, SPARKS, cursor.fraction, cursor.done, blinkOn, w, h, this.jitter[i], this.spark);
      // Sparks leave in a stagger so the effect dissolves instead of jumping.
      const d = clamp01(morph * 1.35 - this.jitter[i] * 0.35);
      const k = d * d * (3 - 2 * d);
      ctx.globalAlpha = lerp(0.85, s.alpha, k);
      ctx.fillStyle = rampAt(colors, s.tone);
      ctx.beginPath();
      ctx.arc(
        lerp(this.startX[i], s.x, k),
        lerp(this.startY[i], s.y, k),
        lerp(0.9, s.radius * scale, k),
        0,
        Math.PI * 2,
      );
      ctx.fill();
    }
    ctx.globalAlpha = 1;
    ctx.globalCompositeOperation = "source-over";
  }

  dispose() {
    this.running = false;
    cancelAnimationFrame(this.raf);
    this.snapshot = null;
  }
}
