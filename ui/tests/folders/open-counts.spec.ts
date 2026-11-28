// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

// The sidebar's project count on the sample data: open tasks only. Closing
// a task from its panel lowers it, reopening raises it, and its accessible
// name and tooltip say "open tasks".

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null) => (text ?? "").replace(/[⁨⁩]/g, "");

function projectTree(page: Page): Locator {
  return page.getByRole("treegrid", { name: "Projects" }).or(page.getByRole("tree", { name: "Projects" }));
}

function lane(page: Page, name: string): Locator {
  return page.locator("section[data-lane]").filter({ has: page.getByRole("heading", { name, exact: true }) });
}

async function setStatus(page: Page, name: string) {
  await page.getByRole("complementary", { name: /^Task / }).getByRole("button", { name: /^Status:/ }).click();
  await page.getByRole("menuitemradio", { name }).click();
}

test("closing a task lowers its project's count and reopening raises it", async ({ page }) => {
  await page.goto("/");
  const row = projectTree(page).getByRole("row", { name: "Personal" });
  const count = row.locator('[data-count="project"]');
  await expect(count).toHaveText(/^\d+$/);
  const before = Number(await count.innerText());
  expect(plain(await count.getAttribute("aria-label"))).toBe(`${String(before)} open tasks`);
  expect(plain(await count.getAttribute("title"))).toBe(`${String(before)} open tasks`);

  await row.click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  await lane(page, "To do").getByRole("button", { name: /Add task to/ }).click();
  await page.keyboard.type("Count me");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  const card = page.locator("section[data-lane] [data-card]", { hasText: "Count me" });
  await expect(card).toBeVisible();
  await expect(count).toHaveText(String(before + 1));

  await card.dblclick();
  await setStatus(page, "Done");
  await expect(count).toHaveText(String(before));
  await expect.poll(async () => plain(await count.getAttribute("aria-label"))).toBe(`${String(before)} open tasks`);

  await setStatus(page, "To do");
  await expect(count).toHaveText(String(before + 1));
});
