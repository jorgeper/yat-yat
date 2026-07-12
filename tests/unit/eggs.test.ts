// U16–U18: the easter-egg helpers (SPEC11 §5, trigger superseded by
// SPEC12 §1) — pure, purely-cosmetic logic.

import { describe, expect, it } from "vitest";
import {
  danceMatches,
  KONAMI,
  konamiProgress,
  SLEEP_AFTER_MS,
  SLEEP_LEVEL,
  SleepTracker,
} from "../../src/lib/eggs";
import { SECRET_THEME_CSS, SECRET_THEME_ID } from "../../src/overlay/secretTheme";

describe("U18: dance detection", () => {
  it("matches the word case-insensitively with any punctuation around it", () => {
    expect(danceMatches("I want to say dance right now")).toBe(1);
    expect(danceMatches("Dance!")).toBe(1);
    expect(danceMatches("okay, dance.")).toBe(1);
    expect(danceMatches("DANCE")).toBe(1);
  });

  it("counts every occurrence", () => {
    expect(danceMatches("dance dance revolution")).toBe(2);
    expect(danceMatches("dance, then dance again")).toBe(2);
    expect(danceMatches("")).toBe(0);
  });

  it("never fires inside other words", () => {
    expect(danceMatches("the dancer danced")).toBe(0);
    expect(danceMatches("an abundance of caution")).toBe(0);
    expect(danceMatches("attendance dropped")).toBe(0);
  });
});

describe("U16: konami progress", () => {
  it("the full sequence fires", () => {
    let p = 0;
    for (const key of KONAMI) p = konamiProgress(p, key);
    expect(p).toBe(KONAMI.length);
  });

  it("a wrong key resets; re-entering the first key starts fresh", () => {
    let p = 0;
    p = konamiProgress(p, "ArrowUp");
    p = konamiProgress(p, "ArrowUp");
    p = konamiProgress(p, "x");
    expect(p).toBe(0);
    // ArrowUp mid-failure counts as a fresh attempt's first key.
    p = konamiProgress(2, "ArrowUp");
    expect(p).toBe(1);
  });

  it("letters are case-insensitive", () => {
    expect(konamiProgress(8, "B")).toBe(9);
    expect(konamiProgress(9, "A")).toBe(10);
  });
});

describe("U16: the secret Yat95 theme", () => {
  const CONTRACT = [
    "--nh-pill-bg",
    "--nh-pill-border",
    "--nh-text",
    "--nh-tentative-opacity",
    "--nh-placeholder",
    "--nh-timer",
    "--nh-rec-dot",
    "--nh-fx-primary",
    "--nh-fx-accent",
    "--nh-fx-glow",
    "--nh-font",
  ];

  it("defines the full 11-variable contract", () => {
    for (const v of CONTRACT) {
      expect(SECRET_THEME_CSS, v).toContain(`${v}:`);
    }
  });

  it("stays outside the builtin namespace", () => {
    expect(SECRET_THEME_ID.startsWith("secret:")).toBe(true);
  });
});

describe("U17: SleepTracker", () => {
  function tracker(startMs = 0) {
    let now = startMs;
    const t = new SleepTracker(() => now);
    return { t, tick: (ms: number) => (now += ms) };
  }

  it("sleeps strictly after the full silent window", () => {
    const { t, tick } = tracker();
    tick(SLEEP_AFTER_MS - 1);
    expect(t.isAsleep()).toBe(false);
    tick(1);
    expect(t.isAsleep()).toBe(true);
  });

  it("any loud sample resets the window", () => {
    const { t, tick } = tracker();
    tick(SLEEP_AFTER_MS - 100);
    t.feed(SLEEP_LEVEL + 0.01);
    tick(SLEEP_AFTER_MS - 100);
    expect(t.isAsleep()).toBe(false);
    tick(100);
    expect(t.isAsleep()).toBe(true);
  });

  it("quiet samples do not reset; wake is instant via feed", () => {
    const { t, tick } = tracker();
    tick(SLEEP_AFTER_MS / 2);
    t.feed(SLEEP_LEVEL / 2); // below threshold — no reset
    tick(SLEEP_AFTER_MS / 2);
    expect(t.isAsleep()).toBe(true);
    t.feed(0.5);
    expect(t.isAsleep()).toBe(false);
  });

  it("reset() restarts the window explicitly", () => {
    const { t, tick } = tracker();
    tick(SLEEP_AFTER_MS + 5);
    expect(t.isAsleep()).toBe(true);
    t.reset();
    expect(t.isAsleep()).toBe(false);
  });
});
