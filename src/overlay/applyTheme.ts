// Resolves a theme id (built-in or `user:<id>`) to CSS and injects it into
// the document (SPEC6). One style tag per document; re-applying replaces it.

import { api } from "../ipc/api";
import { getBuiltinTheme } from "./themes";
import { secretThemeCss } from "./secretTheme";

const STYLE_ID = "nh-theme-style";

export async function resolveThemeCss(themeId: string): Promise<string> {
  // The Konami-unlocked theme resolves before everything else (SPEC11 §5.2).
  const secret = secretThemeCss(themeId);
  if (secret) return secret;
  if (themeId.startsWith("user:")) {
    try {
      const user = (await api.listUserThemes()).find(
        (t) => t.id === themeId && !t.reason && t.css,
      );
      if (user) return user.css;
    } catch (e) {
      console.error("loading user theme failed:", e);
    }
    // Deleted/broken user theme: fall back to the default built-in.
    return getBuiltinTheme("").css;
  }
  return getBuiltinTheme(themeId).css;
}

// SPEC14 FR-R6: every show-overlay re-applies the theme; when it hasn't
// changed, skip both the user-theme fetch and the style-tag rewrite (a
// textContent write forces a style recalc even with identical CSS).
let appliedThemeId: string | null = null;

/** Forget the memoized theme (user themes reloaded / tests). */
export function invalidateAppliedTheme(): void {
  appliedThemeId = null;
}

export async function applyTheme(themeId: string): Promise<void> {
  if (themeId === appliedThemeId && document.getElementById(STYLE_ID)) {
    return;
  }
  const css = await resolveThemeCss(themeId);
  let tag = document.getElementById(STYLE_ID) as HTMLStyleElement | null;
  if (!tag) {
    tag = document.createElement("style");
    tag.id = STYLE_ID;
    document.head.appendChild(tag);
  }
  tag.textContent = css;
  appliedThemeId = themeId;
}
