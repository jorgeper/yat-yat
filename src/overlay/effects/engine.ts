// Drives one EffectRenderer on a DPR-aware canvas (SPEC6 FR-A1).
// rAF pauses when stop() is called (overlay hidden) and the engine passes
// reducedMotion through so renderers can calm themselves.
//
// SPEC14 FR-R1/FR-R2: the loop is frame-capped (~60 fps, 30 under reduced
// motion) — mic data arrives at ~30 Hz, so repainting at a ProMotion
// display's native 120 Hz was pure waste — and the level decay is dt-based,
// so the look is identical at any refresh rate. One mutable frame object is
// reused across ticks (renderers treat it as read-only per tick).

import type { EffectColors, EffectFrame, EffectRenderer } from "./types";
import { getEffect } from "./index";
import { clamp01 } from "./types";

const HISTORY_CAP = 96;
/// Render-rate caps (SPEC14 FR-R1). The -1 ms slack absorbs rAF timestamp
/// jitter so a 60 Hz display still renders every vsync.
const MAX_FPS = 60;
const REDUCED_FPS = 30;
/** Per-frame decay at the 60 fps reference rate (the pre-SPEC14 look). */
const DECAY_PER_FRAME_60 = 0.92;

interface MutableFrame {
  level: number;
  levels: readonly number[];
  time: number;
  dt: number;
  colors: EffectColors;
  reducedMotion: boolean;
  width: number;
  height: number;
}

export class EffectEngine {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D | null;
  private renderer: EffectRenderer | null = null;
  private raf = 0;
  private running = false;
  private startTs = -1;
  private lastRenderTs = -1;
  private level = 0;
  private levels: number[] = [];
  private colors: EffectColors = { primary: "", accent: "", glow: "" };
  private reducedMotion: boolean;
  private width = 0;
  private height = 0;
  private initialized = false;
  private frame: MutableFrame = {
    level: 0,
    levels: [],
    time: 0,
    dt: 0,
    colors: { primary: "", accent: "", glow: "" },
    reducedMotion: false,
    width: 0,
    height: 0,
  };

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d");
    this.reducedMotion =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  /** Resolve theme colors from CSS variables on (an ancestor of) the canvas. */
  refreshColors() {
    const style = getComputedStyle(this.canvas);
    this.colors = {
      primary: style.getPropertyValue("--nh-fx-primary").trim() || style.color,
      accent: style.getPropertyValue("--nh-fx-accent").trim() || style.color,
      glow: style.getPropertyValue("--nh-fx-glow").trim() || style.color,
    };
  }

  setEffect(id: string) {
    this.renderer?.dispose();
    this.renderer = getEffect(id).create();
    this.initRenderer();
  }

  private initRenderer() {
    if (!this.ctx || !this.renderer) return;
    const rect = this.canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    this.width = Math.max(1, Math.round(rect.width));
    this.height = Math.max(1, Math.round(rect.height));
    this.canvas.width = this.width * dpr;
    this.canvas.height = this.height * dpr;
    this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.renderer.init(this.ctx, this.width, this.height);
    this.initialized = true;
  }

  feed(level: number) {
    this.level = clamp01(level);
    this.levels.push(this.level);
    if (this.levels.length > HISTORY_CAP) {
      this.levels.splice(0, this.levels.length - HISTORY_CAP);
    }
  }

  /** Current (decaying) level — exposed for U20's rate-independence proof. */
  get currentLevel(): number {
    return this.level;
  }

  /**
   * One tick of the render loop with an explicit timestamp (U20 drives this
   * directly). Returns true when a frame was actually rendered — ticks
   * arriving faster than the fps cap are skipped.
   */
  stepFrame(ts: number): boolean {
    const minFrameMs = 1000 / (this.reducedMotion ? REDUCED_FPS : MAX_FPS) - 1;
    if (this.lastRenderTs >= 0 && ts - this.lastRenderTs < minFrameMs) {
      return false;
    }
    if (this.startTs < 0) this.startTs = ts;
    const dt = this.lastRenderTs < 0 ? 0 : Math.min(0.1, (ts - this.lastRenderTs) / 1000);
    this.lastRenderTs = ts;

    const frame = this.frame;
    frame.level = this.level;
    frame.levels = this.levels;
    frame.time = (ts - this.startTs) / 1000;
    frame.dt = dt;
    frame.colors = this.colors;
    frame.reducedMotion = this.reducedMotion;
    frame.width = this.width;
    frame.height = this.height;

    if (this.ctx && this.renderer) {
      this.ctx.clearRect(0, 0, this.width, this.height);
      this.renderer.render(this.ctx, frame as EffectFrame);
    }
    // Levels decay between mic events so silence visibly settles. dt-based:
    // equal decay over equal wall time at any tick rate (was *0.92/frame,
    // which decayed 4x faster on a 120 Hz display than at 30 fps).
    this.level *= Math.pow(DECAY_PER_FRAME_60, dt * MAX_FPS);
    return true;
  }

  start() {
    if (this.running) return;
    this.running = true;
    this.startTs = -1;
    this.lastRenderTs = -1;
    this.refreshColors();
    // setEffect already sized the canvas and init'd the renderer — don't
    // clear + reallocate the backing store a second time (SPEC14 FR-R6).
    if (!this.initialized) this.initRenderer();
    const tick = (ts: number) => {
      if (!this.running) return;
      this.raf = requestAnimationFrame(tick);
      this.stepFrame(ts);
    };
    this.raf = requestAnimationFrame(tick);
  }

  stop() {
    this.running = false;
    cancelAnimationFrame(this.raf);
    this.level = 0;
    this.levels = [];
  }

  dispose() {
    this.stop();
    this.renderer?.dispose();
    this.renderer = null;
    this.initialized = false;
  }
}
