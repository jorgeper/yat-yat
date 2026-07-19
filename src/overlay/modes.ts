// SPEC16 FR-M: the cinema-mode registry — 14 named presets, each a designed
// pair of one effect and one theme. A mode is presentation-layer sugar:
// picking one writes the two existing settings fields, and selection state
// is DERIVED (a mode is selected iff both fields match its pair). There is
// no overlay_mode setting anywhere. Names and taglines are riff-based (the
// Yat95 precedent): evocative, no trademarks, no character names, no film
// titles.

export interface ModeDef {
  id: string;
  name: string;
  tagline: string;
  effect: string;
  theme: string;
}

export const MODES: ModeDef[] = [
  {
    id: "night-scanner",
    name: "Night Scanner",
    tagline: "One eye. All voice.",
    effect: "larson-bar",
    theme: "night-scanner",
  },
  {
    id: "shall-we-play",
    name: "Shall We Play",
    tagline: "Shall we play a game?",
    effect: "phosphor-terminal",
    theme: "war-room",
  },
  {
    id: "grid-rider",
    name: "Grid Rider",
    tagline: "It talks for the users.",
    effect: "light-grid",
    theme: "grid-rider",
  },
  {
    id: "time-circuits",
    name: "Time Circuits",
    tagline: "Where we're going, we don't need keyboards.",
    effect: "led-circuits",
    theme: "time-circuits",
  },
  {
    id: "motion-tracker",
    name: "Motion Tracker",
    tagline: "They mostly talk at night. Mostly.",
    effect: "tracker-sweep",
    theme: "motion-tracker",
  },
  {
    id: "liquid-metal",
    name: "Liquid Metal",
    tagline: "Poured, not typed.",
    effect: "chrome-blob",
    theme: "liquid-metal",
  },
  {
    id: "glyph-rain",
    name: "Glyph Rain",
    tagline: "There is no keyboard.",
    effect: "glyph-rain",
    theme: "glyph-rain",
  },
  {
    id: "the-valley",
    name: "The Valley",
    tagline: "Another day in the valley.",
    effect: "pixel-valley",
    theme: "the-valley",
  },
  {
    id: "arc-hud",
    name: "Arc HUD",
    tagline: "Talk to the suit.",
    effect: "holo-arcs",
    theme: "arc-hud",
  },
  {
    id: "companion",
    name: "Companion",
    tagline: "I'm here to help.",
    effect: "emote-face",
    theme: "companion",
  },
  {
    id: "bridge-66",
    name: "Bridge '66",
    tagline: "Voice: the final frontier.",
    effect: "console-lamps",
    theme: "bridge-66",
  },
  {
    id: "engage",
    name: "Engage",
    tagline: "Make it so.",
    effect: "elbow-panel",
    theme: "engage",
  },
  {
    id: "yellow-fury",
    name: "Yellow Fury",
    tagline: "Wiggle your big toe. Then talk.",
    effect: "blade-slash",
    theme: "yellow-fury",
  },
  {
    id: "royale",
    name: "Royale",
    tagline: "Say ‘what’ again.",
    effect: "neon-diner",
    theme: "royale",
  },
];

/** The mode whose pair matches exactly, else null (diverged or unknown). */
export function modeFor(effect: string, theme: string): ModeDef | null {
  return MODES.find((m) => m.effect === effect && m.theme === theme) ?? null;
}
