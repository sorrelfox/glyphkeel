// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

function lane(page: Page, name: string): Locator {
  return page.locator("section[data-lane]").filter({ has: page.getByRole("heading", { name, exact: true }) });
}

async function laneOf(card: Locator): Promise<string> {
  return card.evaluate((element) => element.closest("section[data-lane]")?.querySelector("h2")?.textContent ?? "");
}

test("create a task, drag it to another column, then move it with the keyboard", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();

  // Create: the column's "+" opens the title field; Enter adds the task.
  await lane(page, "To do").getByRole("button", { name: /Add task to/ }).click();
  await page.keyboard.type("Pack the release");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  // The card in its lane; while it is dragged, a copy follows the pointer.
  const card = page.locator("section[data-lane] [data-card]", { hasText: "Pack the release" });
  await expect(card).toBeVisible();
  expect(await laneOf(card)).toBe("To do");
  const key = (await card.getAttribute("data-card")) ?? "";
  expect(key).toMatch(/^TM-\d+$/);

  // Drag with the mouse into In progress, above its first card.
  await card.scrollIntoViewIfNeeded();
  const from = await card.boundingBox();
  const target = await lane(page, "In progress").locator("[data-card]").first().boundingBox();
  if (!from || !target) {
    throw new Error("the card or the target column isn't on screen");
  }
  await page.mouse.move(from.x + from.width / 2, from.y + 12);
  await page.mouse.down();
  await page.mouse.move(from.x + from.width / 2 + 10, from.y + 20, { steps: 4 });
  await page.mouse.move(target.x + target.width / 2, target.y + 10, { steps: 20 });
  await page.mouse.up();
  await expect.poll(() => laneOf(card)).toBe("In progress");
  await expect.poll(async () => plain(await page.getByTestId("status-note").innerText())).toBe(`${key} moved to In progress`);
  await expect(lane(page, "In progress").locator("[data-card]").first()).toHaveAttribute("data-card", key);

  // Alt and an arrow key move the selected card one column on.
  await card.focus();
  await page.keyboard.press("Alt+ArrowRight");
  await expect.poll(() => laneOf(card)).toBe("Review");
  await expect.poll(async () => plain(await page.getByTestId("status-note").innerText())).toBe(`${key} moved to Review`);

  // The drag-and-drop keys: Space picks it up, an arrow moves it, Space drops it.
  // Screen readers hear each step; the flow waits for the same words.
  const announced = (text: string) => expect.poll(async () => plain((await page.locator("[id^=DndLiveRegion]").textContent()) ?? "")).toContain(text);
  await card.focus();
  await page.keyboard.press("Space");
  await announced(`${key} is over Review.`);
  // dnd-kit listens for the arrow keys only from the task after the pickup
  // (a timer it sets while handling Space), but announces the pickup sooner.
  // A key pressed in between is lost: wait for a timer set after its own.
  await page.evaluate(() => new Promise((resolve) => setTimeout(resolve, 0)));
  await page.keyboard.press("ArrowRight");
  await announced(`${key} is over Done.`);
  await page.keyboard.press("Space");
  await expect.poll(() => laneOf(card)).toBe("Done");
  await expect(card).toHaveClass(/done/);
});
