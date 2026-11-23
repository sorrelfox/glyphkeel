// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test } from "@playwright/test";

test("the window shows its header bar, sidebar and status bar", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("banner")).toContainText("Today");
  await expect(page.getByRole("button", { name: "Close" })).toBeVisible();
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeVisible();
  await expect(page.getByRole("contentinfo")).toContainText("No tasks");
  await expect(page).toHaveTitle(/Glyphkeel: .*Today/);
});

test("the web page's own habits are switched off", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("banner")).toBeVisible();
  const prevented = await page.evaluate(() => {
    const fire = (event: Event, target: EventTarget = document.body) => {
      target.dispatchEvent(event);
      return event.defaultPrevented;
    };
    return {
      contextMenu: fire(new MouseEvent("contextmenu", { bubbles: true, cancelable: true })),
      reload: fire(new KeyboardEvent("keydown", { key: "F5", bubbles: true, cancelable: true })),
      zoomKey: fire(new KeyboardEvent("keydown", { key: "+", ctrlKey: true, bubbles: true, cancelable: true })),
      zoomWheel: fire(new WheelEvent("wheel", { ctrlKey: true, bubbles: true, cancelable: true })),
      caretBrowsing: fire(new KeyboardEvent("keydown", { key: "F7", bubbles: true, cancelable: true })),
      back: fire(new KeyboardEvent("keydown", { key: "ArrowLeft", altKey: true, bubbles: true, cancelable: true })),
    };
  });
  expect(prevented).toEqual({ contextMenu: true, reload: true, zoomKey: true, zoomWheel: true, caretBrowsing: true, back: true });
  const styles = await page.evaluate(() => {
    const root = getComputedStyle(document.documentElement);
    const button = getComputedStyle(document.querySelector("button") ?? document.body);
    return { overscroll: root.overscrollBehaviorY, userSelect: button.userSelect, cursor: button.cursor, spellcheck: document.documentElement.spellcheck };
  });
  expect(styles).toEqual({ overscroll: "none", userSelect: "none", cursor: "default", spellcheck: false });
});
