// U6: local-agreement stabilizer (SPEC3 FR-L4).

import { describe, expect, it } from "vitest";
import { EMPTY_LIVE, stabilize, type LiveText } from "../../src/lib/liveText";

function run(passes: string[]): LiveText {
  return passes.reduce(stabilize, EMPTY_LIVE);
}

describe("U6: live-text stabilizer", () => {
  it("first pass is all tentative", () => {
    const t = stabilize(EMPTY_LIVE, "hello world");
    expect(t.stable).toBe("");
    expect(t.tentative).toBe("hello world");
  });

  it("agreement promotes words to stable as text grows", () => {
    const t = run(["hello", "hello world", "hello world again"]);
    expect(t.stable).toBe("hello world");
    expect(t.tentative).toBe("again");
  });

  it("disagreeing tail is replaced, agreed prefix stays", () => {
    const t = run(["hello ward", "hello world"]);
    expect(t.stable).toBe("hello");
    expect(t.tentative).toBe("world");
  });

  it("stable never shrinks even when a later pass returns less text", () => {
    const grown = run(["hello world", "hello world how are", "hello world how are you"]);
    expect(grown.stable).toBe("hello world how are");
    const shrunk = stabilize(grown, "hello");
    expect(shrunk.stable).toBe("hello world how are");
  });

  it("stable never rewrites — first-seen rendering is kept", () => {
    const t1 = run(["Hello world", "Hello world again"]);
    expect(t1.stable).toBe("Hello world");
    // A later pass recapitalizes: agreement is case-insensitive, but the
    // stable rendering must not change.
    const t2 = stabilize(t1, "HELLO WORLD again more");
    expect(t2.stable.startsWith("Hello world")).toBe(true);
  });

  it("punctuation-tolerant agreement on the trailing word", () => {
    const t = run(["hello.", "hello there"]);
    expect(t.stable).toBe("hello.");
    expect(t.tentative).toBe("there");
  });

  it("empty and noise passes never regress the text", () => {
    const before = run(["hello world", "hello world friend"]);
    expect(stabilize(before, "")).toEqual(before);
    expect(stabilize(before, " . ")).toEqual(before);
    expect(stabilize(before, "...")).toEqual(before);
  });

  it("reset is just starting from EMPTY_LIVE again", () => {
    const t = stabilize(EMPTY_LIVE, "fresh start");
    expect(t.stable).toBe("");
    expect(t.tentative).toBe("fresh start");
  });
});

// U25: a pass that re-words already-stable text shifts every later word by
// a position or two; the tentative tail must still continue from where the
// stable text ends — no repeated word, no swallowed word.
describe("U25: live-text stabilizer realigns after re-worded stable text", () => {
  const feed = (passes: string[]) => passes.reduce(stabilize, EMPTY_LIVE);

  it("u25 two words merging into one does not repeat the next word", () => {
    const live = feed([
      "update the third party notices since we bumped two",
      "update the third party notices since we bumped two",
      "update the third-party notices since we bumped two dependencies",
    ]);
    expect(live.stable).toBe("update the third party notices since we bumped two");
    expect(live.tentative).toBe("dependencies");
  });

  it("u25 one word splitting into two does not swallow the next word", () => {
    const live = feed([
      "write the changelog tonight and tomorrow",
      "write the changelog tonight and tomorrow",
      "write the change log tonight and tomorrow we test",
    ]);
    expect(live.stable).toBe("write the changelog tonight and tomorrow");
    expect(live.tentative).toBe("we test");
  });

  it("u25 without a confident match the positions are left alone", () => {
    const live = feed(["one two three four", "one two three four", "alpha beta gamma delta epsilon"]);
    expect(live.stable).toBe("one two three four");
    expect(live.tentative).toBe("epsilon");
  });
});
