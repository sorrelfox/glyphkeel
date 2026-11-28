// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

// The digest at the top of Today on the sample data, whose made-up history
// has two agent sessions, a rule with a failed run and a change through the
// command line: its groups and counts, the sidebar's Inbox, reverting one
// session, the run log, opening a session in Activity, marking it as seen;
// no WCAG 2.2 AA violations; in the pseudo-locale no text outside the
// message files; and right to left, a mirror image.

/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

const NOW = "2026-11-11T10:00:00";

/** Strips the isolation marks around placeables. */
const plain = (text: string) => text.replace(/[⁨⁩]/gu, "");

/** A locator's text without the isolation marks, read until it settles. */
const shown = (locator: Locator) => expect.poll(async () => plain(await locator.innerText()));

async function axe(page: Page) {
  const results = await new AxeBuilder({ page }).include("[data-digest]").options({ rules: { "target-size": { enabled: true } } }).withTags(TAGS).analyze();
  expect(results.violations.map((violation) => `${violation.id}: ${violation.nodes.map((node) => node.target.join(" ")).join(", ")}`)).toEqual([]);
}

async function start(page: Page): Promise<Locator> {
  await page.clock.setFixedTime(new Date(NOW));
  await page.goto("/");
  const digest = page.locator("[data-digest]");
  await expect(digest).toBeVisible();
  return digest;
}

test("the digest groups and counts what changed, reverts a session and opens it in Activity", async ({ page }) => {
  const digest = await start(page);
  await expect(digest.getByRole("heading", { level: 2, name: "While you were away" })).toBeVisible();
  const groups = await digest.locator("[data-digest-group]").evaluateAll((elements) => elements.map((element) => element.getAttribute("data-digest-group")));
  expect(groups).toEqual(["agents", "automation", "due", "cli"]);

  const agents = digest.locator("[data-digest-group=agents]");
  await shown(agents.locator("[data-digest-summary]")).toBe("2 agent sessions changed 9 items.");
  await expect(agents.getByRole("listitem")).toHaveCount(2);
  const claude = agents.getByRole("listitem").filter({ hasText: "Claude Code" });
  await expect(claude).toContainText("claude-code");
  await shown(claude).toContain("7 changes to 3 tasks");
  await shown(digest.locator("[data-digest-group=automation] [data-digest-summary]")).toMatch(/^One rule ran 2 times\.\s*One run failed\.$/u);
  await shown(digest.locator("[data-digest-group=cli] [data-digest-summary]")).toBe("One command changed 2 items.");

  // The sidebar's count: each session and rule, each reminder and task that
  // came due, and the command line once.
  const due = await digest.locator("[data-digest-group=due] li").count();
  const inbox = page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /^Inbox/ });
  await expect(inbox.locator("[data-count=inbox]")).toHaveText(String(2 + 1 + due + 1));
  await axe(page);

  // Inbox takes the focus to the digest.
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /^All tasks/ }).click();
  await inbox.click();
  await expect(page.getByRole("heading", { level: 2, name: "While you were away" })).toBeFocused();

  // The run log opens in place.
  await digest.getByRole("button", { name: /^Run log\W+Escalate blockers/u }).click();
  const log = digest.getByRole("list", { name: /^Run log\W+Escalate blockers/u });
  await expect(log.getByRole("listitem")).toHaveCount(2);
  await expect(log).toContainText("Failed");
  await axe(page);

  // Reverting one session leaves the other.
  await claude.getByRole("button", { name: /^Revert session\W+Claude Code/u }).click();
  await expect(page.getByText(/^\W*3\W*changes were reverted\.$/u)).toBeVisible();
  await expect(claude).toContainText("Reverted");
  await expect(claude.getByRole("button", { name: /^Revert/ })).toHaveCount(0);
  const planner = agents.getByRole("listitem").filter({ hasText: "Planner" });
  await expect(planner.getByRole("button", { name: /^Revert session\W+Planner/u })).toBeVisible();

  // A task of the session opens beside Today.
  const firstTask = claude.getByRole("button", { name: /^[A-Z]+-\d+$/u }).first();
  const key = plain((await firstTask.textContent()) ?? "").trim();
  await firstTask.click();
  await expect(page.getByRole("complementary", { name: new RegExp(`^Task\\W*${key}`, "u") })).toBeVisible();
  await page.goto("/");

  // Open in Activity shows that session only, until all are asked for.
  await page.locator("[data-digest]").getByRole("button", { name: /^Open in Activity\W+Planner/u }).click();
  await expect(page.locator("[data-activity-session=digest-session-2]")).toHaveText(/Only the changes of one agent session/u);
  await page.getByRole("button", { name: "Show all changes" }).click();
  await expect(page.locator("[data-activity-session]")).toHaveCount(0);
  await expect(page.getByRole("region", { name: "Activity" })).toBeVisible();
});

test("marking the digest as seen empties it and the Inbox count", async ({ page }) => {
  const digest = await start(page);
  const inbox = page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /^Inbox/ });
  await expect(inbox.locator("[data-count=inbox]")).toBeVisible();
  await digest.getByRole("button", { name: "Mark as seen" }).click();
  await expect(digest).toBeHidden();
  await expect(inbox.locator("[data-count=inbox]")).toHaveCount(0);
  // Today's own lists stay.
  await expect(page.locator("[data-today-section]").first()).toBeVisible();
});

// ---------- the pseudo-locale and right to left ----------

/** Accented letters inside brackets: the pseudo-locale's mark on a message. */
const PSEUDO_TEXT = /\[[^\]]*[áéíóúýƀçđĝĥĵķĺñþŕšţṽŵẋž]/u;

/** The sample data's own words: names, keys, titles, clients and projects, the rule's English error, and moments. */
const DATA = ["[class*=name]", ".key", "[class*=taskTitle]", "[data-client]", "[data-project]", "[class*=error]", "time"];

/** Text in the container, its labels and titles that isn't from a message file. */
function english(container: Locator): Promise<string[]> {
  return container.evaluate((root, data) => {
    const leftover = (text: string) => /[A-Za-z]{3,}/u.test(text.replace(/\[[^\]]*\]/gu, " ").replace(/⁨[^⁨⁩]*⁩/gu, " "));
    const skipped = (element: Element) => data.some((selector) => element.closest(selector) !== null);
    const found = new Set<string>();
    for (const element of root.querySelectorAll("*")) {
      if (skipped(element) || element.closest("[hidden]") !== null) {
        continue;
      }
      for (const name of ["aria-label", "aria-description", "placeholder", "title"]) {
        const value = element.getAttribute(name);
        if (value !== null && leftover(value)) {
          found.add(`${name}="${value}"`);
        }
      }
      const text = element.children.length === 0 ? element.textContent : "";
      if (leftover(text)) {
        found.add(`${element.tagName.toLowerCase()}: ${text.trim().slice(0, 80)}`);
      }
    }
    return [...found];
  }, DATA);
}

test.describe("in the pseudo-locale", () => {
  test.use({ locale: "en-XA" });

  test("the digest has no text outside the message files", async ({ page }) => {
    const digest = await start(page);
    await digest.locator("[data-digest-rule] button[aria-expanded]").click();
    const text = await digest.innerText();
    test.skip(!PSEUDO_TEXT.test(text), "the page doesn't show the pseudo-locale");
    expect(await english(digest)).toEqual([]);
  });
});

interface Box {
  name: string;
  start: number;
  end: number;
}

function boxes(container: Locator): Promise<Box[]> {
  return container.evaluate((root) => {
    const elements = [...root.querySelectorAll("button, h2, h3, p, li")].filter((element) => {
      const box = element.getBoundingClientRect();
      return box.width >= 2 && box.height >= 2;
    });
    const frame = root.getBoundingClientRect();
    return elements.map((element) => {
      const box = element.getBoundingClientRect();
      const name = element.getAttribute("aria-label") ?? element.textContent;
      return { name: `${element.tagName.toLowerCase()} "${name.trim().slice(0, 40)}"`, start: box.left - frame.left, end: frame.right - box.right };
    });
  });
}

test("right to left, the digest is the mirror image of left to right", async ({ browser }, info) => {
  const { baseURL, viewport } = info.project.use;
  const measure = async (locale: string, dir: "ltr" | "rtl") => {
    const context = await browser.newContext({ baseURL, viewport, locale });
    const page = await context.newPage();
    const digest = await start(page);
    await expect(page.locator("html")).toHaveAttribute("dir", dir);
    const found = await boxes(digest);
    await context.close();
    return found;
  };
  const ltr = await measure("en-US", "ltr");
  const rtl = await measure("he-IL", "rtl");
  expect(ltr.length).toBeGreaterThan(10);
  expect(rtl.length).toBe(ltr.length);
  const off = ltr.flatMap((a, at) => {
    const b = rtl[at];
    return b && Math.abs(a.start - b.end) > 4 ? [`${a.name}: ${a.start.toFixed(0)} from the left, ${b.end.toFixed(0)} from the right`] : [];
  });
  expect(off).toEqual([]);
});
