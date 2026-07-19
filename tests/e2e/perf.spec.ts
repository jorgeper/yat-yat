// E20 (SPEC14 FR-R5/FR-R6): the hotkey→pill path sheds its IPC round-trips —
// easter_eggs rides the show-overlay payload (no get_settings per recording)
// and an unchanged theme is never re-resolved (no repeat list_user_themes).

import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/?window=overlay");
  await expect(page.getByTestId("overlay-pill")).toBeAttached();
});

const callCount = (page: import("@playwright/test").Page, command: string) =>
  page.evaluate(
    (cmd) => window.__mock!.calls().filter((c) => c.command === cmd).length,
    command,
  );

test("E20a: easter_eggs rides the show payload — no get_settings on show, and the flag works", async ({
  page,
}) => {
  const pill = page.getByTestId("overlay-pill");
  const baseline = await callCount(page, "get_settings"); // the mount-time read

  await page.evaluate(() =>
    window.__mock!.emit("show-overlay", {
      state: "recording",
      live: true,
      easter_eggs: false,
    }),
  );
  await expect(pill).toHaveAttribute("data-state", "recording");
  expect(await callCount(page, "get_settings")).toBe(baseline);

  // The payload flag governs the eggs: dance stays silent when it is false…
  await page.evaluate(() =>
    window.__mock!.emit("stream-text", { text: "everybody dance now" }),
  );
  await expect(pill).not.toHaveClass(/pill-wiggle/);
  expect(await callCount(page, "wiggle_tray")).toBe(0);

  // …and a new show with the flag true re-arms them — still zero get_settings.
  await page.evaluate(() => window.__mock!.emit("hide-overlay", null));
  await page.evaluate(() =>
    window.__mock!.emit("show-overlay", {
      state: "recording",
      live: true,
      easter_eggs: true,
    }),
  );
  await expect(pill).toHaveAttribute("data-state", "recording");
  await page.evaluate(() =>
    window.__mock!.emit("stream-text", { text: "everybody dance now" }),
  );
  await expect(pill).toHaveClass(/pill-wiggle/);
  expect(await callCount(page, "get_settings")).toBe(baseline);
});

test("E20b: an unchanged user theme resolves once across repeated shows", async ({
  page,
}) => {
  const pill = page.getByTestId("overlay-pill");

  await page.evaluate(() =>
    window.__mock!.emit("show-overlay", {
      state: "recording",
      theme: "user:midnight-ocean",
    }),
  );
  await expect(pill).toHaveAttribute("data-state", "recording");
  await expect
    .poll(() => callCount(page, "list_user_themes"))
    .toBe(1);

  // Second and third shows with the same theme: no re-fetch, no re-resolve.
  await page.evaluate(() => window.__mock!.emit("hide-overlay", null));
  await page.evaluate(() =>
    window.__mock!.emit("show-overlay", {
      state: "recording",
      theme: "user:midnight-ocean",
    }),
  );
  await expect(pill).toHaveAttribute("data-state", "recording");
  await page.evaluate(() => window.__mock!.emit("hide-overlay", null));
  await page.evaluate(() =>
    window.__mock!.emit("show-overlay", {
      state: "transcribing",
      theme: "user:midnight-ocean",
    }),
  );
  await expect(pill).toHaveAttribute("data-state", "transcribing");
  expect(await callCount(page, "list_user_themes")).toBe(1);

  // A genuinely different theme id resolves again.
  await page.evaluate(() => window.__mock!.emit("hide-overlay", null));
  await page.evaluate(() =>
    window.__mock!.emit("show-overlay", { state: "recording", theme: "indigo" }),
  );
  await page.evaluate(() => window.__mock!.emit("hide-overlay", null));
  await page.evaluate(() =>
    window.__mock!.emit("show-overlay", {
      state: "recording",
      theme: "user:midnight-ocean",
    }),
  );
  await expect
    .poll(() => callCount(page, "list_user_themes"))
    .toBe(2);
});
