// SPEC13 FR-O1: pure helpers for the transcribing progress fill.
// The overlay only ever moves the fill forward; full is reserved for
// actual completion (fraction >= 1), never the estimate's cap.

export function advance(prev: number, next: number): number {
  if (!Number.isFinite(next)) return prev;
  return Math.max(prev, Math.min(1, Math.max(0, next)));
}

export function filledBars(fraction: number, barCount: number): number {
  if (fraction >= 1) return barCount;
  const clamped = Math.min(1, Math.max(0, fraction));
  return Math.min(barCount - 1, Math.floor(clamped * barCount));
}
