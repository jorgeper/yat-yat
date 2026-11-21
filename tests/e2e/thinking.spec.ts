// E25a–E25b (owner-requested 2026-11-20): stopping a dictation keeps the
// same box, and the effect morphs into the Writing figure while the words
// are worked out.

import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/?window=overlay");
  await expect(page.getByTestId("overlay-pill")).toBeAttached();
});

const snap = (page: import("@playwright/test").Page) =>
  page.evaluate(
    () => (document.querySelector("[data-testid=thinking-fx]") as HTMLCanvasElement).toDataURL(),
  );

test("E25a: transcribing animates the Writing figure in the same slot, with no bar", async ({
  page,
}) => {
  const pill = page.getByTestId("overlay-pill");
  await page.evaluate(() =>
    window.__mock!.emit("show-overlay", { state: "recording", effect: "halftone", theme: "sorbet" }),
  );
  await expect(page.getByTestId("waveform")).toBeVisible();
  const recordingCanvas = await page.getByTestId("waveform").boundingBox();

  await page.evaluate(() => window.__mock!.emit("show-overlay", { state: "transcribing" }));
  await expect(pill).toHaveAttribute("data-state", "transcribing");
  await expect(pill).toHaveAttribute("data-effect", "halftone");

  // The figure canvas sits in the same slot the effect had...
  const thinking = page.getByTestId("thinking-fx");
  await expect(thinking).toBeVisible();
  const thinkingCanvas = await thinking.boundingBox();
  expect(Math.abs(thinkingCanvas!.x - recordingCanvas!.x)).toBeLessThanOrEqual(1);
  expect(Math.abs(thinkingCanvas!.width - recordingCanvas!.width)).toBeLessThanOrEqual(1);

  // ...and it animates with no microphone input at all.
  const first = await snap(page);
  expect(first.length).toBeGreaterThan(200);
  await expect.poll(() => snap(page), { timeout: 4000 }).not.toBe(first);

  // No progress bar: the figure's cursor carries the progress.
  await expect(page.getByTestId("thinking-wave")).toHaveCount(0);
  await page.evaluate(() => window.__mock!.emit("transcribe-progress", 0.5));
  await expect
    .poll(async () => Number(await thinking.getAttribute("data-fill")))
    .toBeGreaterThan(0);
});

test("E25b: a live recording keeps its taller box and its words while transcribing", async ({
  page,
}) => {
  const pill = page.getByTestId("overlay-pill");
  await page.evaluate(() => {
    window.__mock!.emit("show-overlay", { state: "recording", live: true });
    window.__mock!.emit("stream-text", { text: "ship it on tuesday" });
  });
  await expect(pill).toHaveAttribute("data-live", "true");
  await expect(page.getByTestId("live-text")).toContainText("ship it on tuesday");

  // Rust replays the recording's live flag with the transcribing show.
  await page.evaluate(() =>
    window.__mock!.emit("show-overlay", { state: "transcribing", live: true }),
  );
  await expect(pill).toHaveAttribute("data-state", "transcribing");
  await expect(pill).toHaveAttribute("data-live", "true");
  await expect(page.getByTestId("thinking-text")).toContainText("ship it on tuesday");
  await expect(page.getByTestId("thinking-fx")).toBeVisible();

  // Without the flag (a non-live recording) the box is the plain one.
  await page.evaluate(() => window.__mock!.emit("show-overlay", { state: "transcribing" }));
  await expect(pill).toHaveAttribute("data-live", "false");
  await expect(page.getByTestId("thinking-text")).not.toBeAttached();
});
