// E24a–E24b (owner-requested 2026-11-20): the box-size switch, and color
// and effect as two independent choices.

import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("settings-root")).toBeVisible();
  await page.getByTestId("nav-appearance").click();
});

const lastSaved = (page: import("@playwright/test").Page) =>
  page.evaluate(() => {
    const saves = window.__mock!.calls().filter((c) => c.command === "set_settings");
    return (saves[saves.length - 1]?.args?.settings ?? null) as {
      overlay_size?: string;
      overlay_effect?: string;
      overlay_theme?: string;
    } | null;
  });

test("E24a: the size switch resizes the preview, saves, and survives a reload", async ({
  page,
}) => {
  const pill = page.getByTestId("preview-pill");
  await expect(page.getByTestId("size-regular")).toHaveClass(/selected/);
  const regular = await pill.boundingBox();
  expect(Math.round(regular!.width)).toBe(352);
  expect(Math.round(regular!.height)).toBe(62);
  expect(await pill.evaluate((el) => getComputedStyle(el).borderTopLeftRadius)).toBe("17px");

  await page.getByTestId("size-compact").click();
  await expect(page.getByTestId("size-compact")).toHaveClass(/selected/);
  await expect(page.getByTestId("size-regular")).not.toHaveClass(/selected/);
  await expect.poll(async () => Math.round((await pill.boundingBox())!.width)).toBe(280);
  expect(Math.round((await pill.boundingBox())!.height)).toBe(48);
  expect((await lastSaved(page))?.overlay_size).toBe("compact");

  // Gentler corners than the old capsule, scaled down with the box.
  expect(await pill.evaluate((el) => getComputedStyle(el).borderTopLeftRadius)).toBe("14px");

  await page.reload();
  await page.getByTestId("nav-appearance").click();
  await expect(page.getByTestId("size-compact")).toHaveClass(/selected/);
});

test("E24b: color and effect are independent — changing one leaves the other", async ({
  page,
}) => {
  await expect(page.getByTestId("mode-gallery")).toHaveCount(0);

  await page.getByTestId("theme-sorbet").click();
  await expect(page.getByTestId("theme-sorbet")).toHaveClass(/selected/);
  await expect(page.getByTestId("effect-ribbon")).toHaveClass(/selected/);

  await page.getByTestId("effect-mesh").click();
  await expect(page.getByTestId("effect-mesh")).toHaveClass(/selected/);
  await expect(page.getByTestId("theme-sorbet")).toHaveClass(/selected/);
  const saved = await lastSaved(page);
  expect(saved?.overlay_theme).toBe("sorbet");
  expect(saved?.overlay_effect).toBe("mesh");

  // The preview pill carries the color's ramp for the engine to read.
  await expect
    .poll(() =>
      page
        .getByTestId("preview-pill")
        .evaluate((el) => getComputedStyle(el).getPropertyValue("--nh-fx-ramp").trim()),
    )
    .toBe("#ffadc6, #ffc9a3, #fff0a8, #b5f0c0, #a8dcff, #cdb8ff");
});
