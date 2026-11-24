// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { join } from "node:path";

import { expect, test, type Locator, type Page } from "@playwright/test";

// Labels, project icons, the time zone and the formatting toolbar, on the
// real engine. Each flow makes its own project. With GK_SCREENSHOT_DIR set,
// each saves a screenshot there.

const token = process.env.GK_BRIDGE_TOKEN ?? "";
const PAGE = `/?bridge=${process.env.GK_BRIDGE_PORT ?? "5321"}&token=${token}`;

// A fixed zone for the browser, so that switching to another always moves the clock.
test.use({ timezoneId: "Europe/Berlin", viewport: { width: 1280, height: 800 } });

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

async function shot(page: Page, name: string, clip?: Locator) {
  const dir = process.env.GK_SCREENSHOT_DIR;
  if (!dir) {
    return;
  }
  await page.waitForTimeout(300);
  const box = clip ? await clip.boundingBox() : null;
  const margin = 12;
  await page.screenshot({
    path: join(dir, name),
    ...(box ? { clip: { x: Math.max(0, box.x - margin), y: Math.max(0, box.y - margin), width: box.width + 2 * margin, height: box.height + 2 * margin } } : {}),
  });
}

/** Opens the page and makes a project, which opens on its board. */
async function newProject(page: Page, name: string, key: string): Promise<string> {
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).not.toHaveText("Workspace unavailable");
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "New project" }).click();
  const dialog = page.getByRole("dialog", { name: "New project" });
  await dialog.getByRole("textbox", { name: "Name" }).fill(name);
  await dialog.getByRole("textbox", { name: "Key" }).fill(key);
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  return key;
}

/** Adds a task from the board's first column and opens it in the panel. */
async function newTask(page: Page, title: string): Promise<Locator> {
  await page.keyboard.type(title);
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  await page.locator("section[data-lane] [data-card]", { hasText: title }).dblclick();
  const panel = page.getByRole("complementary", { name: /^Task / });
  await expect(panel.getByRole("heading", { name: title })).toBeVisible();
  return panel;
}

async function projectSettings(page: Page): Promise<Locator> {
  await page.getByRole("button", { name: /project menu/ }).click();
  await page.getByRole("menuitem", { name: /Project settings/ }).click();
  return page.getByRole("dialog");
}

test("labels added one after another get the suggested colours, from settings and from a task", async ({ page }) => {
  await newProject(page, "Allotment", "PLOT");
  const panel = await newTask(page, "Dig the beds");

  // No labels yet: the "+" opens a form that makes the first one and puts it on the task.
  await panel.getByRole("button", { name: "Change labels" }).click();
  const form = page.getByRole("dialog", { name: "New label" });
  await form.getByRole("textbox", { name: "Name" }).fill("soil");
  await form.getByRole("textbox", { name: "Name" }).press("Enter");
  await expect(panel.locator(".chip", { hasText: "soil" })).toHaveClass(/hue-red/);

  // With labels: the picker creates one by its typed name.
  await panel.getByRole("button", { name: "Change labels" }).click();
  await page.getByRole("searchbox", { name: "Find or create a label" }).fill("water");
  await page.getByRole("menuitemcheckbox", { name: /Create label/ }).click();
  await expect(panel.locator(".chip", { hasText: "water" })).toHaveClass(/hue-orange/);
  // With nothing typed, Escape closes the picker.
  await page.keyboard.press("Escape");
  await expect(page.getByRole("searchbox", { name: "Find or create a label" })).toBeHidden();
  await panel.getByRole("button", { name: "Close panel" }).click();

  // In settings: only the name and Enter, several times.
  const settings = await projectSettings(page);
  await settings.getByRole("tab", { name: "Labels" }).click();
  const name = settings.getByRole("textbox", { name: "New label" });
  for (const label of ["seeds", "tools", "harvest", "compost"]) {
    await name.fill(label);
    await name.press("Enter");
    await expect(settings.locator(".chip", { hasText: label })).toBeVisible();
  }
  const hues = await settings.locator(".chip").evaluateAll((chips) => chips.map((chip) => [chip.textContent, /hue-(\w+)/.exec(chip.getAttribute("class") ?? "")?.[1]]));
  expect(hues).toEqual([
    ["soil", "red"],
    ["water", "orange"],
    ["seeds", "yellow"],
    ["tools", "green"],
    ["harvest", "teal"],
    ["compost", "blue"],
  ]);
  await expect(settings.getByRole("button", { name: /Colour of the new label/ })).toContainText("Purple");
  await shot(page, "labels.png", settings);
});

test("a project's icon shows in its colour in the sidebar and the header", async ({ page }) => {
  const key = await newProject(page, "Launch pad", "PAD");
  const settings = await projectSettings(page);
  await settings.getByRole("radiogroup", { name: "Colour" }).locator("[data-hue=purple]").click();
  await settings.getByRole("radiogroup", { name: "Icon" }).locator("[data-icon=rocket]").click();
  await expect(settings.getByRole("radiogroup", { name: "Icon" }).getByRole("radio", { name: "Rocket" })).toBeChecked();
  const row = page.locator(`[data-key="${key}"]`);
  await expect(row.locator('[data-mark="rocket"]')).toHaveClass(/hue-purple/);
  await expect(page.locator('header [data-mark="rocket"]')).toBeVisible();
  await page.mouse.move(0, 0);
  await shot(page, "project-mark.png");
  await page.keyboard.press("Escape");
  await expect(row.locator('[data-mark="rocket"]')).toBeVisible();
});

test("times follow the chosen time zone", async ({ page }) => {
  await newProject(page, "Travel", "TRIP");
  const panel = await newTask(page, "Call the hotel");
  const composer = panel.getByRole("textbox", { name: "Write a comment" });
  await composer.click();
  await page.keyboard.type("Booked for Friday");
  await page.keyboard.press("Control+Enter");
  const comment = panel.locator("article[data-comment]", { hasText: "Booked for Friday" });
  await expect(comment).toBeVisible();
  const before = plain(await comment.locator(".faint").first().innerText());

  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /Preferences/ }).click();
  const preferences = page.getByRole("dialog", { name: "Preferences" });
  await preferences.getByRole("tab", { name: "Time" }).click();
  const zone = preferences.getByRole("combobox", { name: "Time zone" });
  await expect(zone).toHaveValue(/^System/);
  await zone.fill("Tokyo");
  const tokyo = page.getByRole("option", { name: /Asia\/Tokyo/ });
  expect(plain(await tokyo.innerText())).toContain("UTC+09:00");
  await shot(page, "timezone.png", preferences);
  await tokyo.click();
  await expect(zone).toHaveValue("Asia/Tokyo");
  await page.keyboard.press("Escape");
  await expect(preferences).toBeHidden();

  // Berlin is 7 or 8 hours behind Tokyo: the comment's time moves.
  await expect.poll(async () => plain(await comment.locator(".faint").first().innerText())).not.toBe(before);

  // Back to the system's zone.
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /Preferences/ }).click();
  await preferences.getByRole("tab", { name: "Time" }).click();
  await zone.fill("System");
  await page.getByRole("option", { name: /^System/ }).click();
  await page.keyboard.press("Escape");
  await expect.poll(async () => plain(await comment.locator(".faint").first().innerText())).toBe(before);
});

test("the formatting toolbar makes text bold, and the description saves it", async ({ page }) => {
  await newProject(page, "Notes", "NOTE");
  const panel = await newTask(page, "Write the summary");
  const description = panel.getByRole("region", { name: "Description" });
  await description.getByRole("button", { name: "Edit" }).click();
  const field = description.getByRole("textbox", { name: "Description" });
  await field.click();
  await page.keyboard.type("Ship it today");
  // Select "today" with the mouse: a double click on the line's last word.
  const line = field.locator(".cm-line").first();
  const box = await line.boundingBox();
  const end = await line.evaluate((element) => {
    const range = document.createRange();
    range.selectNodeContents(element);
    const rect = range.getBoundingClientRect();
    return rect.right;
  });
  if (!box) {
    throw new Error("the line isn't on screen");
  }
  await page.mouse.dblclick(end - 8, box.y + box.height / 2);
  const toolbar = description.getByRole("toolbar", { name: "Formatting" });
  await toolbar.getByRole("button", { name: "Bold" }).hover();
  await expect(page.getByRole("tooltip")).toContainText("Bold");
  await expect(page.getByRole("tooltip")).toContainText(/Ctrl\s*B/);
  await shot(page, "toolbar.png", panel);
  await toolbar.getByRole("button", { name: "Bold" }).click();
  await expect(field).toContainText("Ship it");
  await description.getByRole("button", { name: "Save" }).click();
  await expect(description.locator("strong", { hasText: "today" })).toBeVisible();
});
