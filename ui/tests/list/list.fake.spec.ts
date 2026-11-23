// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test } from "@playwright/test";

const BASE = process.env.GK_LIST_FAKE_URL ?? "";
const SCREENSHOT = process.env.GK_LIST_SCREENSHOT;

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string) => text.replace(/[⁨⁩]/g, "");

test("a list of 20,000 tasks scrolls through, a page of rows at a time", async ({ page }) => {
  await page.goto(`${BASE}/?tasks=20000`);
  await page.getByRole("navigation", { name: "Workspace" }).getByText("All tasks").click();
  await expect(page.locator("[data-index='0']")).toBeVisible();
  await expect.poll(async () => plain(await page.getByTestId("list-totals").innerText())).toMatch(/^20,\d{3} tasks/);

  // A fling-like scroll from top to bottom, a screen at a time, timing every frame.
  const frames = await page.evaluate(async () => {
    const scroller = document.querySelector<HTMLElement>("[data-list-scroll]");
    if (!scroller) {
      throw new Error("no list");
    }
    const times: number[] = [];
    let last = performance.now();
    let running = true;
    const tick = (now: number) => {
      times.push(now - last);
      last = now;
      if (running) {
        requestAnimationFrame(tick);
      }
    };
    requestAnimationFrame(tick);
    const end = scroller.scrollHeight - scroller.clientHeight;
    for (let top = 0; top <= end; top += scroller.clientHeight) {
      scroller.scrollTop = top;
      await new Promise((resolve) => requestAnimationFrame(resolve));
    }
    scroller.scrollTop = end;
    running = false;
    return times.slice(1).sort((a, b) => a - b);
  });
  const p95 = frames[Math.floor(frames.length * 0.95)] ?? 0;
  const median = frames[Math.floor(frames.length / 2)] ?? 0;
  test.info().annotations.push({ type: "frames", description: `${String(frames.length)} frames, median ${median.toFixed(1)} ms, p95 ${p95.toFixed(1)} ms` });
  console.log(`list scroll: ${String(frames.length)} frames, median ${median.toFixed(1)} ms, p95 ${p95.toFixed(1)} ms`);

  // The last rows arrive and the first pages were let go: only the rows near the screen are drawn.
  const last = page.locator("[data-list-scroll] [data-row]").last();
  await expect(last).toBeVisible();
  expect(await page.locator("[data-list-scroll] [role=row]").count()).toBeLessThan(80);
  // Desktop Chromium is no stand-in for WebKitGTK; this only catches gross regressions.
  expect(median).toBeLessThan(25);
});

test("the list on sample data, for the screenshot", async ({ page }) => {
  await page.goto(BASE);
  await page.getByRole("row", { name: /Task Manager/ }).first().click();
  await page.getByRole("group", { name: "Views" }).getByRole("button", { name: "List" }).click();
  const field = page.getByRole("textbox", { name: "Filter query" });
  await field.click();
  await page.keyboard.type("is:open -status:Backlog prio>=normal sort:due");
  await page.keyboard.press("Escape");
  await expect.poll(async () => plain(await page.getByRole("group", { name: "Conditions" }).innerText())).toContain("Status is not Backlog");
  await expect.poll(async () => plain(await page.getByTestId("list-banner").innerText())).toMatch(/^Filtered: \d+ of 80 tasks/);
  await page.locator("[data-index='1'] .key").click();
  await page.locator("[data-index='2'] .key").click({ modifiers: ["Control"] });
  await expect.poll(async () => plain(await page.getByTestId("bulk-bar").innerText())).toContain("2 selected");
  if (SCREENSHOT) {
    await page.screenshot({ path: SCREENSHOT });
  }
});
