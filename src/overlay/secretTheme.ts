// The hidden 13th theme (SPEC11 §5.2): Yat95 — a lovingly beveled tribute.
// Lives OUTSIDE the built-in registry so the Appearance picker and U9's
// exactly-12 contract never see it; only the Konami code reaches it.

export const SECRET_THEME_ID = "secret:yat95";

export const SECRET_THEME_CSS = `.nh-theme {
  --nh-pill-bg: #c0c0c0;
  --nh-pill-border: #808080;
  --nh-text: #000080;
  --nh-tentative-opacity: 0.55;
  --nh-placeholder: #606060;
  --nh-timer: #000080;
  --nh-rec-dot: #ff0000;
  --nh-fx-primary: #000080;
  --nh-fx-accent: #008080;
  --nh-fx-glow: rgba(0, 0, 128, 0.4);
  --nh-font: Tahoma, "MS Sans Serif", Geneva, sans-serif;
}
.pill.nh-theme {
  border-radius: 2px;
  backdrop-filter: none;
  box-shadow: inset -2px -2px 0 #808080, inset 2px 2px 0 #ffffff,
    inset -4px -4px 0 #a0a0a0, inset 4px 4px 0 #dfdfdf;
}
`;

export function secretThemeCss(themeId: string): string | null {
  return themeId === SECRET_THEME_ID ? SECRET_THEME_CSS : null;
}
