// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

/** A name with placeables, matched whatever isolation marks Fluent put in it. */
const named = (text: string, whole = true) => new RegExp(`${whole ? "^[⁨⁩]*" : ""}${Array.from(text).map((c) => c.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("[⁨⁩]*")}${whole ? "[⁨⁩]*$" : ""}`);

/** The text of an element without isolation marks. */
const textOf = (locator: Locator) => async () => plain(await locator.innerText());

async function openBoard(page: Page, project: RegExp) {
  await page.getByRole("row", { name: project }).first().click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
}

function lane(page: Page, name: string): Locator {
  return page.locator("section[data-lane]").filter({ has: page.getByRole("heading", { name, exact: true }) });
}

async function addTask(page: Page, title: string): Promise<Locator> {
  await lane(page, "To do").getByRole("button", { name: /Add task to/ }).click();
  await page.keyboard.type(title);
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  const card = page.locator("section[data-lane] [data-card]", { hasText: title });
  await expect(card).toBeVisible();
  return card;
}

const panelOf = (page: Page) => page.getByRole("complementary", { name: /^Task / });

test("make a task repeat, close it for the next one, skip it and stop it", async ({ page }) => {
  await page.goto("/");
  await openBoard(page, /Personal/);
  const card = await addTask(page, "Water the plants");
  await card.dblclick();
  const panel = panelOf(page);

  // The editor: a rule typed in words, previewed by the engine.
  await panel.getByRole("button", { name: /^Repeats:/ }).click();
  const editor = page.getByRole("dialog", { name: "Repeat this task" });
  await editor.getByRole("textbox", { name: "Or type the rule" }).fill("every monday and thursday");
  await expect.poll(async () => plain(await editor.getByTestId("repeat-preview-summary").innerText())).toBe("Every week on Monday and Thursday");
  await expect(editor.getByRole("list", { name: "Next dates" }).getByRole("listitem")).toHaveCount(5);
  await editor.getByRole("button", { name: "Save" }).click();
  await expect(editor).toBeHidden();

  const note = panel.getByTestId("recurrence-note");
  await expect.poll(textOf(note)).toContain("Every week on Monday and Thursday");
  await expect.poll(textOf(note)).toContain("1 occurrence so far");
  await expect.poll(textOf(note)).toContain("this occurrence only");
  await expect(panel.getByRole("button", { name: named("Repeats: Every week on Monday and Thursday") })).toBeVisible();

  // Closing it makes the next one. The key is read first: once it is closed,
  // the next occurrence carries the same title.
  const first = (await card.getAttribute("data-card")) ?? "";
  expect(first).toMatch(/^PER-\d+$/);
  await panel.getByRole("button", { name: /^Status:/ }).click();
  await page.getByRole("menuitem", { name: "Done" }).click();
  await expect(page.locator(`section[data-lane] [data-card="${first}"]`)).toHaveClass(/\bdone\b/);
  const next = page.locator("section[data-lane] [data-card]", { hasText: "Water the plants" }).and(page.locator(`:not([data-card="${first}"])`));
  await expect(next).toBeVisible();
  await next.dblclick();
  await expect.poll(textOf(panel.getByTestId("recurrence-note"))).toContain("2 occurrences so far");

  // The list marks the current occurrence.
  const nextKey = (await next.getAttribute("data-card")) ?? "";
  expect(nextKey).toMatch(/^PER-\d+$/);

  // Skip moves its dates to the next of the rule.
  const due = plain(await panel.getByTestId("recurrence-next").innerText());
  await panel.getByRole("button", { name: "Skip this one" }).click();
  await expect(page.getByText(named("Skipped; the task is now due", false))).toBeVisible();
  await expect.poll(async () => plain(await panel.getByTestId("recurrence-next").innerText())).not.toBe(due);
  await expect.poll(textOf(panel.getByTestId("recurrence-note"))).toContain("3 occurrences so far");

  // Stopping keeps the task; Undo brings the series back.
  await panel.getByRole("button", { name: "Stop repeating" }).click();
  await expect(panel.getByTestId("recurrence-note")).toBeHidden();
  await expect(panel.getByRole("button", { name: named("Repeats: Doesn't repeat") })).toBeVisible();
  await page.getByRole("button", { name: "Undo" }).first().click();
  await expect(panel.getByTestId("recurrence-note")).toBeVisible();

  // The list marks the current occurrence, and only it.
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "List" }).click();
  const field = page.getByRole("textbox", { name: "Filter query" });
  await field.click();
  await page.keyboard.type("Water");
  await page.keyboard.press("Escape");
  const current = page.locator("[data-index]", { hasText: nextKey });
  await expect(current.getByRole("img", { name: "Recurring task" })).toBeVisible();
  await expect(page.locator("[data-index]", { hasText: first }).getByRole("img", { name: "Recurring task" })).toHaveCount(0);
});

test("the structured editor builds a monthly rule from its choices", async ({ page }) => {
  await page.goto("/");
  await openBoard(page, /Personal/);
  const card = await addTask(page, "Pay the rent");
  await card.dblclick();
  const panel = panelOf(page);
  await panel.getByRole("button", { name: /^Repeats:/ }).click();
  const editor = page.getByRole("dialog", { name: "Repeat this task" });
  await editor.getByRole("button", { name: /Unit/ }).click();
  await page.getByRole("option", { name: "month" }).click();
  await editor.getByRole("button", { name: /Falls on/ }).click();
  await page.getByRole("option", { name: "A weekday of the month" }).click();
  await editor.getByRole("button", { name: /Which one/ }).click();
  await page.getByRole("option", { name: "Last" }).click();
  await editor.getByRole("button", { name: /Weekday/ }).click();
  await page.getByRole("option", { name: "Friday" }).click();
  await editor.getByRole("button", { name: /Ends/ }).click();
  await page.getByRole("option", { name: "After a number of times" }).click();
  await editor.getByRole("spinbutton", { name: "Number of times" }).fill("3");
  await expect.poll(async () => plain(await editor.getByTestId("repeat-preview-summary").innerText())).toBe("Every month on the last Friday, 3 times");
  await expect(editor.getByRole("list", { name: "Next dates" }).getByRole("listitem")).toHaveCount(3);
  await editor.getByRole("button", { name: "Save" }).click();
  await expect.poll(textOf(panel.getByTestId("recurrence-note"))).toContain("Every month on the last Friday, 3 times");
});

test("save a task as a template, start a task from it, and manage templates in the project's settings", async ({ page }) => {
  await page.goto("/");
  await openBoard(page, /Personal/);
  const card = await addTask(page, "Plan the trip");
  await card.dblclick();
  const panel = panelOf(page);
  await panel.getByRole("button", { name: "Task menu" }).click();
  await page.getByRole("menuitem", { name: "Save as template…" }).click();
  const dialog = page.getByRole("dialog", { name: "Save as template" });
  await dialog.getByRole("textbox", { name: "Name" }).fill("Trip");
  await dialog.getByRole("button", { name: "Save template" }).click();
  await expect(page.getByText(named("Template “Trip” saved", false))).toBeVisible();

  // The Add task field offers the template; its own title is kept when the field is empty.
  await lane(page, "To do").getByRole("button", { name: /Add task to/ }).click();
  await lane(page, "To do").getByRole("button", { name: /Template/ }).click();
  await page.getByRole("option", { name: "Trip" }).click();
  await lane(page, "To do").getByRole("textbox", { name: /Title of the new task/ }).fill("Plan the ski trip");
  await page.keyboard.press("Enter");
  await expect(page.locator("section[data-lane] [data-card]", { hasText: "Plan the ski trip" })).toBeVisible();

  // Project settings: the task template, renamed, deleted and brought back.
  await page.getByRole("button", { name: /Personal.*project menu/ }).click();
  await page.getByRole("menuitem", { name: /Project settings/ }).click();
  const settings = page.getByRole("dialog");
  await settings.getByRole("tab", { name: "Templates" }).click();
  const own = settings.getByRole("region", { name: "Task templates of this project" });
  await expect(own.getByRole("textbox", { name: named("Name of Trip") })).toHaveValue("Trip");
  await own.getByRole("textbox", { name: named("Description of Trip") }).fill("Holidays");
  await own.getByRole("textbox", { name: named("Description of Trip") }).press("Enter");
  await own.getByRole("button", { name: named("Delete Trip") }).click();
  await expect(own.getByText("No task templates yet.")).toBeVisible();
  await page.getByRole("button", { name: "Undo" }).first().click();
  await expect(own.getByRole("textbox", { name: named("Description of Trip") })).toHaveValue("Holidays");

  // The project as a project template, offered when making a new project.
  await settings.getByRole("textbox", { name: "Name", exact: true }).fill("Household");
  await settings.getByRole("button", { name: "Save as template" }).click();
  await expect(settings.getByRole("region", { name: "Project templates" }).getByRole("textbox", { name: named("Name of Household") })).toBeVisible();

  // A task type's default due date.
  await settings.getByRole("tab", { name: "Task types" }).click();
  const offset = settings.getByRole("textbox", { name: named("Default due date of Bug", false) });
  await offset.fill("3");
  await offset.press("Enter");
  await expect(offset).toHaveValue("3");
});

test("a new project starts from a project template", async ({ page }) => {
  await page.goto("/");
  await openBoard(page, /Personal/);
  await page.getByRole("button", { name: /Personal.*project menu/ }).click();
  await page.getByRole("menuitem", { name: /Project settings/ }).click();
  const settings = page.getByRole("dialog");
  await settings.getByRole("tab", { name: "Templates" }).click();
  await settings.getByRole("textbox", { name: "Name", exact: true }).fill("Home set-up");
  await settings.getByRole("button", { name: "Save as template" }).click();
  await expect(settings.getByRole("region", { name: "Project templates" }).getByRole("textbox", { name: named("Name of Home set-up") })).toBeVisible();
  await settings.getByRole("button", { name: "Close" }).first().click();
  await expect(settings).toBeHidden();

  await page.getByRole("button", { name: /New project/ }).first().click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("textbox").first().fill("Second home");
  await dialog.getByRole("button", { name: /Start from/ }).click();
  await page.getByRole("option", { name: "Home set-up" }).click();
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(dialog).toBeHidden();
  await expect(page.getByRole("row", { name: /Second home/ }).first()).toBeVisible();
});
