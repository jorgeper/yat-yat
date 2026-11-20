// E24: the experimental Core ML toggle (Models section) — off by default,
// and a change is persisted.

import { expect, test } from "@playwright/test";

test("E24: the Core ML toggle defaults off and persists", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("settings-root")).toBeVisible();
  await page.getByTestId("nav-models").click();

  const toggle = page.getByTestId("parakeet-coreml");
  await expect(toggle).not.toBeChecked();

  await toggle.check();
  const persisted = await page.evaluate(() => {
    const saves = window.__mock!.calls().filter((c) => c.command === "set_settings");
    const last = saves[saves.length - 1]?.args?.settings as
      | { parakeet_coreml?: boolean }
      | undefined;
    return last?.parakeet_coreml;
  });
  expect(persisted).toBe(true);
  await expect(toggle).toBeChecked();
});
