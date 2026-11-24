// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

function lane(page: Page, name: string): Locator {
  return page.locator("section[data-lane]").filter({ has: page.getByRole("heading", { name, exact: true }) });
}

/** The text in the comment composer. */
async function composerText(composer: Locator): Promise<string> {
  return composer.evaluate((element) => {
    const lines = [...element.querySelectorAll(".cm-line")].map((line) => line.textContent);
    return lines.join("\n");
  });
}

test("quote-reply: a selection of a comment is quoted with a link back, which opens the comment", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).click();
  await lane(page, "To do").getByRole("button", { name: /Add task to/ }).click();
  await page.keyboard.type("Choose the venue");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  const card = page.locator("section[data-lane] [data-card]", { hasText: "Choose the venue" });
  const key = (await card.getAttribute("data-card")) ?? "";
  await card.dblclick();
  const panel = page.getByRole("complementary", { name: new RegExp(`^Task \\W*${key}\\W*$`) });

  // A first comment, sent with Ctrl+Enter.
  const composer = panel.getByRole("textbox", { name: "Write a comment" });
  await composer.click();
  await page.keyboard.type("The hall is free in May. The barn costs less.");
  await page.keyboard.press("Control+Enter");
  const first = panel.getByRole("article", { name: /^Comment by \W*Me/ }).first();
  await expect(first.getByText("The hall is free in May. The barn costs less.")).toBeVisible();
  await expect.poll(() => composerText(composer)).toBe("");

  // Select a sentence of it and quote it: only the selection is quoted, with the link to the comment.
  await first.getByText("The hall is free in May.", { exact: false }).evaluate((element) => {
    const text = element.firstChild;
    const range = document.createRange();
    if (text) {
      range.setStart(text, 0);
      range.setEnd(text, "The hall is free in May.".length);
    }
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);
  });
  await first.getByRole("button", { name: "Quote in reply" }).click();
  await expect(composer).toBeFocused();
  await expect.poll(() => composerText(composer)).toMatch(new RegExp(`^> The hall is free in May\\.\\n>\\n> — ${key}#c-[0-9a-f]{8}\\n\\n$`));
  const link = /#c-[0-9a-f]{8}/.exec(await composerText(composer))?.[0] ?? "";
  await page.keyboard.type("Let's take the hall.");
  await page.keyboard.press("Control+Enter");

  // The answer shows the quote with its link; the link scrolls to the quoted comment and marks it.
  const answer = panel.getByRole("article", { name: /^Comment by \W*Me/ }).nth(1);
  const back = answer.getByRole("link", { name: `${key}${link}` });
  await expect(answer.locator("blockquote")).toContainText("The hall is free in May.");
  await expect(back).toHaveAttribute("data-comment", link.slice(3));
  await back.click();
  await expect(first).toHaveAttribute("data-shown", "");
  await expect(first).toBeFocused();

  // Quoting without a selection quotes the whole comment, after the text already written.
  await composer.click();
  await page.keyboard.type("Also:");
  await page.evaluate(() => window.getSelection()?.removeAllRanges());
  await first.getByRole("button", { name: "Comment", exact: true }).click();
  await page.getByRole("menuitem", { name: "Quote in reply" }).click();
  await expect.poll(async () => plain(await composerText(composer))).toBe(`Also:\n\n> The hall is free in May. The barn costs less.\n>\n> — ${key}${link}\n\n`);
});
