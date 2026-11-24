// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

// A lane of more than 150 cards draws only the cards near the screen. The
// keyboard still walks through it card by card and moves cards within it and
// between such lanes, and the moved card keeps the focus.

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

function lane(page: Page, name: string): Locator {
  return page.locator("section[data-lane]").filter({ has: page.getByRole("heading", { name, exact: true }) });
}

/** The focused card: its key, its lane's name and its place in that lane. */
async function focused(page: Page): Promise<{ key: string; lane: string; index: number }> {
  return page.evaluate(() => {
    const card = document.activeElement?.closest<HTMLElement>("[data-card]");
    return {
      key: card?.dataset.card ?? "",
      lane: card?.closest("section[data-lane]")?.querySelector("h2")?.textContent ?? "",
      index: Number(card?.closest<HTMLElement>("[data-index]")?.dataset.index ?? -1),
    };
  });
}

/** Presses a key once per card, each time waiting for the focus to reach the next card, as a person's key repeat would. */
async function walk(page: Page, key: "ArrowDown" | "ArrowUp", count: number): Promise<void> {
  for (let step = 0; step < count; step += 1) {
    const before = (await focused(page)).index;
    await page.keyboard.press(key);
    await expect.poll(async () => (await focused(page)).index, { timeout: 2000 }).toBe(before + (key === "ArrowDown" ? 1 : -1));
  }
}

/** Whether the element is inside its lane's scrolled box, so a person sees it. */
async function inView(card: Locator): Promise<boolean> {
  return card.evaluate((element) => {
    const box = element.closest("[data-virtual]")?.getBoundingClientRect();
    const own = element.getBoundingClientRect();
    return box !== undefined && own.top >= box.top - 1 && own.bottom <= box.bottom + 1;
  });
}

test("the keyboard walks and moves cards through lanes of more than 150 cards", async ({ page }) => {
  await page.goto("/?tasks=900");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();

  // Two neighbouring long lanes, each drawn virtually: far fewer cards in the page than in the lane.
  const todo = lane(page, "To do");
  const doing = lane(page, "In progress");
  for (const long of [todo, doing]) {
    await expect(long.locator("[data-virtual]")).toBeVisible();
    const total = Number(plain(await long.getByTestId("lane-count").innerText()).replace(/\D/g, ""));
    expect(total).toBeGreaterThan(150);
    expect(await long.locator("[data-card]").count()).toBeLessThan(60);
  }

  // Down 170 times from the first card: the focus walks past every card not drawn at first.
  await todo.locator("[data-card]").first().focus();
  const first = await focused(page);
  expect(first).toMatchObject({ lane: "To do", index: 0 });
  await walk(page, "ArrowDown", 170);
  await expect.poll(async () => (await focused(page)).index).toBe(170);
  const walked = await focused(page);
  const card = page.locator(`[data-card="${walked.key}"]`);
  expect(await inView(card)).toBe(true);

  // Alt+Up moves it one place up in its lane; it keeps the focus.
  await page.keyboard.press("Alt+ArrowUp");
  await expect.poll(() => focused(page)).toEqual({ key: walked.key, lane: "To do", index: 169 });

  // Alt+Right moves it to the next lane at the same place, far below that lane's first screen.
  await page.keyboard.press("Alt+ArrowRight");
  await expect.poll(() => focused(page)).toEqual({ key: walked.key, lane: "In progress", index: 169 });
  await expect.poll(async () => plain(await page.getByTestId("status-note").innerText())).toBe(`${walked.key} moved to In progress`);
  expect(await inView(card)).toBe(true);

  // And back again.
  await page.keyboard.press("Alt+ArrowLeft");
  await expect.poll(() => focused(page)).toEqual({ key: walked.key, lane: "To do", index: 169 });
  expect(await inView(card)).toBe(true);

  // Up again to the top, card by card.
  await walk(page, "ArrowUp", 169);
  await expect.poll(async () => (await focused(page)).index).toBe(0);
  expect((await focused(page)).key).toBe(first.key);

  // The drag-and-drop keys move a card down a virtual lane one card per key
  // press, past the cards drawn when it was picked up.
  const announced = (text: string) => expect.poll(async () => plain((await page.locator("[id^=DndLiveRegion]").textContent()) ?? "")).toContain(text);
  await page.keyboard.press("Space");
  await announced(`${first.key} is over To do.`);
  await page.evaluate(() => new Promise((resolve) => setTimeout(resolve, 0)));
  const frame = () => page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  for (let step = 0; step < 14; step += 1) {
    await page.keyboard.press("ArrowDown");
    await frame();
  }
  for (let step = 0; step < 2; step += 1) {
    await page.keyboard.press("ArrowUp");
    await frame();
  }
  await page.keyboard.press("Space");
  await announced(`${first.key} dropped in To do.`);
  const place = () => todo.locator(`[data-index] [data-card="${first.key}"]`).evaluate((element) => Number(element.closest<HTMLElement>("[data-index]")?.dataset.index ?? -1));
  await expect.poll(place).toBe(12);
});
