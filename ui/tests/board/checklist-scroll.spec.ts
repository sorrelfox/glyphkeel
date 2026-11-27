// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

// Ticking a checkbox far down a long text keeps the place: the panel stays
// where it was scrolled to and the checkbox keeps the focus, in the
// description, in a comment and in the task's own window.

function lane(page: Page, name: string): Locator {
  return page.locator("section[data-lane]").filter({ has: page.getByRole("heading", { name, exact: true }) });
}

/** A text long enough to scroll, with its checklist at the end. */
function longText(what: string): string {
  return [
    ...Array.from({ length: 30 }, (_, index) => `Paragraph ${String(index + 1)} of the ${what}, long enough to take a line of its own in the panel.\n`),
    `- [ ] First ${what} check\n- [ ] Second ${what} check\n- [ ] Third ${what} check\n`,
  ].join("\n");
}

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

/** Ticks and clears `box` by click and by Space, checking each time that the place and the focus stay. */
async function tickInPlace(page: Page, box: Locator): Promise<void> {
  await box.scrollIntoViewIfNeeded();
  const top = await scrolled(box);
  expect(top).toBeGreaterThan(200);
  const stays = async (checked: boolean) => {
    if (checked) {
      await expect(box).toBeChecked();
    } else {
      await expect(box).not.toBeChecked();
    }
    // The engine's answer and the texts drawn again after it.
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
}

async function openLongTask(page: Page): Promise<{ key: string; panel: Locator }> {
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  await lane(page, "To do").getByRole("button", { name: /Add task to/ }).click();
  await page.keyboard.type("Long plan");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  const card = page.locator("section[data-lane] [data-card]", { hasText: "Long plan" });
  const key = (await card.getAttribute("data-card")) ?? "";
  await card.dblclick();
  const panel = page.getByRole("complementary", { name: new RegExp(`^Task \\W*${key}\\W*$`) });
  const description = panel.getByRole("region", { name: "Description" });
  await description.getByRole("button", { name: "Edit" }).click();
  await page.keyboard.insertText(longText("plan"));
  await page.keyboard.press("Control+Enter");
  await expect(description.getByRole("checkbox", { name: "Second plan check" })).toBeVisible();
  return { key, panel };
}

test("a checkbox in a long description ticks without moving the panel or the focus", async ({ page }) => {
  const { panel } = await openLongTask(page);
  const box = panel.getByRole("region", { name: "Description" }).getByRole("checkbox", { name: "Second plan check" });
  await tickInPlace(page, box);
  // The ticks reached the engine: the progress counts them.
  await box.click();
  await expect(panel.getByText("1/3")).toBeVisible();
});

test("a checkbox in a comment ticks without moving the panel or the focus", async ({ page }) => {
  const { panel } = await openLongTask(page);
  const composer = panel.getByRole("textbox", { name: "Write a comment" });
  await composer.click();
  await page.keyboard.insertText(longText("comment"));
  await page.keyboard.press("Control+Enter");
  const comment = panel.getByRole("article", { name: /^Comment by \W*Me/ }).first();
  const box = comment.getByRole("checkbox", { name: "Second comment check" });
  await expect(box).toBeVisible();
  await tickInPlace(page, box);
});

test("a checkbox in the task's own window ticks without moving it or the focus", async ({ page }) => {
  const { key, panel } = await openLongTask(page);
  await panel.getByRole("button", { name: "Open in its own window" }).click();
  // Without the desktop app, the task's window is this page.
  await expect(page.getByRole("group", { name: /Board of/ })).toBeHidden();
  const box = page
    .getByRole("complementary", { name: new RegExp(`^Task \\W*${key}\\W*$`) })
    .getByRole("region", { name: "Description" })
    .getByRole("checkbox", { name: "Second plan check" });
  await expect(box).toBeVisible();
  await tickInPlace(page, box);
});
