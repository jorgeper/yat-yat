// E16a–E16b (SPEC11): the consented, itemized uninstall dialog through the
// browser mock.

import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("settings-root")).toBeVisible();
});

test("E16a: the plan is itemized with sizes; keep-data drops app data; confirm passes the flag", async ({
  page,
}) => {
  await page.getByTestId("uninstall-open").click();
  await expect(page.getByTestId("uninstall-dialog")).toBeVisible();

  // The mocked plan renders with human sizes.
  const items = page.getByTestId("uninstall-item");
  await expect(items).toHaveCount(4);
  await expect(page.getByTestId("uninstall-dialog")).toContainText("Downloaded models");
  await expect(page.getByTestId("uninstall-dialog")).toContainText("1.9 GB");
  await expect(page.getByTestId("uninstall-dialog")).toContainText("Settings & history");

  // keep-data refetches the plan and the app-data row disappears.
  await page.getByTestId("uninstall-keep-data").check();
  await expect(items).toHaveCount(3);
  await expect(page.getByTestId("uninstall-dialog")).not.toContainText("Settings & history");

  // Confirm calls uninstall_app with the flag.
  await page.getByTestId("uninstall-confirm").click();
  expect(
    await page.evaluate(
      () =>
        window.__mock!.calls().find((c) => c.command === "uninstall_app")?.args?.keepData ?? null,
    ),
  ).toBe(true);
});

test("E16b: cancel never uninstalls and settings stay fully functional", async ({ page }) => {
  await page.getByTestId("uninstall-open").click();
  await expect(page.getByTestId("uninstall-dialog")).toBeVisible();
  await page.getByTestId("uninstall-cancel").click();
  await expect(page.getByTestId("uninstall-dialog")).not.toBeAttached();

  expect(
    await page.evaluate(() => window.__mock!.calls().some((c) => c.command === "uninstall_app")),
  ).toBe(false);

  // Settings remain fully functional after dismissing.
  await page.getByTestId("nav-cleanup").click();
  await expect(page.getByTestId("section-cleanup")).toBeVisible();
});
