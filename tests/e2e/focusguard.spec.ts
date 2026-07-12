// E14a–E14c (SPEC7 §6): the focus-guard prompt in the overlay, the new
// General toggles, and the personal-dictionary table — all through the
// browser mock shim.

import { expect, test } from "@playwright/test";

test("E14a: focus prompt names both apps; Paste confirms, Esc cancels, recording UI restores", async ({
  page,
}) => {
  await page.goto("/?window=overlay");
  const pill = page.getByTestId("overlay-pill");
  await expect(pill).toBeAttached();

  // Focus moved mid-dictation: the prompt appears with both app names.
  await page.evaluate(() =>
    window.__mock!.emit("show-overlay", {
      state: "focus-changed",
      from_app: "Terminal",
      to_app: "Slack",
    }),
  );
  await expect(pill).toHaveAttribute("data-state", "focus-changed");
  await expect(page.getByTestId("focus-from")).toHaveText("Terminal");
  await expect(page.getByTestId("focus-to")).toHaveText("Slack");
  await expect(page.getByTestId("focus-copy-btn")).toBeVisible();

  // Paste -> resolve_focus_prompt { action: "paste" }.
  await page.getByTestId("focus-paste-btn").click();
  expect(
    await page.evaluate(
      () =>
        window
          .__mock!.calls()
          .find((c) => c.command === "resolve_focus_prompt")?.args?.action ?? null,
    ),
  ).toBe("paste");

  // Copy-fallback button -> resolve_focus_prompt { action: "copy" }.
  await page.getByTestId("focus-copy-btn").click();
  expect(
    await page.evaluate(() => {
      const calls = window.__mock!.calls().filter((c) => c.command === "resolve_focus_prompt");
      return calls[calls.length - 1]?.args?.action ?? null;
    }),
  ).toBe("copy");

  // Esc -> cancel (dismiss; the transcript stays in history).
  await page.keyboard.press("Escape");
  expect(
    await page.evaluate(() =>
      window.__mock!.calls().some((c) => c.command === "cancel_dictation"),
    ),
  ).toBe(true);

  // The next dictation renders the normal recording UI untouched.
  await page.evaluate(() => window.__mock!.emit("hide-overlay", null));
  await expect(pill).toHaveAttribute("data-state", "hidden");
  await page.evaluate(() => window.__mock!.emit("show-overlay", { state: "recording" }));
  await expect(pill).toHaveAttribute("data-state", "recording");
  await expect(page.getByTestId("waveform")).toBeVisible();
  await expect(page.getByTestId("focus-prompt")).not.toBeAttached();
});

test("E14b: focus-guard and sound-cues toggles show their defaults and persist", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByTestId("settings-root")).toBeVisible();

  // Documented defaults (SPEC7 FR-G6 / FR-C3): guard on, cues off.
  const guard = page.getByTestId("focus-guard");
  const cues = page.getByTestId("sound-cues");
  await expect(guard).toBeChecked();
  await expect(cues).not.toBeChecked();

  await guard.uncheck();
  await cues.check();
  await expect(guard).not.toBeChecked();
  await expect(cues).toBeChecked();

  // Survives a reload (mock persists set_settings within the tab).
  await page.reload();
  await expect(page.getByTestId("settings-root")).toBeVisible();
  await expect(page.getByTestId("focus-guard")).not.toBeChecked();
  await expect(page.getByTestId("sound-cues")).toBeChecked();
});

test("E14c: dictionary rows add, persist across reload, and delete", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("settings-root")).toBeVisible();
  await page.getByTestId("nav-cleanup").click();
  await expect(page.getByTestId("dictionary-table")).toBeAttached();

  // Add two rows.
  await page.getByTestId("dict-new-from").fill("acme corp");
  await page.getByTestId("dict-new-to").fill("AcmeCorp");
  await page.getByTestId("dict-add").click();
  await expect(page.getByTestId("dict-from-0")).toHaveValue("acme corp");

  await page.getByTestId("dict-new-from").fill("jorge pereira");
  await page.getByTestId("dict-new-to").fill("Jorge Pereira");
  await page.getByTestId("dict-add").click();
  await expect(page.getByTestId("dict-from-1")).toHaveValue("jorge pereira");

  // Both persist across a reload.
  await page.reload();
  await expect(page.getByTestId("settings-root")).toBeVisible();
  await page.getByTestId("nav-cleanup").click();
  await expect(page.getByTestId("dict-from-0")).toHaveValue("acme corp");
  await expect(page.getByTestId("dict-to-0")).toHaveValue("AcmeCorp");
  await expect(page.getByTestId("dict-from-1")).toHaveValue("jorge pereira");

  // Delete the first; the second survives.
  await page.getByTestId("dict-delete-0").click();
  await expect(page.getByTestId("dict-from-0")).toHaveValue("jorge pereira");
  await expect(page.getByTestId("dict-to-0")).toHaveValue("Jorge Pereira");
  await expect(page.locator("[data-testid=dict-row]")).toHaveCount(1);
});
