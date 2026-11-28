// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

// Swimlanes on the real engine: a card dropped into another lane and column
// gets both the status and the lane's priority in one change, which one
// Ctrl+Z takes back; a collapsed lane (the person's own setting, not the
// layout's) stays collapsed after a reload.

const token = process.env.GK_BRIDGE_TOKEN ?? "";
const PAGE = `/?bridge=${process.env.GK_BRIDGE_PORT ?? ""}&token=${token}`;

test.use({ viewport: { width: 1280, height: 800 } });

/** Where Fluent may put an isolation mark, in a pattern. */
const I = "[⁨⁩]*";

const swimlane = (page: Page, name: string): Locator => page.getByRole("region", { name: new RegExp(`^Lane ${I}${name}${I}, ${I}\\d+${I} tasks?$`) });

async function cellOf(card: Locator): Promise<string> {
  return card.evaluate((element) => element.closest("[data-lane]")?.getAttribute("aria-label")?.replace(/[⁨⁩]/g, "") ?? "");
}

test("a drop into another lane and column changes the status and the priority in one change, and a collapsed lane stays so after a reload", async ({ page }) => {
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).not.toHaveText("Workspace unavailable");
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "New project" }).click();
  const dialog = page.getByRole("dialog", { name: "New project" });
  await dialog.getByRole("textbox", { name: "Name" }).fill("Lanes");
  await dialog.getByRole("textbox", { name: "Key" }).fill("SW");
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  /** Loads the page again, as a restart does, and opens the project's board. */
  const reopen = async () => {
    await page.reload();
    await page.getByRole("row", { name: /Lanes/ }).click();
    await expect(page.locator("section[data-swimlane]").first()).toBeVisible();
  };

  // Two tasks in the first column; the first one high.
  for (const title of ["Plan", "Build"]) {
    await page.keyboard.type(title);
    await page.keyboard.press("Enter");
  }
  await page.keyboard.press("Escape");
  const plan = page.locator("[data-lane] [data-card]", { hasText: "Plan" });
  const build = page.locator("[data-lane] [data-card]", { hasText: "Build" });
  await expect(build).toBeVisible();
  await plan.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Priority" }).click();
  await page.getByRole("menuitemradio", { name: "High" }).click();

  await page.getByRole("button", { name: /^Swimlanes: / }).click();
  await page.getByRole("menuitemradio", { name: "Priority", exact: true }).click();
  await expect(swimlane(page, "High").locator("[data-card]", { hasText: "Plan" })).toBeVisible();
  await expect(swimlane(page, "No priority").locator("[data-card]", { hasText: "Build" })).toBeVisible();
  const home = await cellOf(build);
  expect(home).toBe("No priority, Backlog: 1 task");

  // Build into the high lane's To do.
  const target = swimlane(page, "High").getByRole("group", { name: new RegExp(`^${I}High${I}, ${I}To do${I}: `) });
  const from = await build.boundingBox();
  const to = await target.boundingBox();
  if (!from || !to) {
    throw new Error("the card or the target cell isn't on screen");
  }
  await page.mouse.move(from.x + from.width / 2, from.y + 12);
  await page.mouse.down();
  await page.mouse.move(from.x + from.width / 2 + 10, from.y + 20, { steps: 4 });
  await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2, { steps: 20 });
  await page.mouse.up();
  await expect.poll(() => cellOf(build)).toBe("High, To do: 1 task");

  // The engine agrees after a reload: both fields changed.
  await reopen();
  await expect(swimlane(page, "High").locator("[data-card]", { hasText: "Build" })).toBeVisible();
  await expect.poll(() => cellOf(build)).toBe("High, To do: 1 task");

  // One Ctrl+Z: back in its lane and column.
  await page.locator("body").click({ position: { x: 1, y: 1 } });
  await page.keyboard.press("Control+z");
  await expect.poll(() => cellOf(build)).toBe(home);

  // A collapsed lane stays collapsed after a reload.
  await swimlane(page, "High").getByRole("button", { name: new RegExp(`^Collapse lane ${I}High`) }).click();
  await expect(swimlane(page, "High").getByRole("button", { name: new RegExp(`^Expand lane ${I}High`) })).toBeVisible();
  await reopen();
  const expand = swimlane(page, "High").getByRole("button", { name: new RegExp(`^Expand lane ${I}High${I}, ${I}1${I} task$`) });
  await expect(expand).toHaveAttribute("aria-expanded", "false");
  await expect(swimlane(page, "High").locator("[data-card]")).toHaveCount(0);
});
