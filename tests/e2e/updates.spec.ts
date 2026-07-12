// E15a–E15b (SPEC9 §6): the Check for Updates… dialog through the browser
// mock — full happy path, up-to-date path, and honest, recoverable errors.
// The native menu/tray entries emit `check-updates` to the settings window;
// the mock's emit drives the same event.

import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("settings-root")).toBeVisible();
});

test("E15a: check → available → install progress → restart; and the up-to-date path", async ({
  page,
}) => {
  // An update is available.
  await page.evaluate(() => {
    window.__yyUpdate = {
      next: { version: "9.9.9", notes: "Big fixes." },
      progress: [],
      installed: false,
      restarted: false,
    };
    window.__mock!.emit("check-updates", null);
  });
  await expect(page.getByTestId("update-dialog")).toBeVisible();
  await expect(page.getByTestId("update-available")).toBeVisible();
  await expect(page.getByTestId("update-available")).toContainText("9.9.9");
  await expect(page.getByTestId("update-available")).toContainText("Big fixes.");

  await page.getByTestId("update-install").click();
  await expect(page.getByTestId("update-restart")).toBeVisible();
  expect(
    await page.evaluate(() => ({
      installed: window.__yyUpdate!.installed,
      maxPct: Math.max(...window.__yyUpdate!.progress),
    })),
  ).toEqual({ installed: true, maxPct: 100 });

  await page.getByTestId("update-restart").click();
  await expect.poll(() => page.evaluate(() => window.__yyUpdate!.restarted)).toBe(true);

  // Dismiss, then the up-to-date path shows the current version.
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("update-dialog")).not.toBeAttached();
  await page.evaluate(() => {
    window.__yyUpdate!.next = null;
    window.__mock!.emit("check-updates", null);
  });
  await expect(page.getByTestId("update-none")).toBeVisible();
  await expect(page.getByTestId("update-none")).toContainText("0.1.0");
});

test("E15b: errors are honest and recoverable — dialog dismissable, second check succeeds", async ({
  page,
}) => {
  await page.evaluate(() => {
    window.__yyUpdate = {
      next: { error: "manifest signature verification failed" },
      progress: [],
      installed: false,
      restarted: false,
    };
    window.__mock!.emit("check-updates", null);
  });
  await expect(page.getByTestId("update-error")).toBeVisible();
  await expect(page.getByTestId("update-error")).toContainText(
    "manifest signature verification failed",
  );

  // Dismiss; settings still fully functional.
  await page.getByTestId("update-error").getByRole("button", { name: "Close" }).click();
  await expect(page.getByTestId("update-dialog")).not.toBeAttached();
  await page.getByTestId("nav-cleanup").click();
  await expect(page.getByTestId("section-cleanup")).toBeVisible();

  // A second check succeeds — state fully resets.
  await page.evaluate(() => {
    window.__yyUpdate!.next = { version: "9.9.9", notes: "" };
    window.__mock!.emit("check-updates", null);
  });
  await expect(page.getByTestId("update-available")).toBeVisible();
});
