// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

// The main window's smallest size: the dialog is narrowest here.
const SMALLEST_WINDOW = { width: 760, height: 520 };

const THEMES = ["Light", "Dark", "Slate", "Sand", "Contrast"] as const;
const DENSITIES = ["Regular", "Compact"] as const;
const PAGES = ["Appearance", "Time", "Reminders and background", "Workspace"] as const;

async function openPreferences(page: Page): Promise<Locator> {
  // At this size the sidebar is hidden: open them from the main menu.
  await page.getByRole("button", { name: "Main menu" }).click();
  await page.getByRole("menuitem", { name: /Preferences/ }).click();
  const dialog = page.getByRole("dialog", { name: "Preferences" });
  await expect(dialog).toBeVisible();
  return dialog;
}

/**
 * Everything drawn inside the page's groups that sticks out: a row's content
 * past its box, a box or the theme tiles past the page, or a page that
 * scrolls sideways. Empty when the layout fits.
 */
function overflows(panel: Locator): Promise<string[]> {
  return panel.evaluate((root) => {
    const found: string[] = [];
    const slack = 0.5;
    const name = (element: Element) => `${element.tagName.toLowerCase()} "${element.textContent.trim().slice(0, 40)}"`;
    const outside = (inner: DOMRect, outer: DOMRect) => inner.left < outer.left - slack || inner.right > outer.right + slack;
    if (root.scrollWidth > root.clientWidth + 1) {
      found.push(`the page scrolls sideways (${String(root.scrollWidth)} > ${String(root.clientWidth)})`);
    }
    const page = root.getBoundingClientRect();
    for (const group of root.querySelectorAll(":scope > section")) {
      for (const part of group.children) {
        if (outside(part.getBoundingClientRect(), page)) {
          found.push(`${name(part)} runs out of the page`);
        }
      }
      const box = group.lastElementChild;
      if (!box) {
        continue;
      }
      const card = box.getBoundingClientRect();
      for (const row of box.children) {
        for (const element of row.querySelectorAll("*")) {
          const rect = element.getBoundingClientRect();
          // Visually hidden inputs are one pixel and clipped; they draw nothing.
          if (rect.width <= 1 || rect.height <= 1) {
            continue;
          }
          if (outside(rect, card)) {
            found.push(`${name(element)} in ${name(row)} runs out of its box`);
          }
        }
      }
    }
    return found;
  });
}

test.use({ viewport: SMALLEST_WINDOW });

test("no preferences row runs out of its box, on any page, theme or density", async ({ page }) => {
  // Forty combinations of page, theme and density.
  test.setTimeout(120_000);
  await page.goto("/");
  const dialog = await openPreferences(page);
  for (const density of DENSITIES) {
    for (const theme of THEMES) {
      await dialog.getByRole("tab", { name: "Appearance" }).click();
      // A click on the tile, as a person would: its radio input is hidden.
      await dialog.getByRole("radiogroup", { name: "Theme" }).getByText(theme, { exact: true }).click();
      await dialog.getByRole("radiogroup", { name: "Density" }).getByRole("radio", { name: density }).click();
      await expect(page.locator("html")).toHaveAttribute("data-theme", theme.toLowerCase());
      await expect(page.locator("html")).toHaveAttribute("data-density", density.toLowerCase());
      for (const name of PAGES) {
        await dialog.getByRole("tab", { name }).click();
        const panel = dialog.getByRole("tabpanel", { name });
        // The page has its rows once the workspace's settings have arrived.
        await expect(panel.locator(":scope > section").first()).toBeVisible();
        expect(await overflows(panel), `${name}, ${theme}, ${density}`).toEqual([]);
      }
    }
  }
});

test("the theme tiles wrap instead of running out of the page", async ({ page }) => {
  await page.goto("/");
  const dialog = await openPreferences(page);
  const tiles = dialog.getByRole("radiogroup", { name: "Theme" }).getByRole("radio");
  await expect(tiles).toHaveCount(6);
  const panel = dialog.getByRole("tabpanel", { name: "Appearance" });
  const box = await panel.boundingBox();
  const first = await dialog.getByText("System", { exact: true }).boundingBox();
  const last = await dialog.getByText("Contrast", { exact: true }).boundingBox();
  if (!box || !first || !last) {
    throw new Error("the page or its tiles aren't on screen");
  }
  // At the smallest size they take two lines.
  expect(last.y).toBeGreaterThan(first.y + first.height);
  expect(await overflows(panel)).toEqual([]);
});
