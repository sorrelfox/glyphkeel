// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Page } from "@playwright/test";

// The roadmap on the sample data: Task Manager has versions 0.9 (closed),
// 1.0 (open, shared with its sub-projects) and 1.1. The clock is fixed, so
// the sample data is the same on every run.

const TODAY = "2026-11-11";

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null) => (text ?? "").replace(/[⁨⁩]/g, "");

async function openRoadmap(page: Page, project = /Task Manager/) {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await page.getByRole("row", { name: project }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: "Roadmap" }).click();
  await expect(page.getByRole("region", { name: /Roadmap of/ })).toBeVisible();
}

const card = (page: Page, name: string) => page.getByRole("article", { name, exact: true });

test("versions show with their progress; closed ones on request", async ({ page }) => {
  await openRoadmap(page);
  await expect(card(page, "1.0")).toBeVisible();
  await expect(card(page, "1.1")).toBeVisible();
  await expect(card(page, "0.9")).toHaveCount(0);
  const progress = card(page, "1.0").getByRole("progressbar");
  await expect(progress).toHaveAttribute("aria-valuemin", "0");
  await expect(card(page, "1.0").getByText(/tasks? closed/)).toBeVisible();
  await page.getByText(/Show closed versions/).click();
  await expect(card(page, "0.9")).toBeVisible();
});

test("a version is made, a task planned for it in the panel, and the version closed with its open tasks moved", async ({ page }) => {
  await openRoadmap(page);
  await page.getByRole("button", { name: "New version" }).click();
  const dialog = page.getByRole("dialog", { name: "New version" });
  await dialog.getByLabel("Name").fill("2.0");
  await dialog.getByLabel("Due date").fill("2026-12-20");
  await dialog.getByRole("button", { name: "Create" }).click();
  await expect(card(page, "2.0")).toBeVisible();
  await expect(card(page, "2.0").getByText(/Dec/)).toBeVisible();

  // Open a task of 1.0 in the panel and plan it for 2.0.
  const first = card(page, "1.0")
    .getByRole("list", { name: /Tasks of .1\.0/ })
    .getByRole("listitem")
    .filter({ hasNot: page.getByLabel("Closed", { exact: true }) })
    .getByRole("button")
    .first();
  const key = plain(await first.locator(".key").textContent());
  await first.click();
  const version = page.getByRole("button", { name: /^Version: / });
  await expect(version).toHaveAccessibleName(/1\.0/);
  await version.click();
  await page.getByRole("menuitem", { name: "2.0" }).click();
  await expect(version).toHaveAccessibleName(/2\.0/);
  await expect(card(page, "2.0").getByRole("button", { name: new RegExp(`^${key} `) })).toBeVisible();

  // Close 2.0; its open task goes to 1.1.
  await card(page, "2.0").getByRole("button", { name: /^Version .2\.0.$/ }).click();
  await page.getByRole("menuitem", { name: "Close version" }).click();
  const close = page.getByRole("dialog", { name: /Close/ });
  await expect(close.getByText(/still open/)).toBeVisible();
  await close.getByRole("button", { name: /Open tasks go to/ }).click();
  await page.getByRole("option", { name: "1.1" }).click();
  await close.getByRole("button", { name: "Close version" }).click();
  await expect(card(page, "2.0")).toHaveCount(0);
  await expect(card(page, "1.1").getByRole("button", { name: new RegExp(`^${key} `) })).toBeVisible();
  await expect(version).toHaveAccessibleName(/1\.1/);

  // Undo brings the version and the task back.
  await page.getByRole("button", { name: "Undo" }).last().click();
  await expect(card(page, "2.0")).toBeVisible();
  await expect(version).toHaveAccessibleName(/2\.0/);
});

test("the Gantt chart marks versions with a due date", async ({ page }) => {
  await openRoadmap(page);
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: "Gantt" }).click();
  await expect(page.getByRole("treegrid", { name: "Gantt chart" })).toBeVisible();
  await expect(page.locator('[data-version="1.0"]')).toBeAttached();
  await expect(page.locator('[data-version="1.1"]')).toHaveCount(0);
});

test.describe("in the pseudo-locale", () => {
  test.use({ locale: "en-XA" });

  /** Accented letters inside brackets: the pseudo-locale's mark on a message. */
  const PSEUDO_TEXT = /\[[^\]]*[áéíóúýƀçđĝĥĵķĺñþŕšţṽŵẋž]/u;

  test("the roadmap's own words all come from the message files", async ({ page }) => {
    await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
    await page.goto("/");
    const body = await page.locator("body").innerText();
    test.skip(!PSEUDO_TEXT.test(body), "the page doesn't show the pseudo-locale");
    await page.getByRole("row", { name: /Task Manager/ }).first().click();
    // The roadmap is the fifth of the header's views.
    await page.locator(".seg[role=group] button").nth(4).click();
    const screen = page.locator("section").filter({ has: page.getByRole("article") }).first();
    await expect(screen).toBeVisible();
    const controls = screen.locator("> div").first();
    for (const text of await controls.locator("button, label, span").allInnerTexts()) {
      if (text.trim() !== "") {
        expect(text, "a control's text").toMatch(PSEUDO_TEXT);
      }
    }
    const article = page.getByRole("article", { name: "1.0", exact: true });
    expect(await article.getByRole("progressbar").locator("xpath=..").innerText()).toMatch(PSEUDO_TEXT);
    await article.getByRole("button").first().click();
    const menu = page.getByRole("menu");
    for (const text of await menu.getByRole("menuitem").allInnerTexts()) {
      expect(text).toMatch(PSEUDO_TEXT);
    }
  });
});
