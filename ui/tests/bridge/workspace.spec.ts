// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { deflateSync } from "node:zlib";

import { expect, test, type Page } from "@playwright/test";

// This file's own workspace, which no other file touches: the flows below
// check the whole workspace, starting with an empty Undo stack (see
// playwright.bridge.config.ts).
const token = process.env.GK_BRIDGE_TOKEN ?? "";
const BRIDGE = process.env.GK_BRIDGE_FRESH_PORT ?? "5322";
const PAGE = `/?bridge=${BRIDGE}&token=${token}`;

async function openPreferences(page: Page) {
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /Preferences/ }).click();
  await expect(page.getByRole("dialog", { name: "Preferences" })).toBeVisible();
  await page.getByRole("dialog", { name: "Preferences" }).getByRole("tab", { name: "Time" }).click();
}

function day(page: Page, name: string) {
  return page.getByRole("radiogroup", { name: "First day of the week" }).getByRole("radio", { name });
}

test("two windows work on one workspace and follow each other's changes", async ({ browser }) => {
  const a = await browser.newPage();
  const b = await browser.newPage();
  await a.goto(PAGE);
  await b.goto(PAGE);

  // Data from the engine: the workspace's folder name.
  await expect(a.getByTestId("status-workspace")).toHaveText("Personal");
  await expect(b.getByTestId("status-workspace")).toHaveText("Personal");

  // An engine error, typed, as a toast.
  await a.keyboard.press("Control+z");
  await expect(a.getByRole("alert")).toContainText("Nothing to undo.");

  await openPreferences(a);
  await openPreferences(b);
  await expect(day(a, "Monday")).toHaveAttribute("aria-checked", "true");

  // B changes a setting; A shows it through the change event alone.
  await day(b, "Sunday").click();
  await expect(day(b, "Sunday")).toHaveAttribute("aria-checked", "true");
  await expect(day(a, "Sunday")).toHaveAttribute("aria-checked", "true");

  // Undo from B's toast takes it back in both.
  await b.getByRole("status").filter({ hasText: "Settings changed" }).getByRole("button", { name: "Undo" }).click();
  await expect(day(a, "Monday")).toHaveAttribute("aria-checked", "true");
  await expect(day(b, "Monday")).toHaveAttribute("aria-checked", "true");
});

test("a page with the wrong token gets no workspace", async ({ page }) => {
  await page.goto(`/?bridge=${BRIDGE}&token=wrong`);
  await expect(page.getByTestId("status-workspace")).toHaveText("Workspace unavailable");
});

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

function lane(page: Page, name: string) {
  return page.locator("section[data-lane]").filter({ has: page.getByRole("heading", { name, exact: true }) });
}

// Runs after the flows above: it leaves changes on the Undo stack.
test("a project's board on the engine: create a project and a task, drag the task, Undo", async ({ page }) => {
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");

  // A new project opens on its board, with the first column's title field ready.
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "New project" }).click();
  const dialog = page.getByRole("dialog", { name: "New project" });
  await dialog.getByRole("textbox", { name: "Name" }).fill("Garden shed");
  await expect(dialog.getByRole("textbox", { name: "Key" })).toHaveValue(/^[A-Z][A-Z0-9]{1,9}$/);
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  const names = page.locator("section[data-lane] h2");
  await expect(names).toHaveText(["Backlog", "To do", "In progress", "Review", "Done"]);

  await page.keyboard.type("Buy timber");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  const card = page.locator("section[data-lane] [data-card]", { hasText: "Buy timber" });
  await expect(lane(page, "Backlog").locator("[data-card]")).toHaveCount(1);
  const key = (await card.getAttribute("data-card")) ?? "";
  expect(key).toMatch(/^[A-Z][A-Z0-9]+-1$/);

  // Drag it into the empty In progress column.
  const from = await card.boundingBox();
  const target = await lane(page, "In progress").boundingBox();
  if (!from || !target) {
    throw new Error("the card or the target column isn't on screen");
  }
  await page.mouse.move(from.x + from.width / 2, from.y + 12);
  await page.mouse.down();
  await page.mouse.move(from.x + from.width / 2 + 10, from.y + 20, { steps: 4 });
  await page.mouse.move(target.x + target.width / 2, target.y + 120, { steps: 20 });
  await page.mouse.up();
  await expect(lane(page, "In progress").locator(`[data-card="${key}"]`)).toBeVisible();
  await expect.poll(async () => plain(await page.getByTestId("status-note").innerText())).toBe(`${key} moved to In progress`);

  // The engine has it: a fresh page shows the card where it was dropped.
  await page.reload();
  await page.getByRole("row", { name: /Garden shed/ }).click();
  await expect(lane(page, "In progress").locator(`[data-card="${key}"]`)).toBeVisible();

  // Undo takes the move back, through the engine's change event.
  await page.locator(`[data-card="${key}"]`).click();
  await page.keyboard.press("Control+z");
  await expect(lane(page, "Backlog").locator(`[data-card="${key}"]`)).toBeVisible();
  await expect(lane(page, "In progress").locator("[data-card]")).toHaveCount(0);

  // The personal layout: hide a column, then reset it.
  await lane(page, "Review").getByRole("button", { name: /column menu/ }).click();
  await page.getByRole("menuitem", { name: "Hide" }).click();
  await expect(names).toHaveText(["Backlog", "To do", "In progress", "Done"]);
  await page.getByRole("button", { name: /hidden column/ }).click();
  await page.getByRole("menuitem", { name: "Reset to project layout" }).click();
  await expect(names).toHaveText(["Backlog", "To do", "In progress", "Review", "Done"]);

  // The task panel edits through the engine.
  await page.locator(`[data-card="${key}"]`).dblclick();
  const panel = page.getByRole("complementary", { name: /^Task / });
  await expect(panel).toHaveAccessibleName(new RegExp(key));
  await panel.getByRole("button", { name: /^Priority:/ }).click();
  await page.getByRole("menuitemradio", { name: "High" }).click();
  await expect(panel.getByRole("button", { name: /^Priority:/ })).toHaveAccessibleName(/High/);
  await panel.getByRole("button", { name: /Estimate\u2069?:/ }).click();
  await page.keyboard.type("1:30");
  await page.keyboard.press("Enter");
  await expect(panel.getByRole("button", { name: /Estimate\u2069?:/ })).toHaveAccessibleName(/1h 30m/);
  await panel.getByRole("button", { name: "Close panel" }).click();

  // Project settings: a WIP limit shows on the column, a new status becomes a column.
  await page.getByRole("button", { name: /project menu/ }).click();
  await page.getByRole("menuitem", { name: /Project settings/ }).click();
  const settings = page.getByRole("dialog");
  await settings.getByRole("tab", { name: "Statuses" }).click();
  const limit = settings.getByTestId("status-In progress").getByRole("textbox", { name: "WIP limit" });
  await limit.fill("2");
  await limit.press("Tab");
  await expect.poll(async () => plain(await lane(page, "In progress").getByTestId("lane-count").innerText())).toBe("0 / 2");
  const newStatus = settings.getByRole("textbox", { name: "New status" });
  await newStatus.fill("Waiting");
  await newStatus.press("Enter");
  await expect(names).toHaveText(["Backlog", "To do", "In progress", "Review", "Done", "Waiting"]);
});

// A tiny PNG: one transparent pixel.
const PNG = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==", "base64");

test("task details on the engine: a subtask, a link, a comment and an attachment", async ({ page }) => {
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "New project" }).click();
  const dialog = page.getByRole("dialog", { name: "New project" });
  await dialog.getByRole("textbox", { name: "Name" }).fill("Kitchen");
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  await page.keyboard.type("Paint the walls");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Buy paint");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  const cards = lane(page, "Backlog").locator("[data-card]");
  await expect(cards).toHaveCount(2);
  const walls = (await cards.filter({ hasText: "Paint the walls" }).getAttribute("data-card")) ?? "";
  const paint = (await cards.filter({ hasText: "Buy paint" }).getAttribute("data-card")) ?? "";

  await page.locator(`[data-card="${walls}"]`).dblclick();
  const panel = page.getByRole("complementary", { name: /^Task / });
  await expect(panel).toHaveAccessibleName(new RegExp(walls));

  // A subtask, closed from its checkbox.
  const subtasks = panel.getByRole("region", { name: "Subtasks" });
  await subtasks.getByRole("button", { name: "Add subtask" }).click();
  await subtasks.getByRole("textbox", { name: "New subtask" }).fill("Tape the edges");
  await subtasks.getByRole("textbox", { name: "New subtask" }).press("Enter");
  const box = subtasks.getByRole("checkbox", { name: /^Close / });
  await expect(box).toBeVisible();
  await box.click();
  await expect(subtasks.getByRole("checkbox", { name: /^Reopen / })).toBeChecked();
  await expect(subtasks.getByRole("img", { name: /subtasks done/ })).toBeVisible();
  await expect(subtasks).toContainText("1/1");

  // A second, open subtask; the closed one moves past it with Alt+Down, and back by dragging.
  await subtasks.getByRole("button", { name: "Add subtask" }).click();
  await subtasks.getByRole("textbox", { name: "New subtask" }).fill("Sand the walls");
  await subtasks.getByRole("textbox", { name: "New subtask" }).press("Enter");
  await subtasks.getByRole("textbox", { name: "New subtask" }).press("Escape");
  const rows = subtasks.locator("[data-subtask]");
  await expect(rows).toHaveCount(2);
  const tape = (await rows.filter({ hasText: "Tape the edges" }).getAttribute("data-subtask")) ?? "";
  const sand = (await rows.filter({ hasText: "Sand the walls" }).getAttribute("data-subtask")) ?? "";
  const order = () => rows.evaluateAll((all) => all.map((row) => row.getAttribute("data-subtask")));
  await expect.poll(order).toEqual([tape, sand]);
  await subtasks.getByRole("button", { name: new RegExp(`^\\W*Open \\W*${tape}`) }).press("Alt+ArrowDown");
  await expect.poll(order).toEqual([sand, tape]);
  await expect(subtasks.getByRole("button", { name: new RegExp(`^\\W*Open \\W*${tape}`) })).toBeFocused();
  const grip = subtasks.locator(`[data-subtask-grip="${tape}"]`);
  await rows.filter({ hasText: "Tape the edges" }).hover();
  const from = await grip.boundingBox();
  const to = await rows.filter({ hasText: "Sand the walls" }).boundingBox();
  if (!from || !to) {
    throw new Error("the subtask rows aren't laid out");
  }
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(from.x + from.width / 2, to.y + 2, { steps: 12 });
  await page.mouse.up();
  await expect.poll(order).toEqual([tape, sand]);
  // dnd-kit swallows clicks for 50 ms after a drop, against the click a drop can cause; a person is slower.
  await page.waitForTimeout(100);

  // A link: the walls wait for the paint; both cards and the panel say so.
  const links = panel.getByRole("region", { name: "Links" });
  await links.getByRole("button", { name: "Add link" }).click();
  await links.getByRole("button", { name: /Kind of link/ }).click();
  await page.getByRole("listbox").getByRole("option", { name: "Blocked by" }).click();
  await links.getByRole("textbox", { name: "Task key" }).fill(paint);
  await links.getByRole("textbox", { name: "Task key" }).press("Enter");
  await expect(links.getByRole("button", { name: new RegExp(`^\\W*Blocked by.*${paint}.*Buy paint`) })).toBeVisible();
  await expect(panel.getByRole("note", { name: new RegExp(`Blocked by.*${paint}`) })).toBeVisible();
  await expect(page.locator(`[data-card="${walls}"]`).getByText("Blocked")).toBeVisible();

  // A comment, sent with Ctrl+Enter, then edited: the earlier text stays viewable.
  const composer = panel.getByRole("textbox", { name: "Write a comment" });
  await composer.fill("Two coats should do.");
  await composer.press("Control+Enter");
  const comment = panel.getByRole("article", { name: /Comment by/ });
  await expect(comment).toContainText("Two coats should do.");
  await comment.getByRole("button", { name: "Comment" }).click();
  await page.getByRole("menuitem", { name: "Edit" }).click();
  await comment.getByRole("textbox", { name: "Comment text" }).fill("Three coats, to be safe.");
  await comment.getByRole("textbox", { name: "Comment text" }).press("Control+Enter");
  await comment.getByRole("button", { name: /edited/ }).click();
  await expect(comment.getByRole("list", { name: "Earlier versions" })).toContainText("Two coats should do.");

  // An attachment from the file field; the image shows from its content.
  const files = panel.getByRole("region", { name: "Files" });
  await files.locator("input[type=file]").setInputFiles({ name: "swatch.png", mimeType: "image/png", buffer: PNG });
  await expect(files.getByRole("img", { name: "swatch.png" })).toBeVisible();
  await expect(files.getByText("swatch.png")).toBeVisible();
  // A wide image gets the engine's thumbnail, 256 pixels on its longer side.
  await files.locator("input[type=file]").setInputFiles({ name: "banner.png", mimeType: "image/png", buffer: solidPng(512, 128) });
  await expect(files.getByRole("img", { name: "banner.png" })).toHaveAttribute("data-thumbnail", "256x64");

  // The card counts the comment and the file.
  const card = page.locator(`[data-card="${walls}"]`);
  await expect(card.getByLabel(/1 comment/)).toBeVisible();
  await expect(card.getByLabel(/2\W*files/)).toBeVisible();

  // History lists what happened to the task.
  await panel.getByRole("tab", { name: /History/ }).click();
  const history = panel.getByRole("list", { name: "History" });
  await expect(history).toContainText("Comment edited");
  await expect(history).toContainText("File attached");
  await expect(history).toContainText("Link added");
  // The engine keeps it to this task: the other task's creation isn't there.
  await expect(history).toContainText(new RegExp(`Created\\W*${walls}`));
  await expect(history).not.toContainText(new RegExp(`Created\\W*${paint}`));

  // The subtask's own history tells its moves in words.
  await subtasks.getByRole("button", { name: new RegExp(`^\\W*Open \\W*${tape}`) }).click();
  await expect(panel).toHaveAccessibleName(new RegExp(tape));
  await panel.getByRole("tab", { name: /History/ }).click();
  await expect(panel.getByRole("list", { name: "History" })).toContainText("Moved among its parent's subtasks");
});

/** A PNG of one colour, `width` by `height` pixels. */
function solidPng(width: number, height: number): Buffer {
  const crcTable = Array.from({ length: 256 }, (_, n) => {
    let c = n;
    for (let k = 0; k < 8; k += 1) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    return c >>> 0;
  });
  const crc = (bytes: Buffer) => {
    let c = 0xffffffff;
    for (const byte of bytes) {
      c = (crcTable[(c ^ byte) & 0xff] ?? 0) ^ (c >>> 8);
    }
    return (c ^ 0xffffffff) >>> 0;
  };
  const chunk = (type: string, data: Buffer) => {
    const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
    const length = Buffer.alloc(4);
    length.writeUInt32BE(data.length);
    const sum = Buffer.alloc(4);
    sum.writeUInt32BE(crc(body));
    return Buffer.concat([length, body, sum]);
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header.writeUInt8(8, 8); // 8 bits per sample
  header.writeUInt8(2, 9); // RGB
  const row = Buffer.concat([Buffer.from([0]), Buffer.alloc(width * 3, 0x5a)]);
  const pixels = deflateSync(Buffer.concat(Array.from({ length: height }, () => row)));
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", header), chunk("IDAT", pixels), chunk("IEND", Buffer.alloc(0))]);
}

const BEDS = [
  "Plan:",
  "",
  "- [ ] Dig the beds",
  "- [ ] Sow the seeds",
  "",
  "| Bed | Crop |",
  "|-----|------|",
  "| 1 | Beans |",
  "",
  "Area: $w \\times l$",
  "",
  "```rust",
  "fn main() {}",
  "```",
  "",
  "```mermaid",
  "graph LR; Dig-->Sow",
  "```",
  "",
].join("\n");

test("descriptions and comments in Markdown on the engine: edit, tick, a conflict copy and a comment", async ({ browser }) => {
  const a = await browser.newPage();
  const b = await browser.newPage();
  await a.goto(PAGE);
  await expect(a.getByTestId("status-workspace")).toHaveText("Personal");
  await a.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "New project" }).click();
  const dialog = a.getByRole("dialog", { name: "New project" });
  await dialog.getByRole("textbox", { name: "Name" }).fill("Allotment");
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(a.getByRole("group", { name: /Board of/ })).toBeVisible();
  await a.keyboard.type("Plan the beds");
  await a.keyboard.press("Enter");
  await a.keyboard.type("Buy seeds");
  await a.keyboard.press("Enter");
  await a.keyboard.press("Escape");
  const cards = lane(a, "Backlog").locator("[data-card]");
  await expect(cards).toHaveCount(2);
  const beds = (await cards.filter({ hasText: "Plan the beds" }).getAttribute("data-card")) ?? "";
  const seeds = (await cards.filter({ hasText: "Buy seeds" }).getAttribute("data-card")) ?? "";
  await a.locator(`[data-card="${beds}"]`).dblclick();
  // Fluent isolates the key with invisible marks.
  const panel = a.getByRole("complementary", { name: new RegExp(`^Task \\W*${beds}\\W*$`) });
  const description = panel.getByRole("region", { name: "Description" });

  // Edit the description in the Markdown editor and save it.
  await description.getByRole("button", { name: "Edit" }).click();
  const field = description.getByRole("textbox", { name: "Description" });
  await field.fill(BEDS);
  await field.press("Control+Enter");
  await expect(description.getByRole("checkbox", { name: "Dig the beds" })).toBeVisible();
  // Drawn in the page: the table from the engine, KaTeX, Shiki and Mermaid.
  await expect(description.locator("table td").first()).toHaveText("1");
  await expect(description.locator(".katex")).toBeVisible();
  await expect(description.locator("pre.shiki")).toContainText("fn main() {}");
  await expect(description.locator(".gk-diagram svg")).toBeVisible();

  // Tick a checkbox in the reader: the engine changes the text, and the card counts it.
  await description.getByRole("checkbox", { name: "Sow the seeds" }).click();
  await expect(description.getByRole("checkbox", { name: "Sow the seeds" })).toBeChecked();
  await expect(a.locator(`[data-card="${beds}"]`)).toContainText("1/2");

  // A second window opens the same task.
  await b.goto(PAGE);
  await b.getByRole("navigation", { name: "Workspace" }).getByRole("row", { name: /Allotment/ }).click();
  await b.locator(`[data-card="${beds}"]`).dblclick();
  const otherPanel = b.getByRole("complementary", { name: new RegExp(`^Task \\W*${beds}\\W*$`) });
  const otherDescription = otherPanel.getByRole("region", { name: "Description" });
  await expect(otherDescription.getByRole("checkbox", { name: "Sow the seeds" })).toBeChecked();

  // Both edit the first line; the second window saves first, then ticks another item.
  await description.getByRole("button", { name: "Edit" }).click();
  const editor = description.getByRole("textbox", { name: "Description" });
  await expect(editor).toBeVisible();
  await otherDescription.getByRole("button", { name: "Edit" }).click();
  const ticked = BEDS.replace("- [ ] Sow", "- [x] Sow");
  await otherDescription.getByRole("textbox", { name: "Description" }).fill(ticked.replace("Plan:", "Plan for May:"));
  await otherDescription.getByRole("button", { name: "Save" }).click();
  await expect(otherDescription.getByText("Plan for May:")).toBeVisible();
  await otherDescription.getByRole("checkbox", { name: "Dig the beds" }).click();
  await expect(otherDescription.getByRole("checkbox", { name: "Dig the beds" })).toBeChecked();
  // Both changes have reached the first window before anything is typed in its editor,
  // which keeps the text and revision it opened with.
  await expect(a.locator(`[data-card="${beds}"]`)).toContainText("2/2");
  await expect(editor).not.toContainText("Plan for May:");
  const both = ticked.replace("- [ ] Dig", "- [x] Dig");
  await editor.fill(both.replace("Plan:", "Plan for spring:"));
  await description.getByRole("button", { name: "Save" }).click();

  // The stored text stays as the other window left it; this edit is a conflict copy, shown in both windows.
  await expect(a.getByRole("alert").filter({ hasText: "Your edit clashed with another change." })).toBeVisible();
  const banner = description.getByRole("alert").filter({ hasText: "conflict copy" });
  await expect(banner).toContainText("kept as a conflict copy");
  await expect(description.getByText("Plan for May:")).toBeVisible();
  await expect(otherDescription.getByRole("alert")).toContainText("kept as a conflict copy");

  // Resolve it: the differences, then a merge by hand.
  await banner.getByRole("button", { name: "Resolve" }).click();
  const resolver = description.getByRole("group", { name: "Conflict copy" });
  const diff = resolver.getByRole("group", { name: "Differences between the description and the copy" });
  await expect(diff.locator("del")).toHaveText("Plan for May:");
  await expect(diff.locator("ins")).toHaveText("Plan for spring:");
  await resolver.getByRole("button", { name: "Merge by hand" }).click();
  const merge = description.getByRole("textbox", { name: "Description" });
  await merge.fill(both.replace("Plan:", "Plan for spring, sown in May:"));
  await merge.press("Control+Enter");
  await expect(description.getByText("Plan for spring, sown in May:")).toBeVisible();
  await expect(description.getByRole("alert")).toHaveCount(0);
  await expect(otherDescription.getByText("Plan for spring, sown in May:")).toBeVisible();
  await expect(otherDescription.getByRole("alert")).toHaveCount(0);

  // The description's history lists the revisions, the conflict copy among them.
  await description.getByRole("button", { name: "History" }).click();
  const revisions = description.getByRole("list", { name: "History" });
  await expect(revisions.getByRole("listitem").filter({ hasText: "Conflict copy" })).toHaveCount(1);
  await expect(revisions.getByRole("listitem").filter({ hasText: "Resolved" })).toHaveCount(1);
  await description.getByRole("button", { name: "Close" }).click();

  // A comment in Markdown, with a task key that links to the other task.
  const composer = panel.getByRole("textbox", { name: "Write a comment" });
  await composer.fill(`Use **compost** from \`bin 2\` and see ${seeds}.`);
  await composer.press("Control+Enter");
  const comment = panel.getByRole("article", { name: /Comment by/ });
  await expect(comment.locator("strong")).toHaveText("compost");
  await expect(comment.locator("code")).toHaveText("bin 2");
  await comment.getByRole("link", { name: seeds }).click();
  await expect(a.getByRole("complementary", { name: new RegExp(`^Task \\W*${seeds}\\W*$`) }).getByRole("heading", { name: "Buy seeds" })).toBeVisible();

  await a.close();
  await b.close();
});

test("time on the engine: log, edit and delete time, run the timer in two windows, set a reminder", async ({ browser }) => {
  test.setTimeout(60_000);
  const page = await browser.newPage();
  const other = await browser.newPage();
  await page.goto(PAGE);
  await other.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "New project" }).click();
  const dialog = page.getByRole("dialog", { name: "New project" });
  await dialog.getByRole("textbox", { name: "Name" }).fill("Attic");
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  await page.keyboard.type("Insulate the roof");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  const card = lane(page, "Backlog").locator("[data-card]", { hasText: "Insulate the roof" });
  const key = (await card.getAttribute("data-card")) ?? "";
  await card.dblclick();
  const panel = page.getByRole("complementary", { name: new RegExp(`^\\W*Task \\W*${key}`) });
  await expect(panel.getByRole("heading", { name: "Insulate the roof" })).toBeVisible();

  // Time entries: one typed with a decimal comma, then "+30m".
  await panel.getByRole("tab", { name: "Time" }).click();
  await panel.getByRole("button", { name: "Log time" }).click();
  const form = panel.getByRole("group", { name: "Log time" });
  await form.getByRole("textbox", { name: "Duration" }).fill("1,5h");
  await form.getByRole("textbox", { name: "Note" }).fill("Measured the rafters");
  await form.getByRole("textbox", { name: "Duration" }).press("Enter");
  const entries = panel.getByRole("list", { name: "Time entries" });
  await expect(entries.getByRole("listitem")).toHaveCount(1);
  await expect(entries).toContainText("1h 30m");
  await panel.getByRole("button", { name: /log 30 minutes for today$/ }).click();
  await expect(entries.getByRole("listitem")).toHaveCount(2);
  await expect(panel.locator("[data-time-total]")).toHaveText("2h");
  // The card shows the time spent.
  await expect(card.getByLabel(/2h.*spent/)).toBeVisible();

  // Edit an entry to a day's work in the workspace's units, then delete the other.
  const rafters = entries.getByRole("listitem").filter({ hasText: "Measured the rafters" });
  await rafters.getByRole("button", { name: "Time entry actions" }).click();
  await page.getByRole("menuitem", { name: "Edit" }).click();
  await panel.getByRole("textbox", { name: "Duration" }).fill("1d");
  await panel.getByRole("textbox", { name: "Duration" }).press("Enter");
  await expect(panel.locator("[data-time-total]")).toHaveText("1d 30m");
  await entries.getByRole("listitem").filter({ hasNotText: "Measured the rafters" }).getByRole("button", { name: "Time entry actions" }).click();
  await page.getByRole("menuitem", { name: "Delete" }).click();
  await expect(entries.getByRole("listitem")).toHaveCount(1);
  await expect(panel.locator("[data-time-total]")).toHaveText("1d");

  // The timer: started here, it shows in the other window's sidebar, which stops it.
  await panel.getByRole("button", { name: "Start timer" }).click();
  await expect(panel.locator("[data-timer-clock]")).toHaveText(/^0:00:0\d$/);
  const chip = other.getByRole("group", { name: "Timer" });
  await expect(chip).toHaveAttribute("data-timer-chip", key);
  await expect(page.getByRole("group", { name: "Timer" })).toBeVisible();
  await chip.getByRole("button", { name: "Pause the timer" }).click();
  await expect(panel.getByRole("button", { name: /^Timer paused at/ })).toBeVisible();
  await chip.getByRole("button", { name: "Stop the timer and log the time" }).click();
  await expect(other.getByText(/no time was logged/)).toBeVisible();
  await expect(page.getByRole("group", { name: "Timer" })).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "Start timer" })).toBeVisible();

  // A reminder some days before the due date, which the engine places on the calendar.
  await panel.getByRole("button", { name: /^\W*Due\W*: / }).click();
  await panel.getByLabel("Due", { exact: true }).fill("2030-06-15");
  await panel.getByLabel("Due", { exact: true }).press("Enter");
  await expect(panel.getByRole("button", { name: /^\W*Due\W*: \W*Jun 15/ })).toBeVisible();
  await panel.getByRole("button", { name: /^\W*Reminder\W*: \W*None/ }).click();
  const reminder = page.getByRole("dialog", { name: "Reminder" });
  await reminder.getByRole("radio", { name: "Before the due date" }).click();
  await reminder.getByRole("spinbutton", { name: "Days before the due date" }).fill("2");
  await reminder.getByRole("button", { name: "Set reminder" }).click();
  await expect(panel.getByRole("button", { name: /^\W*Reminder\W*: .*Jun 13, 2030.*2\W* days before due/ })).toBeVisible();

  // The time report adds up the week.
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "Time report" }).click();
  await expect(page.locator(`[data-report-row="${key}"]`)).toContainText("1d");
  await page.getByRole("radio", { name: "Project" }).click();
  await expect(page.getByRole("columnheader", { name: "Project" })).toBeVisible();

  await page.close();
  await other.close();
});

// Runs last: it changes the working day for the rest of the workspace.
test("working time and the timer on the engine: hours per day re-format durations, T starts and stops the timer", async ({ page }) => {
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: "New project" }).click();
  const dialog = page.getByRole("dialog", { name: "New project" });
  await dialog.getByRole("textbox", { name: "Name" }).fill("Workshop");
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  await page.keyboard.type("Sharpen the chisels");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  const card = page.locator("section[data-lane] [data-card]", { hasText: "Sharpen the chisels" });
  const key = (await card.getAttribute("data-card")) ?? "";

  // Twelve hours of estimate: a day and a half of eight hours.
  await card.dblclick();
  const panel = page.getByRole("complementary", { name: new RegExp(`^Task \\W*${key}\\W*$`) });
  const estimate = panel.getByRole("button", { name: /Estimate\u2069?:/ });
  await estimate.click();
  await page.keyboard.type("12h");
  await page.keyboard.press("Enter");
  await expect(estimate).toHaveAccessibleName(/1d 4h/);

  // Six-hour days: the same estimate is two days.
  await openPreferences(page);
  const preferences = page.getByRole("dialog", { name: "Preferences" });
  const hours = preferences.getByRole("spinbutton", { name: "Working hours per day" });
  await expect(hours).toHaveValue("8");
  await hours.fill("6");
  await hours.press("Enter");
  await expect(page.getByRole("status").filter({ hasText: "Settings changed" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(preferences).toBeHidden();
  await expect(estimate).toHaveAccessibleName(/2d/);

  // T starts the timer on the open task, and T again stops it.
  await panel.getByRole("button", { name: "Close panel" }).click();
  await card.click();
  await page.keyboard.press("t");
  const chip = page.getByRole("group", { name: "Timer" });
  await expect(chip).toHaveAttribute("data-timer-chip", key);
  await page.keyboard.press("t");
  await expect(chip).toBeHidden();
  await expect(page.getByRole("status").filter({ hasText: /no time was logged|logged on/ })).toBeVisible();
});
