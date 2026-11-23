// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Page } from "@playwright/test";

const THEMES = [
  { id: "light", name: "Light" },
  { id: "dark", name: "Dark" },
  { id: "slate", name: "Slate" },
  { id: "sand", name: "Sand" },
  { id: "contrast", name: "Contrast" },
] as const;

/** A token's value as the browser computes it, for the window's own theme. */
function token(page: Page, name: string, property: "background-color" | "height" = "background-color"): Promise<string> {
  return page.evaluate(
    ([tokenName, cssProperty]) => {
      const probe = document.body.appendChild(document.createElement("div"));
      probe.style.setProperty(cssProperty, `var(${tokenName})`);
      const value = getComputedStyle(probe).getPropertyValue(cssProperty);
      probe.remove();
      return value;
    },
    [name, property] as const,
  );
}

test("the gallery shows every theme and switches the window's theme, accent and density", async ({ page }) => {
  await page.goto("/dev/themes");
  const html = page.locator("html");
  await expect(page.getByRole("heading", { name: "Themes and components" })).toBeVisible();

  // All five themes side by side, each painted with its own tokens.
  const backgrounds = new Set<string>();
  for (const theme of THEMES) {
    const column = page.getByTestId(`theme-${theme.id}`);
    await expect(column).toHaveAttribute("data-theme", theme.id);
    backgrounds.add(await column.evaluate((element) => getComputedStyle(element).backgroundColor));
  }
  expect(backgrounds.size).toBe(THEMES.length);

  // Each theme switch rewrites <html>, and the window takes that theme's colours.
  const themes = page.getByRole("radiogroup", { name: "Theme" });
  for (const theme of THEMES) {
    await themes.getByRole("radio", { name: theme.name }).click();
    await expect(html).toHaveAttribute("data-theme", theme.id);
    const column = await page.getByTestId(`theme-${theme.id}`).evaluate((element) => getComputedStyle(element).backgroundColor);
    const body = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
    expect(body).toBe(column);
  }

  // Density changes the control height for the window and every column.
  const button = page.getByRole("button", { name: "Back to the app" });
  await expect(button).toHaveCSS("height", "30px");
  await page.getByRole("radiogroup", { name: "Density" }).getByRole("radio", { name: "Compact" }).click();
  await expect(html).toHaveAttribute("data-density", "compact");
  await expect(button).toHaveCSS("height", "26px");
  await expect(page.getByTestId("theme-sand")).toHaveAttribute("data-density", "compact");
  await page.getByRole("radiogroup", { name: "Density" }).getByRole("radio", { name: "Regular" }).click();
  await expect(button).toHaveCSS("height", "30px");

  // The accent follows the theme until one is chosen.
  await expect(html).toHaveAttribute("data-accent", "blue");
  await page.getByRole("radiogroup", { name: "Accent" }).getByRole("radio", { name: "Purple" }).click();
  await expect(html).toHaveAttribute("data-accent", "purple");
});

test("overlays opened through portals take the window's theme", async ({ page }) => {
  await page.goto("/dev/themes");
  await page.getByRole("radiogroup", { name: "Theme" }).getByRole("radio", { name: "Slate" }).click();
  await page.getByRole("button", { name: "Open a menu" }).click();
  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  const popover = page.getByTestId("sample-menu");
  // The popover is portalled outside the app's root, yet it is drawn with the slate tokens.
  expect(await popover.evaluate((element) => element.closest("#root") === null)).toBe(true);
  await expect(popover).toHaveCSS("background-color", await token(page, "--popover"));
  await page.keyboard.press("Escape");
  await expect(menu).toBeHidden();

  await page.getByRole("button", { name: "Open a dialog" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Delete 3 tasks" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();

  // Escape on the gallery itself goes back to the app.
  await page.keyboard.press("Escape");
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeVisible();
});
