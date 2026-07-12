// Easter-egg helpers (SPEC11 §5) — all pure, all cosmetic-only. Nothing in
// this file may ever influence recording, transcription, cleanup, or paste.

/** How many times "dance" appears in the RAW stream text. SPEC12 §1
 * superseded SPEC11's "yat yat": Whisper never transcribes the non-word
 * "yat" (measured — it hears "that you add" / "that yet" / "you're at"),
 * so the app answers to "dance" instead. Deliberately loose: the word in
 * any sentence wiggles; it's cosmetic. */
export function danceMatches(text: string): number {
  return (text.match(/\bdance\b/gi) ?? []).length;
}

/** ↑↑↓↓←→←→BA. Feed keys; returns the new progress. A full match returns
 * KONAMI.length (caller resets). Wrong keys reset (re-entering the first
 * key starts a fresh attempt). */
export const KONAMI = [
  "ArrowUp",
  "ArrowUp",
  "ArrowDown",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "ArrowLeft",
  "ArrowRight",
  "b",
  "a",
] as const;

export function konamiProgress(progress: number, key: string): number {
  const normalized = key.length === 1 ? key.toLowerCase() : key;
  if (normalized === KONAMI[progress]) return progress + 1;
  return normalized === KONAMI[0] ? 1 : 0;
}

export const SLEEP_AFTER_MS = 20_000;
export const SLEEP_LEVEL = 0.06;

/** Tracks silence while the recording pill is up. Pure: the clock is
 * injected, so U17 (and Playwright's clock API) control time. */
export class SleepTracker {
  private lastLoud: number;

  constructor(private readonly now: () => number) {
    this.lastLoud = now();
  }

  /** Feed a mic level; loud samples wake/reset instantly. */
  feed(level: number): void {
    if (level >= SLEEP_LEVEL) this.lastLoud = this.now();
  }

  isAsleep(): boolean {
    return this.now() - this.lastLoud >= SLEEP_AFTER_MS;
  }

  reset(): void {
    this.lastLoud = this.now();
  }
}
