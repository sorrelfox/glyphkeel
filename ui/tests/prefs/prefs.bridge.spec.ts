// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

const token = process.env.GK_BRIDGE_TOKEN ?? "";
const PAGE = `/?bridge=${process.env.GK_PREFS_BRIDGE_PORT ?? "5328"}&token=${token}`;
/** Where screenshots of the new screens go, when asked for. */
const SHOTS = process.env.GK_SHOTS;

async function shot(page: Page, name: string) {
  if (SHOTS !== undefined) {
    await page.screenshot({ path: join(SHOTS, name) });
  }
}

/** Errors the page reported; a flow fails on any. */
const errors: string[] = [];

test.beforeEach(({ page }) => {
  errors.length = 0;
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") {
      errors.push(message.text());
    }
  });
});

test.afterEach(() => {
  expect(errors).toEqual([]);
});

async function openPreferences(page: Page, tab: string) {
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /Preferences/ }).click();
  const dialog = page.getByRole("dialog", { name: "Preferences" });
  await dialog.getByRole("tab", { name: tab }).click();
  return dialog;
}

test("the first-run set-up works from the keyboard and is offered once", async ({ page }) => {
  await page.goto(`${PAGE}&first-run`);
  const setUp = page.getByRole("dialog", { name: "Set up Glyphkeel" });
  await expect(setUp).toBeVisible();
  await expect(setUp.getByRole("heading", { name: "Welcome to Glyphkeel" })).toBeVisible();
  await expect(setUp).toContainText("Personal");
  await expect(setUp).toContainText(/Step \W?1\W? of \W?4\W?/);

  // Next has the focus on each step.
  await page.keyboard.press("Enter");
  await expect(setUp.getByRole("heading", { name: "Choose a look" })).toBeVisible();
  await shot(page, "first-run.png");
  await page.keyboard.press("Enter");

  // The project step's field has the focus; Enter creates the project.
  await expect(setUp.getByRole("heading", { name: "Start a project" })).toBeVisible();
  await page.keyboard.type("Home chores");
  await page.keyboard.press("Enter");
  await expect(setUp.getByRole("heading", { name: "Work with agents" })).toBeVisible();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();

  // "Set up agents" ends the set-up on Preferences › Agents & CLI.
  await setUp.getByRole("button", { name: "Set up agents" }).click();
  const preferences = page.getByRole("dialog", { name: "Preferences" });
  await expect(preferences.getByRole("tab", { name: "Agents & CLI" })).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("Escape");

  // Finished once, it isn't offered again.
  await page.goto(`${PAGE}&first-run`);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");
  await expect(page.getByRole("dialog", { name: "Set up Glyphkeel" })).toHaveCount(0);
});

test("Agents & CLI: client set-up to copy, the most an agent may do, sessions", async ({ page }) => {
  await page.goto(PAGE);
  const dialog = await openPreferences(page, "Agents & CLI");
  const snippet = dialog.getByLabel(/^Set-up for \W?Claude Code\W?$/);
  await expect(snippet).toHaveText("claude mcp add --scope user glyphkeel -- glyphkeel mcp");
  await dialog.getByRole("button", { name: "Client" }).click();
  await page.getByRole("option", { name: "Codex" }).click();
  await expect(dialog.getByLabel(/^Set-up for \W?Codex\W?$/)).toContainText('[mcp_servers.glyphkeel]\ncommand = "glyphkeel"\nargs = ["mcp"]');
  await expect(dialog.getByLabel(/Set-up limited to/)).toContainText('"--project"');

  const policy = dialog.getByRole("radiogroup", { name: "Most an agent may do" });
  await expect(policy.getByRole("radio", { name: "Standard" })).toHaveAttribute("aria-checked", "true");
  await policy.getByRole("radio", { name: "Read only" }).click();
  await expect(policy.getByRole("radio", { name: "Read only" })).toHaveAttribute("aria-checked", "true");
  await expect(dialog).toContainText("but change nothing");
  await policy.getByRole("radio", { name: "Standard" }).click();
  await expect(policy.getByRole("radio", { name: "Standard" })).toHaveAttribute("aria-checked", "true");

  await expect(dialog).toContainText("No agent has connected yet.");
  await shot(page, "prefs-agents.png");
});

test("Preferences: shortcuts can be searched, and the language applies at once", async ({ page }) => {
  await page.goto(PAGE);
  const dialog = await openPreferences(page, "Keyboard shortcuts");
  await dialog.getByRole("searchbox", { name: "Search shortcuts" }).fill("command");
  await expect(dialog.getByRole("region", { name: "Navigation" })).toContainText("Search or run a command");
  await expect(dialog.getByRole("region", { name: "Tasks" })).toHaveCount(0);
  await shot(page, "prefs-shortcuts.png");
  await dialog.getByRole("searchbox", { name: "Search shortcuts" }).fill("no such action");
  await expect(dialog).toContainText("No shortcut matches.");

  // The pseudo-locale: accented, longer, in brackets.
  await dialog.getByRole("tab", { name: "Language" }).click();
  await dialog.getByRole("button", { name: "Language of the interface" }).click();
  await page.getByRole("option", { name: "Pseudo-locale (for testing)" }).click();
  // Even the dialog's own title changes.
  await expect(page.getByRole("dialog", { name: "[Þŕééƒééŕééñçééš]" }).getByRole("tab", { name: "[Ţííɱéé]" })).toBeVisible();
  // The close button: Esc right after choosing from a list in a dialog
  // can leave the window inert (an open item, not this page's).
  await page.getByRole("dialog", { name: "[Þŕééƒééŕééñçééš]" }).getByRole("button", { name: "[Çĺóóšéé]" }).click();
  // A board in the pseudo-locale, for clipped or untranslated text.
  await page.getByRole("row", { name: "Home chores" }).click();
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Home chores");
  await page.keyboard.type("Ŵááţéér ţĥéé þĺááñţš");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  await shot(page, "pseudo-locale.png");
});

test("the project tree's menu exports a project and moves it to the trash", async ({ page }) => {
  await page.goto(PAGE);
  const row = page.getByRole("row", { name: "Home chores" });
  await row.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Export…" }).click();
  const exporting = page.getByRole("dialog", { name: /Export/ });
  await expect(exporting.getByRole("checkbox", { name: /Home chores/ })).toBeChecked();
  await page.keyboard.press("Escape");

  await row.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Delete" }).click();
  const deleting = page.getByRole("dialog", { name: /^Delete \W?Home chores\W?$/ });
  await expect(deleting).toContainText("to the trash?");
  await deleting.getByRole("button", { name: "Move to trash" }).click();
  await expect(page.getByRole("status").filter({ hasText: "moved to the trash" })).toBeVisible();
  await expect(page.getByRole("row", { name: "Home chores" })).toHaveCount(0);
});
