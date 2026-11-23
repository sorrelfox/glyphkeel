// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Page } from "@playwright/test";

/** The text exactly as the editor last reported it. */
async function text(page: Page): Promise<string> {
  return JSON.parse((await page.getByTestId("editor-text").textContent()) ?? '""') as string;
}

async function openSample(page: Page, sample: "sample" | "crlf" | "mixed" | "long") {
  await page.goto("/dev/editor");
  await page.getByRole("radio", { name: sample, exact: true }).click();
  await expect(page.locator(".cm-content")).toBeVisible();
}

/** Puts the cursor at the end of the line that contains `snippet`. */
async function cursorAfter(page: Page, snippet: string) {
  await page.locator(".cm-line", { hasText: snippet }).first().click();
  await page.keyboard.press("End");
}

test("Live Preview draws checkboxes, math, diagrams and rendered blocks", async ({ page }) => {
  await openSample(page, "sample");
  const editor = page.locator(".cm-editor");
  await expect(editor.locator("input.cm-gk-task")).toHaveCount(4);
  await expect(editor.locator(".cm-gk-math-block .katex-display")).toBeVisible();
  await expect(editor.locator(".cm-gk-diagram svg")).toBeVisible();
  await expect(editor.locator(".cm-gk-table pre")).toContainText("| a | b |");
  await expect(editor.locator(".cm-gk-callout")).toContainText("[!WARNING]");
  // Markers show on the cursor's line and hide away from it.
  await expect(editor.locator(".cm-gk-h1")).toHaveText("# Fix the sync bug");
  await page.locator(".cm-line", { hasText: "Blocked by" }).click();
  await expect(editor.locator(".cm-gk-h1")).toHaveText("Fix the sync bug");
});

test("a checkbox click changes exactly one character, or asks the engine", async ({ page }) => {
  await openSample(page, "sample");
  const before = await text(page);
  await page.locator("input.cm-gk-task").first().click();
  const after = await text(page);
  expect(after.length).toBe(before.length);
  expect(Array.from({ length: after.length }, (_, i) => i).filter((i) => after.charCodeAt(i) !== before.charCodeAt(i)).length).toBe(1);
  expect(after).toContain("- [x] first item");

  await page.getByText("Checkboxes through the engine").click();
  await page.locator("input.cm-gk-task").nth(1).click();
  const toggles = JSON.parse((await page.getByTestId("editor-toggles").textContent()) ?? "[]") as { number: number; checked: boolean; text: string }[];
  expect(toggles).toEqual([expect.objectContaining({ number: 2, checked: true, text: "second bold item" })]);
  expect(await text(page)).toContain("- [ ] second **bold** item");
});

test("arrow keys enter a block widget and open its source", async ({ page }) => {
  await openSample(page, "sample");
  const editor = page.locator(".cm-editor");
  await expect(editor.locator(".cm-gk-callout")).toBeVisible();
  await cursorAfter(page, "Blocked by");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  await expect(editor.locator(".cm-gk-callout")).toHaveCount(0);
  await expect(editor.locator(".cm-line", { hasText: "[!WARNING]" })).toBeVisible();
  // Off the cursor's line, inline math in the open callout is drawn by KaTeX.
  await expect(editor.locator(".cm-gk-math .katex")).toBeVisible();

  // Up from the line under the math block enters it from below.
  await expect(editor.locator(".cm-gk-math-block")).toBeVisible();
  await page.locator(".cm-line", { hasText: "first item" }).click();
  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("ArrowUp");
  await expect(editor.locator(".cm-gk-math-block")).toHaveCount(0);
  await expect(editor.locator(".cm-line", { hasText: "\\int_0^1" })).toBeVisible();
});

test("completion offers task keys and pages, and slash commands insert blocks", async ({ page }) => {
  await openSample(page, "sample");
  await page.locator(".cm-line").first().click();
  await page.keyboard.press("Control+End");
  await page.keyboard.press("Enter");
  await page.keyboard.type("See TM-4");
  await expect(page.locator(".cm-tooltip-autocomplete")).toContainText("TM-41");
  await page.keyboard.press("Enter");
  await page.keyboard.type(" and [[Road");
  await expect(page.locator(".cm-tooltip-autocomplete")).toContainText("Roadmap");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await page.keyboard.type("/ta");
  await expect(page.locator(".cm-tooltip-autocomplete")).toContainText("Table");
  await page.keyboard.press("Enter");
  expect(await text(page)).toContain("See TM-41 and [[Roadmap]]\n| Column | Column |\n| --- | --- |\n|  |  |");
});

test("Source mode shows the plain Markdown", async ({ page }) => {
  await openSample(page, "sample");
  await page.getByRole("radio", { name: "Source" }).click();
  await expect(page.locator("input.cm-gk-task")).toHaveCount(0);
  await expect(page.locator(".cm-line", { hasText: "# Fix the sync bug" })).toBeVisible();
  await page.getByRole("radio", { name: "Live Preview" }).click();
  await expect(page.locator("input.cm-gk-task")).toHaveCount(4);
});

test("in a CRLF text, Enter, pastes and snippets leave no bare line breaks", async ({ page }) => {
  await openSample(page, "crlf");
  const original = await text(page);
  expect(original).toContain("\r\n");
  await cursorAfter(page, "second");
  await page.keyboard.press("Enter");
  await page.keyboard.type("typed");
  // An HTML paste and a plain paste, as the clipboard delivers them.
  await page.locator(".cm-content").evaluate((content) => {
    const paste = (type: string, data: string) => {
      const transfer = new DataTransfer();
      transfer.setData(type, data);
      content.dispatchEvent(new ClipboardEvent("paste", { clipboardData: transfer, bubbles: true, cancelable: true }));
    };
    paste("text/html", "<ul><li>pasted one</li><li>pasted two</li></ul>");
    paste("text/plain", "\nplain one\nplain two\n");
  });
  await expect.poll(async () => (await text(page)).includes("plain two")).toBe(true);
  await page.keyboard.type(" /code");
  await expect(page.locator(".cm-tooltip-autocomplete")).toContainText("Code block");
  await page.keyboard.press("Enter");
  const after = await text(page);
  expect(after).toContain("- pasted one\r\n- pasted two");
  expect(after).toContain("```\r\n\r\n```");
  expect(after.match(/\r(?!\n)|(?<!\r)\n/g) ?? []).toEqual([]);
});

test("mixed line breaks are converted on opening, with a note", async ({ page }) => {
  await openSample(page, "mixed");
  await expect(page.getByRole("status")).toContainText("All of them now use");
  const converted = await text(page);
  const kinds = new Set(converted.match(/\r\n|\r|\n/g));
  expect(kinds.size).toBe(1);
  await page.getByRole("button", { name: "Dismiss" }).click();
  await expect(page.getByRole("status")).toHaveCount(0);
});

test("typing stays quick on a 100 KB text", async ({ page }) => {
  await openSample(page, "long");
  const length = async () => Number(await page.getByTestId("editor-text").getAttribute("data-length"));
  const before = await length();
  await page.locator(".cm-line").first().click();
  // Into the middle of the text.
  for (let i = 0; i < 40; i++) {
    await page.keyboard.press("PageDown");
  }
  await page.keyboard.press("End");
  // From each key's keydown in the page to the frame after it.
  await page.evaluate(() => {
    const times: number[] = [];
    (window as unknown as { keyTimes: number[] }).keyTimes = times;
    document.addEventListener(
      "keydown",
      () => {
        const start = performance.now();
        requestAnimationFrame(() => {
          setTimeout(() => times.push(performance.now() - start), 0);
        });
      },
      { capture: true },
    );
  });
  for (let i = 0; i < 40; i++) {
    await page.keyboard.press("a");
  }
  await expect.poll(() => page.evaluate(() => (window as unknown as { keyTimes: number[] }).keyTimes.length)).toBe(40);
  const timings = await page.evaluate(() => (window as unknown as { keyTimes: number[] }).keyTimes.slice());
  timings.sort((a, b) => a - b);
  const p95 = timings[Math.floor(timings.length * 0.95)] ?? 0;
  console.info(`100 KB, Chromium: median ${String(timings[20]?.toFixed(1))} ms, p95 ${p95.toFixed(1)} ms per key to the next frame`);
  expect(await length()).toBe(before + 40);
  // A smoke check, not the latency budget: headless Chromium on a busy machine is noisy.
  expect(p95).toBeLessThan(100);
});
