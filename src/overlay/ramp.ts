// Color ramps (owner-requested 2026-11-20). A theme may define
// `--nh-fx-ramp`, a comma-separated list of 2+ colors; the engine expands it
// into a smooth RAMP_STEPS-entry gradient that effects index with rampAt().
// Themes without the variable (user themes, Yat95) get a two-stop ramp from
// their primary and accent colors. Lives outside src/overlay/effects/ on
// purpose: this is the one place color math is allowed, effects stay
// literal-free (U8).

export const RAMP_STEPS = 32;

type Rgba = [number, number, number, number];

/** Parse `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(...)` or `rgba(...)`; else null. */
export function parseColor(input: string): Rgba | null {
  const s = input.trim().toLowerCase();
  const hex = s.match(/^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/)?.[1];
  if (hex) {
    const full = hex.length === 3 ? hex.replace(/./g, "$&$&") : hex;
    const n = (i: number) => parseInt(full.slice(i, i + 2), 16);
    return [n(0), n(2), n(4), full.length === 8 ? n(6) / 255 : 1];
  }
  const fn = s.match(/^rgba?\(([^)]+)\)$/)?.[1];
  if (fn) {
    const parts = fn.split(/[\s,/]+/).filter(Boolean).map(Number);
    if ((parts.length === 3 || parts.length === 4) && parts.every((p) => Number.isFinite(p))) {
      return [parts[0], parts[1], parts[2], parts.length === 4 ? parts[3] : 1];
    }
  }
  return null;
}

/** Split a `--nh-fx-ramp` value on top-level commas (rgb() commas survive). */
export function splitRamp(value: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let current = "";
  for (const ch of value) {
    if (ch === "(") depth++;
    if (ch === ")") depth = Math.max(0, depth - 1);
    if (ch === "," && depth === 0) {
      if (current.trim()) out.push(current.trim());
      current = "";
    } else {
      current += ch;
    }
  }
  if (current.trim()) out.push(current.trim());
  return out;
}

/**
 * Expand color stops into `steps` evenly spaced colors. Stops that cannot be
 * parsed are never blended — the result then repeats the nearest stop, so an
 * exotic CSS color still shows up, just in bands.
 */
export function expandRamp(stops: readonly string[], steps = RAMP_STEPS): string[] {
  if (stops.length === 0) return [];
  const parsed = stops.map(parseColor);
  const blendable = stops.length > 1 && parsed.every((p) => p !== null);
  const out: string[] = [];
  for (let i = 0; i < steps; i++) {
    const t = steps === 1 ? 0 : (i / (steps - 1)) * (stops.length - 1);
    if (!blendable) {
      out.push(stops[Math.round(t)]);
      continue;
    }
    const lo = Math.min(stops.length - 2, Math.floor(t));
    const f = t - lo;
    const a = parsed[lo] as Rgba;
    const b = parsed[lo + 1] as Rgba;
    const mix = (k: number) => a[k] + (b[k] - a[k]) * f;
    const alpha = Math.round(mix(3) * 1000) / 1000;
    out.push(
      alpha >= 1
        ? `rgb(${Math.round(mix(0))}, ${Math.round(mix(1))}, ${Math.round(mix(2))})`
        : `rgba(${Math.round(mix(0))}, ${Math.round(mix(1))}, ${Math.round(mix(2))}, ${alpha})`,
    );
  }
  return out;
}
