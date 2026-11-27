// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

// A project's dashboards from inside the project: its menu leads to the
// dashboards page, a dashboard is pinned to the project from there, and the
// menu then lists it.

/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

test("a project's menu opens its pinned dashboards, and a dashboard is pinned to it", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("navigation", { name: "Workspace" }).getByText("Task Manager").first().click();
  const projectMenu = page.getByRole("button", { name: /Task Manager.*project menu/ });

  // No dashboard is pinned yet: the menu offers to pin one.
  await projectMenu.click();
  await page.getByRole("menuitem", { name: "Pin a dashboard here…" }).click();
  await expect(page.getByRole("region", { name: "Dashboards" })).toBeVisible();
  await page.getByRole("button", { name: "Create an empty one" }).click();

  // The dashboard menu pins the dashboard shown to the project it came from.
  await page.getByRole("button", { name: "Dashboard menu" }).click();
  const pin = page.getByRole("menuitem", { name: /^Pin to [⁨]?TM/ });
  const results = await new AxeBuilder({ page }).withTags(TAGS).include(".menuPopover").analyze();
  expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`)).toEqual([]);
  await pin.click();
  await expect(page.getByText(/pinned to/)).toBeVisible();

  // Back in the project, its menu lists the dashboard and opens it.
  await page.getByRole("navigation", { name: "Workspace" }).getByText("Task Manager").first().click();
  await projectMenu.click();
  await expect(page.getByRole("menuitem", { name: "All dashboards…" })).toBeVisible();
  await page.getByRole("menuitem", { name: "New dashboard" }).click();
  await expect(page.getByRole("region", { name: "Dashboards" })).toBeVisible();
  await expect(page.getByText("This dashboard has no widgets yet.")).toBeVisible();

  // Rename or describe also moves it back to the whole workspace.
  await page.getByRole("button", { name: "Dashboard menu" }).click();
  await page.getByRole("menuitem", { name: "Rename or describe…" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: /^Pinned to/ }).click();
  await page.getByRole("option", { name: "The whole workspace" }).click();
  await dialog.getByRole("button", { name: "Save" }).click();
  await expect(dialog).toHaveCount(0);
  await page.getByRole("navigation", { name: "Workspace" }).getByText("Task Manager").first().click();
  await projectMenu.click();
  await expect(page.getByRole("menuitem", { name: "Pin a dashboard here…" })).toBeVisible();
});
