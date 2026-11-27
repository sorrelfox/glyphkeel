// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null) => (text ?? "").replace(/[⁨⁩]/g, "");

/** A 1×1 PNG. */
const PNG = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==", "base64");

/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

/** Chooses `item` in the board's options: a choice, or an action. */
async function options(page: Page, item: string | RegExp): Promise<void> {
  await page.getByRole("button", { name: /^Board options/ }).click();
  await page.getByRole("menuitemradio", { name: item }).or(page.getByRole("menuitem", { name: item })).click();
}

// The board's options colour the cards' edges and show covers; a rule's
// mistake stays in its dialog; the edge is named in words; Undo takes a
// change back; and the board stays clean for axe with both on.
test("cards are coloured by priority or by rules and show a chosen cover", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("navigation", { name: "Workspace" }).getByText("Task Manager").first().click();
  await expect(page.locator("section[data-lane]").first()).toBeVisible();
  await expect(page.locator("[data-card][data-edge]")).toHaveCount(0);

  // By priority: every card with a priority gets an edge, named in its title.
  await options(page, "By priority");
  const edged = page.locator("[data-card][data-edge]");
  await expect(edged.first()).toBeVisible();
  const urgent = page.locator("[data-card][data-edge=red]").first();
  await expect(urgent).toBeVisible();
  expect(plain(await urgent.locator("[title]").first().getAttribute("title"))).toBe("Colour: priority Urgent");

  // Rules: a mistake stays in the dialog, under its rule.
  await options(page, "Edit colour rules…");
  const dialog = page.getByRole("dialog", { name: "Card colour rules" });
  const first = dialog.locator("[data-rule=\"1\"]");
  await first.getByRole("textbox", { name: "Query" }).fill("priority:");
  await dialog.getByRole("button", { name: "Save rules" }).click();
  await expect(first.getByRole("alert")).toBeVisible();
  await expect(first.getByRole("textbox", { name: "Query" })).toHaveAttribute("aria-invalid", "true");
  await first.getByRole("textbox", { name: "Query" }).fill("priority:urgent");
  await first.getByRole("textbox", { name: "Name" }).fill("Fire");
  await first.getByRole("radiogroup", { name: "Colour" }).locator("[data-hue=pink]").click();
  await dialog.getByRole("button", { name: "Save rules" }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator("[data-card][data-edge=pink]").first()).toBeVisible();
  await expect(page.locator("[data-card][data-edge=red]")).toHaveCount(0);
  expect(plain(await page.locator("[data-card][data-edge=pink] [title]").first().getAttribute("title"))).toBe("Colour: rule Fire");

  // Undo goes back to colouring by priority.
  await page.getByRole("button", { name: "Undo" }).click();
  await expect(page.locator("[data-card][data-edge=red]").first()).toBeVisible();

  // A cover: attach an image to a card's task and choose it.
  const card = page.locator("section[data-lane] [data-card]").first();
  const key = (await card.getAttribute("data-card")) ?? "";
  await card.dblclick();
  const panel = page.getByRole("complementary", { name: /^Task / });
  const files = panel.getByRole("region", { name: "Files" });
  await files.locator("input[type=file]").setInputFiles({ name: "mock-up.png", mimeType: "image/png", buffer: PNG });
  await expect(files.getByText("mock-up.png")).toBeVisible();
  await files.getByRole("button", { name: /^File .*mock-up\.png/ }).click();
  await page.getByRole("menuitem", { name: "Use as card cover" }).click();
  await expect(files.getByText("Cover", { exact: true })).toBeVisible();

  await options(page, "Chosen images");
  const cover = page.locator(`[data-card="${key}"] img`);
  await expect(cover).toBeVisible();
  await expect(cover).toHaveAttribute("alt", "mock-up.png");
  await expect(cover).toHaveAttribute("loading", "lazy");

  // Covers off again: the image goes.
  await options(page, "No covers");
  await expect(cover).toHaveCount(0);
  await options(page, "Chosen images");
  await expect(cover).toBeVisible();

  const results = await new AxeBuilder({ page }).withTags(TAGS).include("section[data-lane]").analyze();
  expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`)).toEqual([]);
});
