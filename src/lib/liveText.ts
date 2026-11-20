// Local-agreement stabilizer for live transcription (SPEC3 FR-L4, U6).
//
// Successive re-transcription passes wobble at the tail (the engine changes
// its mind as context grows). Words that two consecutive passes agree on are
// promoted to `stable`; stable text NEVER shrinks and NEVER rewrites — the
// UI renders stable solid and the still-changing tail dimmed.

export interface LiveText {
  stable: string;
  tentative: string;
}

export const EMPTY_LIVE: LiveText = { stable: "", tentative: "" };

/** Case- and punctuation-insensitive word identity for agreement checks. */
function norm(word: string): string {
  return word.toLowerCase().replace(/[^\p{L}\p{N}']/gu, "");
}

function words(text: string): string[] {
  return text.split(/\s+/).filter(Boolean);
}

function join(list: string[]): string {
  return list.join(" ");
}

/** A pass is noise if it contains no word-like content (e.g. "" or "."). */
function isNoise(list: string[]): boolean {
  return list.every((w) => norm(w) === "");
}

export function stabilize(prev: LiveText, nextRaw: string): LiveText {
  const nextWords = words(nextRaw);
  if (isNoise(nextWords)) {
    return prev; // never regress on an empty/noise pass
  }

  const stableWords = words(prev.stable);
  const prevAll = [...stableWords, ...words(prev.tentative)];

  // Longest agreeing prefix between the previous pass's full text and this one.
  let agree = 0;
  while (
    agree < prevAll.length &&
    agree < nextWords.length &&
    norm(prevAll[agree]) !== "" &&
    norm(prevAll[agree]) === norm(nextWords[agree])
  ) {
    agree++;
  }

  // Stable never shrinks; agreed words beyond it are promoted. Promoted words
  // keep their first-seen rendering (from the previous pass); the tentative
  // tail always shows the newest rendering.
  const stableCount = Math.max(stableWords.length, agree);
  const stable = join([...stableWords, ...prevAll.slice(stableWords.length, stableCount)]);
  const tentative = join(nextWords.slice(stableCount + drift(stableWords, nextWords, agree)));

  return { stable, tentative };
}

/** How far to look for the stable text's end in a pass that re-worded it. */
const MAX_DRIFT = 3;
/** Words of stable text that must reappear in the pass to trust the match. */
const ANCHOR = 3;

/**
 * A pass that re-words text already stable ("third party" → "third-party")
 * has its later words at different positions than the stable text does.
 * Stable never changes, but the tentative tail must still start right after
 * the words stable ends with — or it repeats one or swallows one. Returns
 * how far the pass's words are shifted against stable (0 = aligned, or no
 * confident match).
 */
function drift(stableWords: string[], nextWords: string[], agree: number): number {
  if (agree >= stableWords.length || stableWords.length < ANCHOR) return 0;
  const anchor = stableWords.slice(-ANCHOR).map(norm);
  if (anchor.some((w) => w === "")) return 0;
  const end = stableWords.length;
  for (const shift of [-1, 1, -2, 2, -3, 3].filter((s) => Math.abs(s) <= MAX_DRIFT)) {
    const from = end + shift - ANCHOR;
    if (from < 0 || from + ANCHOR > nextWords.length) continue;
    if (anchor.every((w, i) => norm(nextWords[from + i]) === w)) return shift;
  }
  return 0;
}
