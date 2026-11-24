// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

const token = process.env.GK_BRIDGE_TOKEN ?? "";
const PAGE = `/?bridge=${process.env.GK_BRIDGE_PORT ?? "5321"}&token=${token}`;

function lane(page: Page, name: string): Locator {
  return page.locator("section[data-lane]").filter({ has: page.getByRole("heading", { name, exact: true }) });
}

/** The text of a Markdown editor, line by line. */
async function editorText(editor: Locator): Promise<string> {
  return editor.evaluate((element) => [...element.querySelectorAll(".cm-line")].map((line) => line.textContent).join("\n"));
}

/** A little longer than a draft waits for the typing to pause. */
const SAVED = 1500;

test("drafts on the engine: unsent text survives a reload, and is gone once saved or sent", async ({ page }) => {
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "New project" }).click();
  const dialog = page.getByRole("dialog", { name: "New project" });
  await dialog.getByRole("textbox", { name: "Name" }).fill("Pantry");
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  await page.keyboard.type("Stock up");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  const card = lane(page, "Backlog").locator("[data-card]", { hasText: "Stock up" });
  const key = (await card.getAttribute("data-card")) ?? "";
  const panelOf = (p: Page) => p.getByRole("complementary", { name: new RegExp(`^Task \\W*${key}\\W*$`) });
  /** Loads the page again, as a restart does, and opens the task. */
  const restart = async () => {
    await page.reload();
    await page.getByRole("row", { name: /Pantry/ }).click();
    await page.locator(`[data-card="${key}"]`).dblclick();
    return panelOf(page);
  };

  await card.dblclick();
  let panel = panelOf(page);
  // An unsent comment and an unsaved description.
  await panel.getByRole("textbox", { name: "Write a comment" }).click();
  await page.keyboard.type("Flour and rice");
  const description = panel.getByRole("region", { name: "Description" });
  await description.getByRole("button", { name: "Edit" }).click();
  await description.getByRole("textbox", { name: "Description" }).click();
  await page.keyboard.type("Shelves first");
  await page.waitForTimeout(SAVED);

  // A restart of the page: the window's memory is gone, the engine kept both.
  panel = await restart();
  await expect.poll(() => editorText(panel.getByRole("textbox", { name: "Write a comment" }))).toBe("Flour and rice");
  const kept = panel.getByRole("region", { name: "Description" });
  await expect(kept.getByText("Unsaved changes to this description are kept from earlier.")).toBeVisible();
  await kept.getByRole("button", { name: "Continue editing" }).click();
  const editor = kept.getByRole("textbox", { name: "Description" });
  await expect.poll(() => editorText(editor)).toBe("Shelves first");
  await editor.click();
  await page.keyboard.press("Control+Enter");
  await expect(kept.getByText("Shelves first")).toBeVisible();

  // Send the comment, then quote it in a reply.
  await panel.getByRole("textbox", { name: "Write a comment" }).click();
  await page.keyboard.press("Control+Enter");
  const first = panel.getByRole("article", { name: /^Comment by \W*Me/ }).first();
  await expect(first.getByText("Flour and rice")).toBeVisible();
  await first.getByRole("button", { name: "Quote in reply" }).click();
  const composer = panel.getByRole("textbox", { name: "Write a comment" });
  await expect.poll(() => editorText(composer)).toMatch(new RegExp(`^> Flour and rice\\n>\\n> — ${key}#c-[0-9a-f]{8}\\n\\n$`));
  await page.keyboard.type("And oats.");
  await page.keyboard.press("Control+Enter");
  const answer = panel.getByRole("article", { name: /^Comment by \W*Me/ }).nth(1);
  const back = answer.getByRole("link", { name: new RegExp(`^${key}#c-[0-9a-f]{8}$`) });
  await expect(back).toBeVisible();
  await back.click();
  await expect(first).toHaveAttribute("data-shown", "");
  await page.waitForTimeout(SAVED);

  // Saved and sent: nothing is offered again.
  panel = await restart();
  await expect(panel.getByRole("article", { name: /^Comment by \W*Me/ })).toHaveCount(2);
  await expect.poll(() => editorText(panel.getByRole("textbox", { name: "Write a comment" }))).toBe("");
  await expect(panel.getByText("Unsaved changes to this description are kept from earlier.")).toHaveCount(0);
  // Drafts are no history: the task's history has no trace of them.
  await panel.getByRole("tab", { name: /History/ }).click();
  await expect(panel.getByRole("list", { name: "History" })).not.toContainText(/draft/i);
});
