// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Page } from "@playwright/test";

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

async function openBoard(page: Page, project: RegExp) {
  await page.getByRole("row", { name: project }).first().click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
}

test("move a task with its subtasks to another project, then undo it", async ({ page }) => {
  await page.goto("/");
  await openBoard(page, /Task Manager/);
  const card = page.locator("section[data-lane] [data-card]").first();
  const key = (await card.getAttribute("data-card")) ?? "";
  expect(key).toMatch(/^TM-\d+$/);
  await card.dblclick();
  const panel = page.getByRole("complementary", { name: /^Task / });
  await panel.getByRole("button", { name: "Task menu" }).click();
  await page.getByRole("menuitem", { name: /Move to another project/ }).click();

  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: /Project/ }).first().click();
  await page.getByRole("option", { name: /PER.*·.*Personal/ }).click();
  const keys = dialog.getByTestId("move-keys");
  await expect(keys).toContainText("becomes");
  const said = plain(await keys.innerText());
  expect(said).toContain(`${key} becomes`);
  const newKey = /becomes (PER-\d+)/.exec(said)?.[1] ?? "";
  expect(newKey).not.toBe("");
  await dialog.getByRole("button", { name: "Move", exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(panel.getByText(newKey, { exact: true }).first()).toBeVisible();

  // Undo takes it back to its project and key.
  await page.getByRole("button", { name: "Undo" }).first().click();
  await expect(page.locator(`section[data-lane] [data-card="${key}"]`)).toBeVisible();
});

test("change a project's key; its tasks take the new key", async ({ page }) => {
  await page.goto("/");
  await openBoard(page, /Personal/);
  const key = (await page.locator("section[data-lane] [data-card]").first().getAttribute("data-card")) ?? "";
  expect(key).toMatch(/^PER-\d+$/);
  await page.getByRole("button", { name: /Personal.*project menu/ }).click();
  await page.getByRole("menuitem", { name: /Project settings/ }).click();
  const settings = page.getByRole("dialog");
  await settings.getByRole("button", { name: "Change…" }).click();
  await settings.getByRole("textbox", { name: "New key" }).fill("ME");
  await settings.getByRole("button", { name: "Change key" }).click();
  await expect(settings.getByTestId("project-key")).toHaveText("ME");
  await expect(settings.getByText(/Earlier key:.*PER/)).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.locator(`section[data-lane] [data-card="${key.replace("PER-", "ME-")}"]`)).toBeVisible();
});

test("move a project under another one from its menu, then back to the top level", async ({ page }) => {
  await page.goto("/");
  await openBoard(page, /Home lab/);
  const tree = page.getByRole("treegrid").or(page.getByRole("tree")).first();
  const homeLab = tree.getByRole("row", { name: /Home lab/ });
  await expect(homeLab).toHaveAttribute("aria-level", "1");

  await page.getByRole("button", { name: /Home lab.*project menu/ }).click();
  await page.getByRole("menuitem", { name: "Move to", exact: true }).click();
  // It is at the top level already.
  await expect(page.getByRole("menuitem", { name: "Top level" })).toBeDisabled();
  await page.getByRole("menuitem", { name: "Personal", exact: true }).click();
  await expect(homeLab).toHaveAttribute("aria-level", "2");
  await expect(page.getByRole("button", { name: /Home lab.*project menu/ })).toContainText("Personal");

  // From the sidebar's menu, back to the top level.
  await homeLab.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Move to", exact: true }).click();
  await page.getByRole("menuitem", { name: "Top level" }).click();
  await expect(homeLab).toHaveAttribute("aria-level", "1");
});

test("make another workspace, work in it, and switch back", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("row", { name: /Task Manager/ }).first()).toBeVisible();
  const switcher = page.getByTestId("status-workspace");
  await expect(switcher).toContainText("Sample workspace");
  await switcher.click();
  await page.getByRole("menuitem", { name: "New workspace…" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("textbox", { name: "Name" }).fill("Side project");
  await dialog.getByRole("button", { name: "Create and open" }).click();
  await expect(dialog).toBeHidden();
  await expect(switcher).toContainText("Side project");
  await expect(page.getByRole("row", { name: /Task Manager/ })).toHaveCount(0);

  // A name taken already is refused in the dialog, in words.
  await switcher.click();
  await page.getByRole("menuitem", { name: "New workspace…" }).click();
  await dialog.getByRole("textbox", { name: "Name" }).fill("sample WORKSPACE");
  await dialog.getByRole("button", { name: "Create and open" }).click();
  await expect(dialog.getByRole("alert")).toContainText("already a workspace");
  await dialog.getByRole("button", { name: "Cancel" }).click();

  // Rename the open one, then switch back.
  await switcher.click();
  await page.getByRole("menuitem", { name: "Rename this workspace…" }).click();
  await dialog.getByRole("textbox", { name: "Name" }).fill("Hobby");
  await dialog.getByRole("button", { name: "Rename", exact: true }).click();
  await expect(switcher).toContainText("Hobby");
  await switcher.click();
  await page.getByRole("menuitemradio", { name: "Sample workspace" }).click();
  await expect(switcher).toContainText("Sample workspace");
  await expect(page.getByRole("row", { name: /Task Manager/ }).first()).toBeVisible();
});
