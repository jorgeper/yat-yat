// E23: the Whisper language setting (Models section) — defaults to
// automatic detection and persists a chosen language.

import { expect, test } from "@playwright/test";

test("E23: the Whisper language select defaults to auto and persists a choice", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByTestId("settings-root")).toBeVisible();
  await page.getByTestId("nav-models").click();

  const select = page.getByTestId("whisper-language");
  await expect(select).toHaveValue("auto");

  await select.selectOption("en");
  const persisted = await page.evaluate(() => {
    const saves = window.__mock!.calls().filter((c) => c.command === "set_settings");
    const last = saves[saves.length - 1]?.args?.settings as
      | { whisper_language?: string }
      | undefined;
    return last?.whisper_language;
  });
  expect(persisted).toBe("en");
  await expect(select).toHaveValue("en");
});
