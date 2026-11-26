// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

/** The start data the app hands the page on each platform. */
const STARTS = {
  windows: { titleBar: "system", locale: "en-US", platform: "windows" },
  linux: { titleBar: "app", locale: "en-US", platform: "linux" },
} as const;

async function startAs(page: Page, platform: keyof typeof STARTS): Promise<void> {
  await page.addInitScript((start) => {
    Object.defineProperty(window, "__GLYPHKEEL_START__", { value: Object.freeze(start) });
  }, STARTS[platform]);
  await page.goto("/");
  await expect(page.getByRole("banner")).toBeVisible();
}

function fontUi(page: Page): Promise<string> {
  return page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--font-ui").trim());
}

async function openPreferencesPage(page: Page, name: string): Promise<Locator> {
  await page.getByRole("button", { name: "Main menu" }).click();
  await page.getByRole("menuitem", { name: /Preferences/ }).click();
  const dialog = page.getByRole("dialog", { name: "Preferences" });
  await dialog.getByRole("tab", { name }).click();
  return dialog;
}

/** Opens the first task of the sample project and starts editing its description. */
async function editDescription(page: Page): Promise<Locator> {
  await page.getByRole("row", { name: /Task Manager/ }).click();
  await page.locator("section[data-lane] [data-card]").first().dblclick();
  const description = page.getByRole("region", { name: "Description" });
  await description.getByRole("button", { name: "Edit" }).click();
  return description.getByRole("textbox", { name: "Description" });
}

test("on Windows: the system title bar, Segoe UI, Windows key names and wording, the web view's spelling", async ({ page }) => {
  await startAs(page, "windows");
  await expect(page.locator("html")).toHaveClass(/(^|\s)platform-windows(\s|$)/);
  expect(await fontUi(page)).toMatch(/^"Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI",/);

  // The system draws the window buttons and moves the window.
  await expect(page.getByRole("group", { name: "Window", exact: true })).toHaveCount(0);
  await expect(page.locator("[data-tauri-drag-region]")).toHaveCount(0);

  // The shortcut sheet names Ctrl, never Super or Meta.
  await page.locator("body").press("?");
  const sheet = page.getByRole("dialog", { name: "Keyboard shortcuts" });
  await expect(sheet).toBeVisible();
  await expect(sheet).toContainText("Ctrl");
  await expect(sheet).not.toContainText(/Super|Meta/);
  await page.keyboard.press("Escape");

  // Starting with the system is signing in to Windows.
  const notifications = await openPreferencesPage(page, "Notifications");
  await expect(notifications.getByText("Start when you sign in", { exact: true })).toBeVisible();
  await expect(notifications.getByText("Start when you log in")).toHaveCount(0);
  await page.keyboard.press("Escape");

  // WebView2 checks the description's spelling itself.
  const field = await editDescription(page);
  await expect(field).toHaveAttribute("spellcheck", "true");
});

test("on Linux: unchanged, the app's header bar, its fonts and wording, the app's spelling", async ({ page }) => {
  await startAs(page, "linux");
  await expect(page.locator("html")).toHaveClass(/(^|\s)platform-linux(\s|$)/);
  expect(await fontUi(page)).toMatch(/^"Adwaita Sans", "Inter", "Cantarell", "Noto Sans"/);

  await expect(page.getByRole("group", { name: "Window", exact: true })).toBeVisible();
  await expect(page.getByRole("banner")).toHaveAttribute("data-tauri-drag-region", "true");

  const notifications = await openPreferencesPage(page, "Notifications");
  await expect(notifications.getByText("Start when you log in", { exact: true })).toBeVisible();
  await page.keyboard.press("Escape");

  const field = await editDescription(page);
  await expect(field).toHaveAttribute("spellcheck", "false");
});
