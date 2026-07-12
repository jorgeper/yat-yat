// E17a–E17c (SPEC11 §5; E17a's trigger superseded by SPEC12 §1–2) and
// E18a–E18b (SPEC12 §3, the Easter-eggs switch): purely cosmetic, driven
// through the browser mock. E17c/E18b use Playwright's clock API instead of
// real waits.

import { expect, test } from "@playwright/test";

const KONAMI_KEYS = [
  "ArrowUp",
  "ArrowUp",
  "ArrowDown",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "ArrowLeft",
  "ArrowRight",
  "b",
  "a",
];

test("E17a: 'dance' in the raw stream wiggles the pill and the tray; plain text never does", async ({
  page,
}) => {
  await page.goto("/?window=overlay");
  const pill = page.getByTestId("overlay-pill");
  await expect(pill).toBeAttached();
  await page.evaluate(() =>
    window.__mock!.emit("show-overlay", { state: "recording", live: true }),
  );
  await expect(pill).toHaveAttribute("data-state", "recording");

  // Plain text: no wiggle.
  await page.evaluate(() => window.__mock!.emit("stream-text", { text: "hello world" }));
  await expect(pill).not.toHaveClass(/pill-wiggle/);

  // The magic word arrives in the raw stream.
  await page.evaluate(() =>
    window.__mock!.emit("stream-text", { text: "everybody dance now" }),
  );
  await expect(pill).toHaveClass(/pill-wiggle/);
  // The animation ends and the class clears.
  await expect(pill).not.toHaveClass(/pill-wiggle/, { timeout: 5000 });

  // Exactly one wiggle_tray call reached the host alongside the pill wiggle.
  expect(
    await page.evaluate(
      () => window.__mock!.calls().filter((c) => c.command === "wiggle_tray").length,
    ),
  ).toBe(1);
});

test("E17b: Konami unlocks Yat95 (outside the 12-theme picker) and toggles back", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByTestId("settings-root")).toBeVisible();

  // The picker shows exactly 12 built-in swatches before...
  await page.getByTestId("nav-appearance").click();
  await expect(page.getByTestId("theme-picker").locator("[data-testid^=theme-]")).toHaveCount(12);

  for (const key of KONAMI_KEYS) await page.keyboard.press(key);
  await expect(page.getByTestId("egg-toast")).toBeVisible();
  await expect(page.getByTestId("egg-toast")).toContainText("Yat95");
  expect(
    await page.evaluate(() => {
      const calls = window.__mock!.calls().filter((c) => c.command === "set_settings");
      const last = calls[calls.length - 1]?.args?.settings as { overlay_theme?: string };
      return last?.overlay_theme ?? null;
    }),
  ).toBe("secret:yat95");

  // ...and still exactly 12 after — the secret never joins the list.
  await expect(page.getByTestId("theme-picker").locator("[data-testid^=theme-]")).toHaveCount(12);

  // Konami again restores the previous theme.
  for (const key of KONAMI_KEYS) await page.keyboard.press(key);
  expect(
    await page.evaluate(() => {
      const calls = window.__mock!.calls().filter((c) => c.command === "set_settings");
      const last = calls[calls.length - 1]?.args?.settings as { overlay_theme?: string };
      return last?.overlay_theme ?? null;
    }),
  ).toBe("indigo");
});

test("E17c: 20 s of silence puts the waveform to sleep; one loud sample wakes it", async ({
  page,
}) => {
  await page.clock.install();
  await page.goto("/?window=overlay");
  const pill = page.getByTestId("overlay-pill");
  await expect(pill).toBeAttached();
  await page.evaluate(() =>
    window.__mock!.emit("show-overlay", { state: "recording", live: false }),
  );
  await expect(pill).toHaveAttribute("data-state", "recording");

  // Feed nothing but silence, then fast-forward past the sleep window.
  await page.evaluate(() => window.__mock!.emit("mic-level", 0.01));
  await page.clock.fastForward(21_000);
  await expect(pill).toHaveClass(/pill-asleep/);
  await expect(page.getByTestId("sleep-zzz")).toBeVisible();

  // One loud sample wakes it instantly.
  await page.evaluate(() => window.__mock!.emit("mic-level", 0.5));
  await expect(pill).not.toHaveClass(/pill-asleep/);
  await expect(page.getByTestId("sleep-zzz")).not.toBeAttached();
});

test("E18a: the Easter-eggs switch defaults ON and, when off, silences dance and Konami", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByTestId("settings-root")).toBeVisible();

  // General shows the switch, on by default (SPEC12 §3).
  const eggs = page.getByTestId("eggs-toggle");
  await expect(eggs).toBeChecked();
  await eggs.uncheck();
  await expect(eggs).not.toBeChecked();
  expect(
    await page.evaluate(() => {
      const calls = window.__mock!.calls().filter((c) => c.command === "set_settings");
      const last = calls[calls.length - 1]?.args?.settings as { easter_eggs?: boolean };
      return last?.easter_eggs ?? null;
    }),
  ).toBe(false);

  // Konami is inert: no toast, no settings write.
  const writesBefore = await page.evaluate(
    () => window.__mock!.calls().filter((c) => c.command === "set_settings").length,
  );
  for (const key of KONAMI_KEYS) await page.keyboard.press(key);
  await expect(page.getByTestId("egg-toast")).not.toBeAttached();
  expect(
    await page.evaluate(
      () => window.__mock!.calls().filter((c) => c.command === "set_settings").length,
    ),
  ).toBe(writesBefore);

  // Same tab (the mock persists settings): the overlay ignores dance entirely.
  await page.goto("/?window=overlay");
  const pill = page.getByTestId("overlay-pill");
  await expect(pill).toBeAttached();
  await page.evaluate(() =>
    window.__mock!.emit("show-overlay", { state: "recording", live: true }),
  );
  await expect(pill).toHaveAttribute("data-state", "recording");
  await page.evaluate(() =>
    window.__mock!.emit("stream-text", { text: "everybody dance now" }),
  );
  await expect(pill).not.toHaveClass(/pill-wiggle/);
  expect(
    await page.evaluate(
      () => window.__mock!.calls().filter((c) => c.command === "wiggle_tray").length,
    ),
  ).toBe(0);
});

test("E18b: eggs off never naps; switched back on, the nap returns", async ({ page }) => {
  await page.clock.install();

  // Switch eggs off through the real UI.
  await page.goto("/");
  await expect(page.getByTestId("settings-root")).toBeVisible();
  await page.getByTestId("eggs-toggle").uncheck();

  // A silent recording sails past the sleep window wide awake.
  await page.goto("/?window=overlay");
  const pill = page.getByTestId("overlay-pill");
  await expect(pill).toBeAttached();
  await page.evaluate(() =>
    window.__mock!.emit("show-overlay", { state: "recording", live: false }),
  );
  await expect(pill).toHaveAttribute("data-state", "recording");
  await page.evaluate(() => window.__mock!.emit("mic-level", 0.01));
  await page.clock.fastForward(21_000);
  await expect(pill).not.toHaveClass(/pill-asleep/);
  await expect(page.getByTestId("sleep-zzz")).not.toBeAttached();

  // Back on in the same session: the next recording naps again.
  await page.goto("/");
  await expect(page.getByTestId("settings-root")).toBeVisible();
  await page.getByTestId("eggs-toggle").check();

  await page.goto("/?window=overlay");
  const pill2 = page.getByTestId("overlay-pill");
  await expect(pill2).toBeAttached();
  await page.evaluate(() =>
    window.__mock!.emit("show-overlay", { state: "recording", live: false }),
  );
  await expect(pill2).toHaveAttribute("data-state", "recording");
  await page.evaluate(() => window.__mock!.emit("mic-level", 0.01));
  await page.clock.fastForward(21_000);
  await expect(pill2).toHaveClass(/pill-asleep/);
  await expect(page.getByTestId("sleep-zzz")).toBeVisible();
});
