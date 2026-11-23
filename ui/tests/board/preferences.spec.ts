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

// ---------- choices drawn by the app, never by the platform ----------

/** A token's value on <html>, as the browser computes a background in it. */
function tokenColour(page: Page, token: string): Promise<string> {
  return page.evaluate((name) => {
    const probe = document.createElement("div");
    probe.style.background = `var(${name})`;
    document.body.append(probe);
    const colour = getComputedStyle(probe).backgroundColor;
    probe.remove();
    return colour;
  }, token);
}

const SYSTEM_COLOURS = ["rgb(0, 0, 0)", "rgb(255, 255, 255)", "rgba(0, 0, 0, 0)"];

/** Opens a select by its name, checks its popover is the theme's, and closes it. */
async function checkSelect(page: Page, scope: Locator, name: RegExp, dark: boolean) {
  await scope.getByRole("button", { name }).first().click();
  const list = page.getByRole("listbox");
  await expect(list).toBeVisible();
  const popover = list.locator("xpath=ancestor-or-self::*[contains(@class, 'pop')][1]");
  const background = await popover.evaluate((element) => getComputedStyle(element).backgroundColor);
  expect(background, `${String(name)} popover`).toBe(await tokenColour(page, "--popover"));
  if (dark) {
    expect(SYSTEM_COLOURS).not.toContain(background);
  }
  // The items are the design's, in the theme's text colour.
  const colour = await list.getByRole("option").first().evaluate((element) => getComputedStyle(element).color);
  expect(colour).toBe(await page.evaluate(() => getComputedStyle(document.body).color));
  await page.keyboard.press("Escape");
  await expect(list).toBeHidden();
}

test.describe("choices in the window's theme", () => {
  test.use({ viewport: { width: 1280, height: 860 } });

  for (const theme of ["Dark", "Light"] as const) {
    test(`the accent swatches and every select follow the ${theme} theme`, async ({ page }) => {
      await page.goto("/");
      const dialog = await openPreferences(page);
      await dialog.getByRole("radiogroup", { name: "Theme" }).getByText(theme, { exact: true }).click();
      await expect(page.locator("html")).toHaveAttribute("data-theme", theme.toLowerCase());
      const dark = theme === "Dark";

      // Accent colour: swatches, no native select, each in its own colour.
      const accents = dialog.getByRole("radiogroup", { name: "Accent colour" });
      await expect(accents.getByRole("radio")).toHaveCount(10);
      expect(await page.locator("select").count()).toBe(0);
      const teal = accents.locator("[data-accent=teal]");
      expect(await teal.evaluate((element) => getComputedStyle(element).backgroundColor)).toBe("rgb(26, 127, 143)");
      // Its name shows as the design's tooltip: --fg on --bg.
      await teal.hover();
      const tip = teal.getByText("Teal", { exact: true });
      await expect(tip).toBeVisible();
      expect(await tip.evaluate((element) => getComputedStyle(element).backgroundColor)).toBe(await tokenColour(page, "--fg"));
      await teal.click();
      await expect(page.locator("html")).toHaveAttribute("data-accent", "teal");
      await accents.getByText("From the desktop").click();
      await page.keyboard.press("Escape");
      await expect(dialog).toBeHidden();

      // Time report: the project filter.
      await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "Time report" }).click();
      await checkSelect(page, page.locator("main"), /Project/, dark);

      // A project's settings: category, colour, shape and the delete target.
      await page.getByRole("row", { name: /Task Manager/ }).first().click();
      await page.getByRole("button", { name: /Task Manager.*project menu/ }).click();
      await page.getByRole("menuitem", { name: /Project settings/ }).click();
      const settings = page.getByRole("dialog");
      await settings.getByRole("tab", { name: "Statuses" }).click();
      await checkSelect(page, settings, /Category/, dark);
      await checkSelect(page, settings, /Colour/, dark);
      await settings.getByRole("button", { name: /^Delete status/ }).first().click();
      await checkSelect(page, settings, /Status for its tasks/, dark);
      await settings.getByRole("tab", { name: "Task types" }).click();
      await checkSelect(page, settings, /Shape/, dark);
      await settings.getByRole("tab", { name: "Labels" }).click();
      await checkSelect(page, settings, /Colour/, dark);
      await page.keyboard.press("Escape");
      await expect(settings).toBeHidden();

      // A task's links: the kind of link.
      await page.locator("section[data-lane] [data-card]").first().dblclick();
      const links = page.getByRole("complementary", { name: /^Task / }).getByRole("region", { name: "Links" });
      await links.getByRole("button", { name: "Add link" }).click();
      await checkSelect(page, links, /Kind of link/, dark);

      // React Aria keeps a hidden native select for forms; none is shown.
      expect(await page.locator("select:not([aria-hidden=true] select)").count()).toBe(0);
    });
  }

  test("screenshot: the accent swatches in Slate", async ({ page }) => {
    await page.setViewportSize({ width: 1100, height: 760 });
    await page.goto("/");
    const dialog = await openPreferences(page);
    await dialog.getByRole("radiogroup", { name: "Theme" }).getByText("Slate", { exact: true }).click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "slate");
    const accents = dialog.getByRole("radiogroup", { name: "Accent colour" });
    await accents.locator("[data-accent=purple]").click();
    await accents.locator("[data-accent=teal]").hover();
    await expect(accents.getByText("Teal", { exact: true })).toBeVisible();
    await page.waitForTimeout(700);
    const out = process.env.GK_SCREENSHOT;
    if (out) {
      await page.screenshot({ path: out });
    }
  });
});
