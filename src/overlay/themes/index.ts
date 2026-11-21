// The built-in color catalog (owner-requested 2026-11-20, replacing the
// SPEC6 FR-A3 / SPEC16 FR-T sets): 10 themes, each defining the full
// CSS-variable contract on `.nh-theme` (the pill root and the settings
// preview both carry that class) plus the optional ramp variables. A theme
// is a COLOR only — any of them pairs with any effect. System font stacks
// only.

export interface ThemeDef {
  id: string;
  name: string;
  variant: "light" | "dark";
  css: string;
  swatch: { bg: string; primary: string; accent: string; text: string };
  /** The color ramp effects draw with, for the picker's swatch strip. */
  ramp: string[];
}

interface ThemeSpec {
  bg: string;
  border: string;
  text: string;
  recDot: string;
  /** 3–6 hues; effects sample a smooth gradient through them. */
  ramp: string[];
  font: string;
  /** Overrides the pill's default soft drop shadow. */
  shadow?: string;
}

/** `#rrggbb` → `rgba(r, g, b, a)`. */
function alpha(hex: string, a: number): string {
  const n = (i: number) => parseInt(hex.slice(i, i + 2), 16);
  return `rgba(${n(1)}, ${n(3)}, ${n(5)}, ${a})`;
}

function theme(id: string, name: string, variant: "light" | "dark", spec: ThemeSpec): ThemeDef {
  const primary = spec.ramp[0];
  const accent = spec.ramp[1];
  const vars: Record<string, string> = {
    "--nh-pill-bg": spec.bg,
    "--nh-pill-border": spec.border,
    "--nh-text": spec.text,
    "--nh-tentative-opacity": "0.55",
    "--nh-placeholder": alpha(spec.text, 0.45),
    "--nh-timer": alpha(spec.text, 0.75),
    "--nh-rec-dot": spec.recDot,
    "--nh-fx-primary": primary,
    "--nh-fx-accent": accent,
    "--nh-fx-glow": alpha(primary, 0.8),
    "--nh-fx-ramp": spec.ramp.join(", "),
    // Overlapping layers add light on dark pills and deepen on light ones.
    "--nh-fx-blend": variant === "dark" ? "lighter" : "multiply",
    "--nh-font": spec.font,
  };
  if (spec.shadow) vars["--nh-pill-shadow"] = spec.shadow;
  const css = `.nh-theme {\n${Object.entries(vars)
    .map(([k, v]) => `  ${k}: ${v};`)
    .join("\n")}\n}`;
  return {
    id,
    name,
    variant,
    css,
    swatch: { bg: spec.bg, primary, accent, text: spec.text },
    ramp: spec.ramp,
  };
}

const SANS = `-apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif`;
const ROUNDED = `ui-rounded, "SF Pro Rounded", -apple-system, "Segoe UI", system-ui, sans-serif`;
const MONO = `ui-monospace, "SF Mono", SFMono-Regular, Menlo, monospace`;
const SERIF = `"Iowan Old Style", "Palatino", Georgia, serif`;
const GENEVA = `Geneva, Verdana, "Segoe UI", sans-serif`;

export const DEFAULT_THEME = "midnight";

export const THEMES: ThemeDef[] = [
  // A modern terminal: soft syntax colors on navy, monospace.
  theme("midnight", "Midnight", "dark", {
    bg: "rgba(18, 20, 32, 0.92)",
    border: "#2f3552",
    text: "#c8d0f0",
    recDot: "#ff7d95",
    ramp: ["#7aa8ff", "#7fd6ff", "#a4d873", "#e6b673", "#b99cff", "#ff7d95"],
    font: MONO,
  }),
  // Pastel oranges on espresso.
  theme("apricot", "Apricot", "dark", {
    bg: "rgba(31, 20, 16, 0.92)",
    border: "#5a3a2a",
    text: "#ffeede",
    recDot: "#ff7a59",
    ramp: ["#ff9e6b", "#ffb98a", "#ffd3a8", "#ffe7c4", "#ff8fa0"],
    font: ROUNDED,
  }),
  // 1984 desktop: ink on paper, hard drop shadow.
  theme("one-bit", "One-Bit", "light", {
    bg: "rgba(255, 255, 255, 0.97)",
    border: "#0a0a0a",
    text: "#0a0a0a",
    recDot: "#0a0a0a",
    ramp: ["#0a0a0a", "#0a0a0a", "#5e5e5e", "#0a0a0a"],
    font: GENEVA,
    shadow: "2.5px 2.5px 0 #0a0a0a",
  }),
  // Ivory paper, clay, kraft and slate, in a serif.
  theme("clay", "Clay", "light", {
    bg: "rgba(240, 238, 230, 0.95)",
    border: "#d6d0c1",
    text: "#1f1e1d",
    recDot: "#d97757",
    ramp: ["#d97757", "#c6613f", "#d4a27f", "#3d3d3a"],
    font: SERIF,
  }),
  // System greys on dark glass with one highlight; low contrast.
  theme("graphite", "Graphite", "dark", {
    bg: "rgba(30, 30, 32, 0.8)",
    border: "rgba(255, 255, 255, 0.14)",
    text: "#f5f5f7",
    recDot: "#ff453a",
    ramp: ["#8e8e93", "#c7c7cc", "#e5e5ea", "#0a84ff"],
    font: SANS,
  }),
  // Six pastels end to end on plum.
  theme("sorbet", "Sorbet", "dark", {
    bg: "rgba(28, 26, 36, 0.9)",
    border: "#3d3850",
    text: "#f4f0fa",
    recDot: "#ffadc6",
    ramp: ["#ffadc6", "#ffc9a3", "#fff0a8", "#b5f0c0", "#a8dcff", "#cdb8ff"],
    font: ROUNDED,
  }),
  // Pastel aquas over deep teal.
  theme("lagoon", "Lagoon", "dark", {
    bg: "rgba(6, 32, 42, 0.9)",
    border: "#14495c",
    text: "#dcf7fb",
    recDot: "#ff9a9a",
    ramp: ["#4fc3d9", "#7fe7f2", "#a8f3e6", "#c9f7ff", "#8fb8ff"],
    font: SANS,
  }),
  // Rose through hot pink on blush paper.
  theme("peony", "Peony", "light", {
    bg: "rgba(255, 240, 244, 0.95)",
    border: "#f3bfcf",
    text: "#5a1830",
    recDot: "#e0245e",
    ramp: ["#d63a7a", "#ff5c93", "#ff8fb3", "#ffb8cf", "#ff9e7a"],
    font: ROUNDED,
  }),
  // Lavender, lilac and periwinkle on indigo.
  theme("wisteria", "Wisteria", "dark", {
    bg: "rgba(23, 19, 41, 0.9)",
    border: "#3a2f63",
    text: "#ece6ff",
    recDot: "#ff7ab8",
    ramp: ["#8f7bff", "#b79cff", "#d7c4ff", "#f0a8ff", "#7fa8ff"],
    font: SANS,
  }),
  // Dusty muted pastels on grey-green paper; the quietest.
  theme("fog", "Fog", "light", {
    bg: "rgba(230, 232, 227, 0.94)",
    border: "#c9cec5",
    text: "#4a5049",
    recDot: "#c98a7e",
    ramp: ["#9db5a6", "#c4a9a0", "#a9aec4", "#c9c19a", "#9fb9c2"],
    font: SANS,
  }),
];

/** Unknown ids fall back to the default — settings resilience. */
export function getBuiltinTheme(id: string): ThemeDef {
  return THEMES.find((t) => t.id === id) ?? THEMES.find((t) => t.id === DEFAULT_THEME)!;
}
