// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

// The sprints page, the project's automation rules, the dashboards, the
// board's colour rules and its options for colours and covers, the
// Security page, the lock screen and the local HTTP endpoint for agents, on
// the sample data:
//
// - in the pseudo-locale, no English is left in their text, labels,
//   placeholders and titles, apart from the sample data's own words;
// - right to left, every control's box is the mirror image of where it is
//   left to right: the same screen opens once in English and once in
//   Hebrew (with English text), and the page sets `dir` from the locale.
//
// Navigation goes by structure (icons, the keys of tabs and menu items),
// never by the interface's own words, so the same steps work in every
// locale.

const TODAY = "2026-11-11";

/** Accented letters inside brackets: the pseudo-locale's mark on a message. */
const PSEUDO_TEXT = /\[[^\]]*[áéíóúýƀçđĝĥĵķĺñþŕšţṽŵẋž]/u;

interface Screen {
  name: string;
  /** Opens the screen and returns its container. */
  open: (page: Page) => Promise<Locator>;
  /** Parts that show the sample data's own words, left out of the check. */
  data: string[];
  /** The sample data's own words where they share an element with a message. */
  words?: string[];
}

async function start(page: Page): Promise<void> {
  await page.clock.setFixedTime(new Date(`${TODAY}T10:00:00`));
  await page.goto("/");
  await expect(page.locator("nav").first()).toBeVisible();
}

async function openProject(page: Page): Promise<void> {
  await start(page);
  await page.getByRole("row", { name: /Task Manager/ }).first().click();
  await expect(page.locator("section[data-lane]").first()).toBeVisible();
}

/** A page of Preferences, by its key. */
function preferences(key: string): Screen["open"] {
  return async (page) => {
    await start(page);
    await page.keyboard.press("Control+Comma");
    const dialog = page.getByRole("dialog");
    await dialog.locator(`[role=tab][data-key="${key}"]`).click();
    const panel = dialog.getByRole("tabpanel");
    await expect(panel).toBeVisible();
    return panel;
  };
}

/** The board's options menu: the toolbar's menu button with an icon only (the columns' menus are flat). */
async function boardOptions(page: Page): Promise<void> {
  await page.locator("button.btn.flat.icon[aria-haspopup]:not(section[data-lane] *)").last().click();
  await expect(page.getByRole("menu")).toBeVisible();
}

const SCREENS: Screen[] = [
  {
    name: "sprints page",
    open: async (page) => {
      await openProject(page);
      await page.locator(".seg[role=group] button").filter({ has: page.locator("svg.lucide-iteration-cw") }).click();
      const region = page.locator("section[class*=screen]").filter({ has: page.locator("[data-chart]") });
      await expect(region).toBeVisible();
      return region;
    },
    // The sprints' names and goals, and the tasks' keys, titles, types and
    // statuses; the dates, which the locale formats (the pseudo-locale has
    // no month names of its own).
    data: ["[role=row]", "article h2", "article h3", "article [class*=goal]", "[data-chart] text", "[class*=dates]"],
  },
  {
    name: "automation tab",
    open: async (page) => {
      await openProject(page);
      await page.locator("button[class*=projectButton]").first().click();
      await page.getByRole("menuitem").first().click();
      const dialog = page.getByRole("dialog");
      await dialog.locator('[role=tab][data-key="automation"]').click();
      await expect(dialog.locator('[role=tab][data-key="automation"]')).toHaveAttribute("aria-selected", "true");
      // A new rule's form, with its condition and first action.
      await dialog.getByRole("tabpanel").locator("button.btn").first().click();
      await expect(dialog.getByRole("form")).toBeVisible();
      return dialog;
    },
    data: [],
    // The new rule's first action adds the project's first label.
    words: ["board"],
  },
  {
    name: "dashboards",
    open: async (page) => {
      await start(page);
      await page.locator("nav button").filter({ has: page.locator("svg.lucide-layout-dashboard") }).click();
      await page.locator("main section button.btn.primary").first().click();
      const widgets = page.locator("[data-widget]");
      await expect(widgets).toHaveCount(6);
      return page.locator("main");
    },
    // What the widgets show: task titles, status names, dates and the
    // activity's words. The starter's first widget has a title of its own,
    // stored with the dashboard in the language it was made in.
    data: ["[data-widget] li", "[data-widget] svg text", "[data-widget] td", "[data-widget] th", "[data-widget] [class*=bar]", '[data-widget="query_count"] h2'],
  },
  {
    name: "board options for colours and covers",
    open: async (page) => {
      await openProject(page);
      await boardOptions(page);
      return page.getByRole("menu");
    },
    // The card fields named after custom fields are the project's own.
    data: ['[data-key^="field-custom"]'],
  },
  {
    name: "card colour rules dialog",
    open: async (page) => {
      await openProject(page);
      await boardOptions(page);
      await page.locator('[role=menuitem][data-key="colour-rules"]').click();
      const dialog = page.getByRole("dialog");
      await expect(dialog).toBeVisible();
      // Without rules it opens with an empty one, so its fields show.
      await expect(dialog.locator("[data-rule]")).toHaveCount(1);
      return dialog;
    },
    data: [],
  },
  { name: "security page", open: preferences("security"), data: [] },
  {
    name: "lock screen",
    open: async (page) => {
      const panel = await preferences("security")(page);
      // The lock's passphrase is set with the button before "Lock now", the page's last.
      const buttons = panel.locator("button.btn");
      await buttons.nth((await buttons.count()) - 2).click();
      const form = panel.locator("form");
      await form.locator("input[type=password]").nth(0).fill("orchard lantern");
      await form.locator("input[type=password]").nth(1).fill("orchard lantern");
      await form.locator("button[type=submit]").click();
      await expect(form).toHaveCount(0);
      await expect(buttons.last()).toBeEnabled();
      await buttons.last().click();
      const screen = page.locator("main").filter({ has: page.locator("form h1") });
      await expect(screen.locator("input[type=password]")).toBeVisible();
      return screen;
    },
    data: [],
  },
  {
    name: "agents page with the HTTP endpoint on",
    open: async (page) => {
      const panel = await preferences("agents")(page);
      // The endpoint's group is the one with the port.
      // The endpoint's switch is the one just before its port.
      const serve = panel.getByRole("spinbutton").first().locator("xpath=preceding::*[@role='switch'][1]");
      await serve.locator("xpath=ancestor::label[1]").click();
      await expect(panel.getByTestId("agent-http-url")).toBeVisible();
      return panel;
    },
    // The address, the token, the folders and the commands are not words to
    // translate, and the client chosen for the set-up is a product's name.
    data: ["code", "pre"],
    words: ["Claude Code"],
  },
];

/** Text in the container, its labels, placeholders and titles that isn't from a message file. */
function english(container: Locator, data: string[], words: string[]): Promise<string[]> {
  return container.evaluate(
    (root, [data, words]) => {
    // Messages are bracketed; placeables are isolated. What is left over
    // and still has a word in it came from somewhere else.
    const leftover = (text: string) => /[A-Za-z]{3,}/.test(words.reduce((rest, word) => rest.split(word).join(" "), text.replace(/\[[^\]]*\]/g, " ").replace(/⁨[^⁨⁩]*⁩/g, " ")));
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
    },
    [data, words] as const,
  );
}

interface Box {
  name: string;
  start: number;
  end: number;
}

/** The boxes of the container's controls, from its two edges. */
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
    const elements = [...root.querySelectorAll("button, input, textarea, h1, h2, h3, h4, label, legend, th, p, [role=tab], [role=checkbox], [role=switch], [role=progressbar], [role=menuitem], [role=menuitemradio], [role=menuitemcheckbox]")].filter(shown);
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
      expect(await english(container, screen.data, screen.words ?? [])).toEqual([]);
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
