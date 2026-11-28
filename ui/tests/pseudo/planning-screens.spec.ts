// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

// The board with swimlanes, My work, the project settings' Fields, Rules
// and Templates tabs, the CSV import dialog, the roadmap and the reports, on
// the sample data:
//
// - in the pseudo-locale, no English is left in their text, labels,
//   placeholders and titles, apart from the sample data's own words;
// - right to left, every control's box is the mirror image of where it is
//   left to right. No language with right-to-left text ships yet, but the
//   page takes its direction from the locale: the same screen opens once in
//   English and once in Hebrew (with English text), and the page itself
//   sets `dir` on the root.
//
// Navigation goes by structure, never by the interface's own words, so the
// same steps work in every locale.

const TODAY = "2026-11-11";

/** Accented letters inside brackets: the pseudo-locale's mark on a message. */
const PSEUDO_TEXT = /\[[^\]]*[áéíóúýƀçđĝĥĵķĺñþŕšţṽŵẋž]/u;

interface Screen {
  name: string;
  /** Opens the screen and returns its container. */
  open: (page: Page) => Promise<Locator>;
  /** Parts that show the sample data's own words, left out of the check. */
  data: string[];
}

async function openProject(page: Page): Promise<void> {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await page.getByRole("row", { name: /Task Manager/ }).first().click();
  await expect(page.locator("section[data-lane]").first()).toBeVisible();
}

/** The project's menu, then one of its items by place: settings, new sub-project, CSV import. */
async function projectMenu(page: Page, item: number): Promise<void> {
  await page.locator("button[class*=projectButton]").first().click();
  await page.getByRole("menuitem").nth(item).click();
}

/** A tab of the project settings by place. */
function settingsTab(tab: number): Screen["open"] {
  return async (page) => {
    await openProject(page);
    await projectMenu(page, 0);
    const dialog = page.getByRole("dialog");
    await dialog.getByRole("tab").nth(tab).click();
    await expect(dialog.getByRole("tab").nth(tab)).toHaveAttribute("aria-selected", "true");
    return dialog;
  };
}

const SCREENS: Screen[] = [
  {
    name: "board with swimlanes",
    open: async (page) => {
      await openProject(page);
      // The toolbar's swimlanes menu, then its third choice: by priority.
      await page.locator("[role=toolbar] button").filter({ has: page.locator("svg.lucide-rows-3") }).click();
      await page.getByRole("menuitemradio").nth(2).click();
      await expect(page.locator("section[data-swimlane]").first()).toBeVisible();
      return page.locator("section[data-swimlane]").first().locator("xpath=..");
    },
    // The cards and the status names are the sample data's own.
    data: ["[data-card]", "[data-column] h2"],
  },
  {
    name: "my work",
    open: async (page) => {
      await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
      await page.goto("/");
      await page.locator("nav button").filter({ has: page.locator("svg.lucide-square-kanban") }).click();
      await expect(page.locator("section[data-swimlane]").first()).toBeVisible();
      return page.locator("main");
    },
    // The cards and the lanes' project names are the sample data's own.
    data: ["[data-card]", "section[data-swimlane] h3"],
  },
  { name: "fields tab", open: settingsTab(4), data: [] },
  // The status names are the project's own.
  { name: "rules tab", open: settingsTab(5), data: ["[data-testid^='rules-'] > button > span:nth-child(2)"] },
  { name: "templates tab", open: settingsTab(7), data: [] },
  {
    name: "CSV import dialog",
    open: async (page) => {
      await openProject(page);
      await projectMenu(page, 2);
      const dialog = page.getByRole("dialog");
      await dialog.getByRole("textbox").first().fill("tasks.csv");
      // The dialog's first button is the file's; "Check" comes next.
      await dialog.getByRole("button").nth(1).click();
      await expect(dialog.getByTestId("csv-summary")).toBeVisible();
      return dialog;
    },
    // The file's column names and examples.
    data: ["td"],
  },
  {
    name: "roadmap",
    open: async (page) => {
      await openProject(page);
      // The roadmap is the fifth of the header's views.
      await page.locator(".seg[role=group] button").nth(4).click();
      await expect(page.getByRole("article").first()).toBeVisible();
      return page.locator("main");
    },
    // The versions' tasks and descriptions.
    data: ["article li", "article p"],
  },
  {
    name: "reports",
    open: async (page) => {
      await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
      await page.goto("/");
      await page.locator("nav button").filter({ has: page.locator("svg.lucide-chart-area") }).click();
      await expect(page.locator("[data-chart]").first()).toBeVisible();
      return page.locator("main");
    },
    // The axis shows dates, formatted by the locale.
    data: ["svg text"],
  },
];

/** Text in the container, its labels, placeholders and titles that isn't from a message file. */
function english(container: Locator, data: string[]): Promise<string[]> {
  return container.evaluate((root, data) => {
    // Messages are bracketed; placeables are isolated. What is left over
    // and still has a word in it came from somewhere else.
    const leftover = (text: string) => /[A-Za-z]{3,}/.test(text.replace(/\[[^\]]*\]/g, " ").replace(/⁨[^⁨⁩]*⁩/g, " "));
    const skipped = (element: Element) => data.some((selector) => element.closest(selector) !== null);
    const hidden = (element: Element) => {
      for (let node: Element | null = element; node && node !== document.body; node = node.parentElement) {
        const style = getComputedStyle(node);
        if (style.display === "none" || style.visibility === "hidden" || style.clipPath === "inset(50%)") {
          return true;
        }
      }
      return false;
    };
    const found = new Set<string>();
    for (const element of root.querySelectorAll("*")) {
      if (skipped(element) || hidden(element)) {
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
  }, data);
}

interface Box {
  name: string;
  start: number;
  end: number;
}

/** The boxes of the container's controls, from its two edges, in the direction the page has. */
function boxes(container: Locator): Promise<Box[]> {
  return container.evaluate((root) => {
    const shown = (element: Element) => {
      const box = element.getBoundingClientRect();
      if (box.width < 2 || box.height < 2) {
        return false;
      }
      for (let node: Element | null = element; node && node !== document.body; node = node.parentElement) {
        if (getComputedStyle(node).clipPath === "inset(50%)") {
          return false;
        }
      }
      return true;
    };
    const elements = [...root.querySelectorAll("button, input, textarea, h1, h2, h3, h4, label, legend, th, p, [role=tab], [role=checkbox], [role=switch], [role=progressbar]")].filter(shown);
    const frame = root.getBoundingClientRect();
    return elements.map((element) => {
      const box = element.getBoundingClientRect();
      const name = element.getAttribute("aria-label") ?? element.textContent;
      return { name: `${element.tagName.toLowerCase()} "${name.trim().slice(0, 40)}"`, start: box.left - frame.left, end: frame.right - box.right };
    });
  });
}

/**
 * Controls whose box doesn't mirror: the distance from the container's left
 * edge, left to right, must be the distance from its right edge, right to left.
 */
function unmirrored(ltr: Box[], rtl: Box[]): string[] {
  if (ltr.length === 0) {
    return ["no controls found"];
  }
  if (ltr.length !== rtl.length) {
    return [`${String(ltr.length)} controls left to right, ${String(rtl.length)} right to left`];
  }
  const off: string[] = [];
  ltr.forEach((a, at) => {
    const b = rtl[at];
    if (b && Math.abs(a.start - b.end) > 4) {
      off.push(`${a.name}: ${a.start.toFixed(0)} from the left, ${b.end.toFixed(0)} from the right`);
    }
  });
  return off;
}

test.describe("in the pseudo-locale", () => {
  test.use({ locale: "en-XA" });

  for (const screen of SCREENS) {
    test(`the ${screen.name} has no text outside the message files`, async ({ page }) => {
      const container = await screen.open(page);
      const text = await container.innerText();
      test.skip(!PSEUDO_TEXT.test(text), "the page doesn't show the pseudo-locale");
      expect(await english(container, screen.data)).toEqual([]);
    });
  }
});

test.describe("right to left", () => {
  for (const screen of SCREENS) {
    test(`the ${screen.name} mirrors`, async ({ browser }, info) => {
      const { baseURL, viewport } = info.project.use;
      const measure = async (locale: string, dir: "ltr" | "rtl") => {
        const context = await browser.newContext({ baseURL, viewport, locale });
        const page = await context.newPage();
        const container = await screen.open(page);
        // The page's own direction, from the locale: nothing here sets it.
        await expect(page.locator("html")).toHaveAttribute("dir", dir);
        const found = await boxes(container);
        await context.close();
        return found;
      };
      const ltr = await measure("en-US", "ltr");
      const rtl = await measure("he-IL", "rtl");
      expect(unmirrored(ltr, rtl)).toEqual([]);
    });
  }
});
