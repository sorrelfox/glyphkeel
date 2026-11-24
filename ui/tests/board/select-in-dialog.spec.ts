// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Page } from "@playwright/test";

async function choose(page: Page, how: "mouse" | "keyboard") {
  const dialog = page.getByRole("dialog", { name: "Preferences" });
  await dialog.getByRole("tab", { name: "Agents & CLI" }).click();
  const field = dialog.getByRole("button", { name: /Client/ }).first();
  if (how === "mouse") {
    await field.click();
    await page.getByRole("option").nth(1).click();
  } else {
    await field.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("option").first()).toBeVisible();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Enter");
  }
}

// A list closed while it was still fading in used to stay on top of the
// dialog until the fade ended: Escape went to the list, the dialog stayed
// open, and the window under it took no clicks. A long fade makes every
// choice here land inside it.
for (const how of ["mouse", "keyboard"] as const) {
  test(`Escape right after choosing from a list in a dialog closes the dialog (${how})`, async ({ page }) => {
    await page.setViewportSize({ width: 760, height: 520 });
    await page.goto("/");
    await page.addStyleTag({ content: ".menuPopover { --dur: 10s !important; }" });
    await page.getByRole("button", { name: "Main menu" }).click();
    await page.getByRole("menuitem", { name: /Preferences/ }).click();
    await expect(page.getByRole("dialog", { name: "Preferences" })).toBeVisible();

    await choose(page, how);
    // The list is gone at once, and the field has the focus again.
    await expect(page.getByRole("option")).toHaveCount(0, { timeout: 1000 });
    await expect(page.getByRole("button", { name: /Client/ }).first()).toBeFocused({ timeout: 1000 });
    await page.keyboard.press("Escape");

    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(page.locator("[inert]")).toHaveCount(0);
    await page.getByRole("button", { name: "Main menu" }).click({ timeout: 2000 });
    await expect(page.getByRole("menuitem", { name: /Preferences/ })).toBeVisible();
  });
}
