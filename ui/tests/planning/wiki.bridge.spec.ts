// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator } from "@playwright/test";

import { gk, openView, project } from "./engine";

// The wiki on the engine: pages made from the command line show in the
// tree with their links both ways; an edit, a ticked checkbox and a move
// in the app reach the engine; a change from another process shows
// without a reload.

interface Page {
  body: string;
}
interface Listed {
  items: { title: string; depth: number }[];
}

const rows = async (tree: Locator) => (await tree.getByRole("row").allInnerTexts()).map((text) => text.trim().split("\n")[0]);
const titles = () => (gk("wiki", "list", "--project", "DOC") as Listed).items.map((item) => `${"  ".repeat(item.depth)}${item.title}`);
const body = (title: string) => (gk("wiki", "get", title, "--project", "DOC") as Page).body;

test("pages from the command line: links both ways, an edit, a checkbox and a move reach the engine, and outside changes show", async ({ page }) => {
  project("Docs", "DOC");
  gk("wiki", "new", "--project", "DOC", "--title", "Home", "--body=# Home\n\nStart at [[Guide#Install]].\n");
  gk("wiki", "new", "--project", "DOC", "--title", "Guide", "--body=# Guide\n\n## Install\n\n- [ ] download\n- [ ] unpack\n\nBack to [[Home]].\n");
  gk("wiki", "new", "--project", "DOC", "--title", "Archive", "--body=# Archive\n");
  gk("task", "create", "--project", "DOC", "--title", "Write the guide", "--description=Follow [[Guide]].");

  await openView(page, "Docs", "Wiki");
  const tree = page.getByRole("treegrid", { name: "Pages" });
  await expect(tree.getByRole("row")).toHaveCount(3);
  expect(await rows(tree)).toEqual(["Home", "Guide", "Archive"]);

  // The page with what links to it: a page and a task.
  await tree.getByRole("row", { name: "Guide" }).click();
  const view = page.getByRole("region", { name: /Wiki page/ });
  await expect(view.getByRole("heading", { name: "Install" })).toBeVisible();
  await expect(view.getByRole("region", { name: "Linked from" }).getByRole("button", { name: "Home" })).toBeVisible();
  await expect(view.getByRole("region", { name: "Tasks that link here" })).toContainText("DOC-1");
  await expect(view.locator('a[data-wiki="Home"]')).not.toHaveAttribute("data-missing", "");

  // A checkbox ticked in the reader is a new revision in the engine.
  const box = view.locator("article").getByRole("checkbox", { name: /unpack/ });
  await box.click();
  await expect(box).toBeChecked();
  await expect.poll(() => body("Guide")).toContain("- [x] unpack");

  // An edit saved in the app.
  await view.getByRole("heading", { name: "Guide", level: 1 }).click();
  await page.keyboard.press("e");
  const editor = view.getByRole("textbox", { name: /Text of/ });
  await expect(editor).toBeFocused();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("\nEdited in the app.");
  await view.getByRole("button", { name: "Save" }).click();
  await expect(view.getByText("Edited in the app.")).toBeVisible();
  await expect.poll(() => body("Guide")).toContain("Edited in the app.");
  expect(body("Guide")).toContain("- [x] unpack");

  // A move with the keyboard.
  await tree.getByRole("row", { name: "Archive" }).click();
  await page.keyboard.press("Alt+ArrowUp");
  await expect.poll(() => rows(tree)).toEqual(["Home", "Archive", "Guide"]);
  await expect.poll(titles).toEqual(["Home", "Archive", "Guide"]);

  // Changes from another process: a new page in the tree, a new text on
  // the page on screen.
  await tree.getByRole("row", { name: "Home" }).click();
  await expect(view.getByText("Start at")).toBeVisible();
  gk("wiki", "put", "Home", "--project", "DOC", "--edits", JSON.stringify([{ old_text: "Start at", new_text: "Begin at" }]));
  gk("wiki", "new", "--project", "DOC", "--title", "Later");
  await expect(view.getByText("Begin at")).toBeVisible();
  await expect.poll(() => rows(tree)).toEqual(["Home", "Archive", "Guide", "Later"]);
});
