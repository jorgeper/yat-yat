// E19 (SPEC13): the transcribing state carries an estimated progress fill
// driven by transcribe-progress events. Never regresses, resets per
// session, and full is reserved for completion. Since 2026-11-20 the fill
// is the Writing figure's cursor position (the shimmer bars are gone), so
// these read it from the figure canvas's data-fill / data-fill-total.

import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/?window=overlay");
  await expect(page.getByTestId("overlay-pill")).toBeAttached();
});

const fillCount = async (page: import("@playwright/test").Page) =>
  Number(await page.getByTestId("thinking-fx").getAttribute("data-fill"));

test("E19a: progress fill advances and never regresses", async ({ page }) => {
  const pill = page.getByTestId("overlay-pill");

  await page.evaluate(() => window.__mock!.emit("show-overlay", { state: "transcribing" }));
  await expect(pill).toHaveAttribute("data-state", "transcribing");
  await expect(page.getByTestId("thinking-fx")).toBeVisible();
  await expect(pill).toHaveAttribute("data-progress", "0");
  expect(await fillCount(page)).toBe(0);

  await page.evaluate(() => window.__mock!.emit("transcribe-progress", 0.25));
  await expect(pill).toHaveAttribute("data-progress", "25");
  const atQuarter = await fillCount(page);
  expect(atQuarter).toBeGreaterThan(0);

  await page.evaluate(() => window.__mock!.emit("transcribe-progress", 0.6));
  await expect(pill).toHaveAttribute("data-progress", "60");
  const atSixty = await fillCount(page);
  expect(atSixty).toBeGreaterThan(atQuarter);

  // A late lower fraction must not pull the fill backward.
  await page.evaluate(() => window.__mock!.emit("transcribe-progress", 0.3));
  await expect(pill).toHaveAttribute("data-progress", "60");
  expect(await fillCount(page)).toBe(atSixty);
});

test("E19b: completion fills all bars; sessions reset; other states ignore it", async ({
  page,
}) => {
  const pill = page.getByTestId("overlay-pill");

  await page.evaluate(() => window.__mock!.emit("show-overlay", { state: "transcribing" }));
  await page.evaluate(() => window.__mock!.emit("transcribe-progress", 1.0));
  await expect(pill).toHaveAttribute("data-progress", "100");
  const total = Number(await page.getByTestId("thinking-fx").getAttribute("data-fill-total"));
  expect(total).toBeGreaterThan(0);
  expect(await fillCount(page)).toBe(total);

  // A fresh transcribing session starts empty — no bleed-through.
  await page.evaluate(() => window.__mock!.emit("hide-overlay", null));
  await page.evaluate(() => window.__mock!.emit("show-overlay", { state: "transcribing" }));
  await expect(pill).toHaveAttribute("data-progress", "0");
  expect(await fillCount(page)).toBe(0);

  // Progress events during recording leave the waveform state untouched.
  await page.evaluate(() => window.__mock!.emit("show-overlay", { state: "recording" }));
  await page.evaluate(() => window.__mock!.emit("transcribe-progress", 0.5));
  await expect(pill).toHaveAttribute("data-state", "recording");
  await expect(page.getByTestId("waveform")).toBeVisible();
  await expect(page.getByTestId("thinking-fx")).not.toBeAttached();
  // ...and did not seed the next transcribing session either.
  await page.evaluate(() => window.__mock!.emit("show-overlay", { state: "transcribing" }));
  await expect(pill).toHaveAttribute("data-progress", "0");
  expect(await fillCount(page)).toBe(0);
});
