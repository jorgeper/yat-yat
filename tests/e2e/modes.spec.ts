// E22a–E22b (SPEC16 §6): the cinema-mode gallery — a card drives BOTH
// settings fields in one write, selection is derived (and deselects on
// divergence), and the segmented control anchors the sections.

import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("settings-root")).toBeVisible();
  await page.getByTestId("nav-appearance").click();
  await expect(page.getByTestId("mode-gallery")).toBeVisible();
});

test("E22a: a mode card sets effect+theme, persists, and deselects on divergence", async ({
  page,
}) => {
  await page.getByTestId("mode-glyph-rain").click();
  await expect(page.getByTestId("mode-glyph-rain")).toHaveClass(/selected/);
  await expect(page.getByTestId("effect-glyph-rain")).toHaveClass(/selected/);
  await expect(page.getByTestId("theme-glyph-rain")).toHaveClass(/selected/);

  // Persists across a reload (mock persists settings per-tab).
  await page.reload();
  await page.getByTestId("nav-appearance").click();
  await expect(page.getByTestId("mode-glyph-rain")).toHaveClass(/selected/);

  // Diverge à la carte: a different effect deselects the card, theme stays.
  await page.getByTestId("effect-pulse-orb").click();
  await expect(page.getByTestId("mode-glyph-rain")).not.toHaveClass(/selected/);
  await expect(page.getByTestId("theme-glyph-rain")).toHaveClass(/selected/);
});

test("E22b: anchors scroll the sections and a card click is exactly one settings write", async ({
  page,
}) => {
  // The segmented control brings each section into view.
  await page.getByTestId("appearance-nav-themes").click();
  await expect(page.getByTestId("theme-picker")).toBeInViewport();
  await page.getByTestId("appearance-nav-effects").click();
  await expect(page.getByTestId("effect-picker")).toBeInViewport();
  await page.getByTestId("appearance-nav-modes").click();
  await expect(page.getByTestId("mode-gallery")).toBeInViewport();

  // One card click ⇒ exactly one set_settings call carrying both fields.
  const before = await page.evaluate(
    () => window.__mock!.calls().filter((c) => c.command === "set_settings").length,
  );
  await page.getByTestId("mode-royale").click();
  await expect(page.getByTestId("mode-royale")).toHaveClass(/selected/);
  const saves = await page.evaluate(() =>
    window.__mock!.calls().filter((c) => c.command === "set_settings"),
  );
  expect(saves.length).toBe(before + 1);
  const last = saves[saves.length - 1].args?.settings as {
    overlay_effect: string;
    overlay_theme: string;
  };
  expect(last.overlay_effect).toBe("neon-diner");
  expect(last.overlay_theme).toBe("royale");
});
