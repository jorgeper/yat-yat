// U22: applyTheme memoization (SPEC14 FR-R6) — every show-overlay re-applies
// the theme, so an unchanged theme must be a no-op: no style-tag rewrite
// (a textContent write forces a style recalc) and no user-theme IPC fetch.

import { beforeEach, describe, expect, it, vi } from "vitest";

const listUserThemes = vi.fn(async () => [
  {
    id: "user:midnight",
    name: "Midnight",
    variant: "dark",
    css: ".nh-theme { --nh-fx-primary: rgba(74, 168, 255, 1); }",
    reason: null,
  },
]);

vi.mock("../../src/ipc/api", () => ({
  api: { listUserThemes: () => listUserThemes() },
}));

import { applyTheme, invalidateAppliedTheme } from "../../src/overlay/applyTheme";

const styleTag = () => document.getElementById("nh-theme-style") as HTMLStyleElement | null;

describe("U22: applyTheme memoizes the applied theme", () => {
  beforeEach(() => {
    invalidateAppliedTheme();
    styleTag()?.remove();
    listUserThemes.mockClear();
  });

  it("re-applying the same built-in theme rewrites nothing", async () => {
    await applyTheme("indigo");
    const tag = styleTag();
    expect(tag).not.toBeNull();
    expect(tag!.textContent).not.toBe("");
    // Plant a sentinel: a second apply of the same id must not touch the tag.
    tag!.textContent = "SENTINEL";
    await applyTheme("indigo");
    expect(styleTag()!.textContent).toBe("SENTINEL");
    // A different theme id writes again.
    await applyTheme("");
    expect(styleTag()!.textContent).not.toBe("SENTINEL");
  });

  it("re-applying the same user theme performs no second IPC fetch", async () => {
    await applyTheme("user:midnight");
    expect(listUserThemes).toHaveBeenCalledTimes(1);
    await applyTheme("user:midnight");
    await applyTheme("user:midnight");
    expect(listUserThemes).toHaveBeenCalledTimes(1);
    // Switching away and back re-resolves.
    await applyTheme("indigo");
    await applyTheme("user:midnight");
    expect(listUserThemes).toHaveBeenCalledTimes(2);
  });

  it("invalidateAppliedTheme forces the next apply through (reloaded user themes)", async () => {
    await applyTheme("user:midnight");
    expect(listUserThemes).toHaveBeenCalledTimes(1);
    invalidateAppliedTheme();
    await applyTheme("user:midnight");
    expect(listUserThemes).toHaveBeenCalledTimes(2);
  });

  it("a missing style tag defeats the memo (fresh document, same id)", async () => {
    await applyTheme("indigo");
    styleTag()!.remove();
    await applyTheme("indigo");
    expect(styleTag()).not.toBeNull();
  });
});
