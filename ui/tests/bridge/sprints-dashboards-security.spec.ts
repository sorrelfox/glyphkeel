// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { execFileSync } from "node:child_process";

import { expect, test, type Locator, type Page } from "@playwright/test";

// Sprints, automation rules, dashboards, card colours and covers, and the
// Security and Agents pages of Preferences, on the real engine. Each flow
// makes its own project from the command line, another process on the same
// workspace, and checks what the engine holds with it afterwards.
//
// The app lock and the local HTTP endpoint belong to the app, not the
// engine: outside the app the page keeps stand-ins for them. Their flows
// check that the pages work beside a real workspace, not the app's side.

const token = process.env.GK_BRIDGE_TOKEN ?? "";
const PAGE = `/?bridge=${process.env.GK_BRIDGE_PORT ?? "5321"}&token=${token}`;

/** A 1×1 PNG. */
const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

/** Runs `glyphkeel --json <args>` on the bridge's workspace and returns its output. */
function gk(...args: string[]): unknown {
  const out = execFileSync(process.env.GK_BRIDGE_CLI ?? "glyphkeel", ["--workspace", process.env.GK_BRIDGE_WORKSPACE ?? "", "--json", "--tz", "UTC", ...args], {
    encoding: "utf8",
    env: { ...process.env, NO_COLOR: "1" },
  });
  return out.trim() === "" ? null : JSON.parse(out);
}

/** A task made from the command line; returns its key. */
function task(project: string, title: string, ...more: string[]): string {
  const made = gk("task", "create", "--project", project, "--title", title, ...more) as { task: { key: string } };
  return made.task.key;
}

/** A task as the engine has it. */
function shown(key: string): Record<string, unknown> {
  const out = gk("task", "show", key) as { items: Record<string, unknown>[] };
  return out.items[0] ?? {};
}

/** A whole name, whatever isolation marks Fluent put around its placeables. */
const loose = (text: string, whole = true) => {
  const body = Array.from(text, (c) => c.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("[⁨⁩]*");
  return new RegExp(whole ? `^[⁨⁩]*${body}[⁨⁩]*$` : `^[⁨⁩]*${body}`);
};

async function open(page: Page): Promise<void> {
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).not.toHaveText("Workspace unavailable");
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeVisible();
}

/** Opens a project made from the command line on its board. */
async function openProject(page: Page, name: string): Promise<void> {
  await open(page);
  await page.getByRole("navigation", { name: "Workspace" }).getByText(name, { exact: true }).first().click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
}

async function preferences(page: Page, tab: string): Promise<Locator> {
  await page.keyboard.press("Control+Comma");
  const dialog = page.getByRole("dialog", { name: "Preferences" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("tab", { name: tab }).click();
  return dialog.getByRole("tabpanel", { name: tab });
}

test("a sprint is planned, filled, started and a task joins it from its panel", async ({ page }) => {
  gk("project", "create", "--name", "Orchard", "--key", "ORC");
  const prune = task("ORC", "Prune the apples");
  const pick = task("ORC", "Pick the pears");
  await openProject(page, "Orchard");
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: "Sprints" }).click();
  await expect(page.getByRole("region", { name: /Sprints of/ })).toBeVisible();

  await page.getByRole("button", { name: "New sprint" }).click();
  const dialog = page.getByRole("dialog", { name: "New sprint" });
  await expect(dialog.getByLabel("Name")).toHaveValue("Sprint 1");
  await dialog.getByLabel("Goal").fill("The first harvest");
  await dialog.getByRole("button", { name: "Create sprint" }).click();
  const card = page.getByRole("article", { name: "Sprint 1", exact: true });
  await expect(card).toBeVisible();

  // One task by choice, through "Move to".
  const backlog = page.getByRole("grid", { name: loose("Backlog") });
  const row = backlog.getByRole("row", { name: loose(`${prune} `, false) });
  await row.locator("label").first().click();
  await page.getByRole("button", { name: "Move to" }).click();
  await page.getByRole("menuitem", { name: "Sprint 1" }).click();
  await expect(page.getByRole("grid", { name: loose("Tasks of Sprint 1") }).getByRole("row", { name: loose(`${prune} `, false) })).toBeVisible();

  await card.getByRole("button", { name: loose("Start Sprint 1") }).click();
  await page.getByRole("dialog", { name: loose("Start Sprint 1") }).getByRole("button", { name: "Start sprint" }).click();
  await expect(card.getByText("Active", { exact: true })).toBeVisible();
  const sprints = gk("sprint", "list", "ORC") as { items: { name: string; state: string; goal?: string }[] };
  expect(sprints.items.map((s) => [s.name, s.state, s.goal])).toEqual([["Sprint 1", "active", "The first harvest"]]);
  expect((shown(prune)["sprint"] as { name?: string } | undefined)?.name).toBe("Sprint 1");

  // The other task joins the running sprint from the task panel's picker.
  await backlog.getByRole("row", { name: loose(`${pick} `, false) }).click();
  const panel = page.getByRole("complementary", { name: /^Task / });
  await panel.getByRole("button", { name: loose("Sprint: Backlog") }).click();
  await page.getByRole("menuitemradio", { name: loose("Sprint 1 (active)") }).click();
  await expect(panel.getByRole("button", { name: loose("Sprint: Sprint 1 Active") })).toBeVisible();
  await expect.poll(() => (shown(pick)["sprint"] as { name?: string } | undefined)?.name).toBe("Sprint 1");
});

test("an automation rule saved in the project's settings acts on a new task", async ({ page }) => {
  gk("project", "create", "--name", "Workshop", "--key", "WSH");
  gk("label", "add", "--project", "WSH", "--name", "triage");
  await openProject(page, "Workshop");
  await page.getByRole("button", { name: /Workshop.*project menu/ }).click();
  await page.getByRole("menuitem", { name: /Project settings/ }).click();
  const settings = page.getByRole("dialog");
  await settings.getByRole("tab", { name: "Automation" }).click();
  await settings.getByRole("button", { name: "New rule" }).click();
  const editor = settings.getByRole("form", { name: "New rule" });
  await editor.getByRole("textbox", { name: "Name" }).fill("Triage crashes");
  await editor.getByRole("textbox", { name: "Only for tasks that match" }).fill("title:crash");
  await editor.getByRole("button", { name: "Save rule" }).click();
  await expect(settings.getByTestId("automation-Triage crashes")).toBeVisible();
  await page.keyboard.press("Escape");
  const rules = gk("automation", "list", "--project", "WSH") as { items: { name: string; enabled: boolean }[] };
  expect(rules.items.map((r) => [r.name, r.enabled])).toEqual([["Triage crashes", true]]);

  // A task made on the board that matches gets the rule's label.
  const lane = page.locator("section[data-lane]").first();
  await lane.getByRole("button", { name: "Add task", exact: true }).click();
  await lane.getByRole("textbox", { name: /Title of the new task/ }).fill("Lathe crash");
  await page.keyboard.press("Enter");
  const card = page.locator("section[data-lane] [data-card]", { hasText: "Lathe crash" });
  await expect(card).toContainText("triage");
  const key = (await card.getAttribute("data-card")) ?? "";
  const labels = (shown(key)["labels"] as { name: string }[] | undefined)?.map((l) => l.name);
  expect(labels).toEqual(["triage"]);
  const runs = gk("automation", "runs", "--project", "WSH") as { items: unknown[] };
  expect(runs.items).toHaveLength(1);
});

test("a starter dashboard shows what the engine counts", async ({ page }) => {
  gk("project", "create", "--name", "Gauges", "--key", "GAU");
  task("GAU", "Read the meter", "--priority", "urgent");
  task("GAU", "Log the reading");
  await open(page);
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /^Dashboards/ }).click();
  await expect(page.getByRole("region", { name: "Dashboards" })).toBeVisible();
  await page.getByRole("button", { name: "Create a starter dashboard" }).click();
  const widgets = page.getByRole("list", { name: "Widgets" }).locator(":scope > [data-widget]");
  await expect(widgets).toHaveCount(6);

  // The open tasks the first widget counts are the engine's.
  const counted = gk("task", "list", "--query", "is:open", "--limit", "1") as { total: number };
  await expect(page.locator('[data-widget="query_count"]')).toContainText(String(counted.total));
  const dashboards = gk("dashboard", "list", "--all", "true") as { items: { name: string; widgets: { kind: string }[] }[] };
  expect(dashboards.items).toHaveLength(1);
  expect(dashboards.items[0]?.widgets.map((w) => w.kind)).toEqual(["query_count", "overdue", "due_soon", "tasks_by_field", "created_closed", "activity"]);
});

test("the board colours cards by priority and shows their first image, as the engine keeps them", async ({ page }) => {
  gk("project", "create", "--name", "Gatehouse", "--key", "GTH");
  const gate = task("GTH", "Fix the gate", "--priority", "urgent");
  task("GTH", "Paint the fence");
  gk("file", "attach", "--task", gate, "--name", "gate.png", "--data-base64", PNG);
  await openProject(page, "Gatehouse");
  const options = async (item: string) => {
    await page.getByRole("button", { name: /^Board options/ }).click();
    await page.getByRole("menuitemradio", { name: item }).click();
  };
  await options("By priority");
  await expect(page.locator(`[data-card="${gate}"][data-edge=red]`)).toBeVisible();
  await options("Chosen or first image");
  const cover = page.locator(`[data-card="${gate}"] img`);
  await expect(cover).toBeVisible();
  await expect(cover).toHaveAttribute("alt", "gate.png");

  // The engine has the layout and works the edge and cover out itself.
  const board = gk("board", "GTH") as { layout: { colour_by: string; covers: string }; columns: { cards: { key: string; edge?: { hue: string }; cover?: { name: string } }[] }[] };
  expect([board.layout.colour_by, board.layout.covers]).toEqual(["priority", "first_image"]);
  const card = board.columns.flatMap((c) => c.cards).find((c) => c.key === gate);
  expect([card?.edge?.hue, card?.cover?.name]).toEqual(["red", "gate.png"]);
  // And it stays when the page is opened again.
  await openProject(page, "Gatehouse");
  await expect(page.locator(`[data-card="${gate}"][data-edge=red] img`)).toBeVisible();
});

test("the Security page locks the page and the passphrase opens the workspace again", async ({ page }) => {
  await open(page);
  const panel = await preferences(page, "Security");
  await panel.getByRole("button", { name: "Set…" }).click();
  await panel.getByLabel("New passphrase").fill("orchard lantern");
  await panel.getByLabel("The same passphrase again").fill("orchard lantern");
  await panel.getByRole("button", { name: "Save" }).click();
  await expect(panel.getByText("A passphrase is set.")).toBeVisible();
  await panel.getByRole("button", { name: "Lock", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Glyphkeel is locked" })).toBeVisible();
  await expect(page.getByRole("navigation", { name: "Workspace" })).toHaveCount(0);
  await page.getByLabel("Passphrase", { exact: true }).fill("orchard lantern");
  await page.getByRole("button", { name: "Unlock" }).click();
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeVisible();
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");
});

test("the Agents page shows the HTTP endpoint's set-up and keeps the workspace's confirmations setting", async ({ page }) => {
  await open(page);
  const panel = await preferences(page, "Agents & CLI");
  // Outside the app nothing listens; the set-up is still shown.
  await expect(panel.getByText("Only the app serves agents over HTTP.", { exact: false })).toBeVisible();
  const ask = panel.getByRole("switch", { name: "Agents may ask you" });
  await expect(ask).toBeChecked();
  await ask.focus();
  await page.keyboard.press("Space");
  await expect(ask).not.toBeChecked();
  // The setting is the workspace's, in the engine.
  const settings = () => (gk("workspace") as { settings: { agent_confirmations?: boolean; agent_max_profile: string } }).settings;
  await expect.poll(() => settings().agent_confirmations).toBe(false);
  // Turned on again from the command line, the page shows it on.
  gk("agents", "policy", "--max-profile", settings().agent_max_profile, "--confirmations", "true");
  await open(page);
  const again = await preferences(page, "Agents & CLI");
  await expect(again.getByRole("switch", { name: "Agents may ask you" })).toBeChecked();
});
