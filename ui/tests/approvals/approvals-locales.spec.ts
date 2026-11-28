// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

// The Approvals inbox, its sidebar count and the approvals rows of
// Preferences in the pseudo-locale (every word from the message files,
// nothing cut off at a third more length) and right to left (the cards
// mirror), with no WCAG 2.2 AA violations in either.

/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

/** Accented letters inside brackets: the pseudo-locale's mark on a message. */
const PSEUDO_TEXT = /\[[^\]]*[áéíóúýƀçđĝĥĵķĺñþŕšţṽŵẋž]/u;

async function axe(page: Page) {
  const results = await new AxeBuilder({ page }).options({ rules: { "target-size": { enabled: true } } }).withTags(TAGS).analyze();
  expect(results.violations.map((violation) => `${violation.id}: ${violation.nodes.map((node) => node.target.join(" ")).join(", ")}`)).toEqual([]);
}

const approvalsButton = (page: Page) => page.locator("nav button").filter({ has: page.locator("svg.lucide-shield-check") });
const screenOf = (page: Page) => page.locator("section[data-screen=approvals]");

async function openApprovals(page: Page) {
  await page.goto("/?proposals");
  await approvalsButton(page).click();
  await expect(screenOf(page).locator("[data-proposal]").first()).toBeVisible();
}

/** Texts in `root` cut off: wider than their box, with the overflow hidden and no ellipsis. */
function clipped(root: Locator): Promise<string[]> {
  return root.evaluate((element) => {
    const found: string[] = [];
    for (const each of [element, ...element.querySelectorAll("*")]) {
      if (!(each instanceof HTMLElement)) {
        continue;
      }
      const style = getComputedStyle(each);
      const text = each.innerText.trim();
      // Visually hidden text (a select's native options, for assistive technology) is no box to fit.
      const box = each.getBoundingClientRect();
      const hidden = box.width < 2 || box.height < 2 || each.closest("[data-react-aria-prevent-focus], [aria-hidden=true]") !== null;
      if (hidden || text === "" || style.overflowX === "visible" || style.display === "contents") {
        continue;
      }
      if (each.scrollWidth > each.clientWidth + 1 && style.textOverflow !== "ellipsis") {
        found.push(text.slice(0, 60));
      }
    }
    return found;
  });
}

/** The texts of the leaves of `root` that no message made: no pseudo-locale mark. */
function untranslated(root: Locator): Promise<string[]> {
  return root.evaluate((element, pattern) => {
    const mark = new RegExp(pattern, "u");
    const found: string[] = [];
    for (const each of element.querySelectorAll("h1, h2, h3, p, li, button, label, span, [role=radio], textarea")) {
      if (!(each instanceof HTMLElement)) {
        continue;
      }
      // Leaves only: an element whose own text nodes say something.
      const own = [...each.childNodes].filter((node) => node.nodeType === Node.TEXT_NODE).map((node) => node.textContent ?? "").join("").trim();
      // A message of placeables only (a task's key and title) has its brackets and nothing to accent.
      const text = each.innerText.trim();
      if (own !== "" && !mark.test(text) && !(text.startsWith("[") && text.endsWith("]"))) {
        found.push(own.slice(0, 60));
      }
    }
    return found;
  }, PSEUDO_TEXT.source);
}

test.describe("in the pseudo-locale", () => {
  test.use({ locale: "en-XA" });

  test("the inbox, its count and the setting come from the message files and fit", async ({ page }) => {
    await page.goto("/?proposals");
    test.skip(!PSEUDO_TEXT.test(await page.locator("body").innerText()), "the page doesn't show the pseudo-locale");
    await openApprovals(page);
    const screen = screenOf(page);
    expect(await untranslated(screen)).toEqual([]);
    expect(await clipped(screen)).toEqual([]);
    expect(await clipped(approvalsButton(page))).toEqual([]);
    await expect(approvalsButton(page)).toHaveText(PSEUDO_TEXT);
    await axe(page);

    // The rejection dialog.
    await screen.locator("[data-proposal]").filter({ has: page.locator("button.primary") }).first().locator("button").filter({ has: page.locator("svg.lucide-x") }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    expect(await untranslated(dialog)).toEqual([]);
    expect(await clipped(dialog)).toEqual([]);
    await axe(page);
    await page.keyboard.press("Escape");

    // Preferences › Agents: the approvals rows.
    await page.locator("nav button").filter({ has: page.locator("svg.lucide-sliders-horizontal") }).click();
    const preferences = page.getByRole("dialog");
    await preferences.locator('[role=tab][data-key="agents"]').click();
    const rows = preferences.locator("[data-approval-rows]");
    await rows.scrollIntoViewIfNeeded();
    expect(await untranslated(rows)).toEqual([]);
    expect(await clipped(rows)).toEqual([]);
    await axe(page);
  });
});

test.describe("right to left", () => {
  test.use({ locale: "he-IL" });

  test("the cards mirror and nothing is cut off", async ({ page }) => {
    await openApprovals(page);
    await expect(page.locator("html")).toHaveAttribute("dir", "rtl");
    const screen = screenOf(page);
    const card = screen.locator("[data-proposal]").filter({ has: page.locator("button.primary") }).first();
    const box = await card.boundingBox();
    const approve = await card.locator("button.primary").boundingBox();
    const title = await card.locator("span").first().boundingBox();
    expect(box && approve && title).toBeTruthy();
    if (box && approve && title) {
      // The buttons at the inline end (the left), the title at the start (the right).
      expect(approve.x - box.x).toBeLessThan(box.x + box.width - (approve.x + approve.width));
      expect(box.x + box.width - (title.x + title.width)).toBeLessThan(title.x - box.x);
    }
    expect(await clipped(screen)).toEqual([]);
    expect(await clipped(approvalsButton(page))).toEqual([]);
    await axe(page);
  });
});
