// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

// Ticking a checkbox far down a long wiki page keeps the place: the page
// stays where it was scrolled to and the checkbox keeps the focus.

/** How far the box that scrolls `inside` is scrolled. */
function scrolled(inside: Locator): Promise<number> {
  return inside.evaluate((element) => {
    for (let box = element.parentElement; box; box = box.parentElement) {
      if (box.scrollHeight > box.clientHeight && getComputedStyle(box).overflowY !== "visible") {
        return box.scrollTop;
      }
    }
    return -1;
  });
}

async function openLongPage(page: Page): Promise<Locator> {
  await page.goto("/?wiki");
  await page.getByRole("navigation", { name: "Workspace" }).getByText("Task Manager").first().click();
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "Wiki" }).click();
  await page.getByRole("treegrid", { name: "Pages" }).getByRole("row", { name: "Query language" }).click();
  const view = page.getByRole("region", { name: /Wiki page/ });
  await view.getByRole("button", { name: /^Edit/ }).click();
  await expect(view.getByRole("textbox", { name: /Text of/ })).toBeFocused();
  await page.keyboard.press("Control+Home");
  await page.keyboard.insertText(Array.from({ length: 40 }, (_, index) => `Note ${String(index + 1)} on the query language, long enough to fill a line.\n\n`).join(""));
  await view.getByRole("button", { name: "Save" }).click();
  await expect(view.locator("article").getByText("Note 40 on the query language")).toBeVisible();
  return view;
}

test("a checkbox far down a wiki page ticks without moving the page or the focus", async ({ page }) => {
  const view = await openLongPage(page);
  const box = view.locator("article").getByRole("checkbox", { name: /Formal grammar/ });
  await box.scrollIntoViewIfNeeded();
  const top = await scrolled(box);
  expect(top).toBeGreaterThan(200);
  const stays = async (checked: boolean) => {
    if (checked) {
      await expect(box).toBeChecked();
    } else {
      await expect(box).not.toBeChecked();
    }
    await expect(box).not.toHaveAttribute("aria-busy");
    await page.waitForTimeout(300);
    expect(Math.abs((await scrolled(box)) - top)).toBeLessThanOrEqual(2);
    await expect(box).toBeFocused();
  };
  await box.click();
  await stays(true);
  await box.click();
  await stays(false);
  await page.keyboard.press("Space");
  await stays(true);
  await page.keyboard.press("Space");
  await stays(false);
});
