// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test } from "@playwright/test";

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null) => (text ?? "").replace(/[⁨⁩]/g, "");

/** A one-page PDF whose page shows `text`. */
function pdf(text: string): Buffer {
  return Buffer.from(`%PDF-1.4\n1 0 obj\n<< /Length 40 >>\nstream\nBT /F1 18 Tf 72 720 Td (${text}) Tj ET\nendstream\nendobj\n%%EOF\n`, "latin1");
}

// A word that is only inside an attached PDF finds the task's file, named
// with its task, with the words around the match marked; Preferences, Data
// counts the files read for search.
test("search finds a word inside an attached PDF and Preferences shows the files read", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("navigation", { name: "Workspace" }).getByText("Task Manager").first().click();
  const card = page.locator("section[data-lane] [data-card]").first();
  await expect(card).toBeVisible();
  const key = (await card.getAttribute("data-card")) ?? "";
  await card.dblclick();
  const files = page.getByRole("complementary", { name: /^Task / }).getByRole("region", { name: "Files" });
  await files.locator("input[type=file]").setInputFiles({ name: "contract.pdf", mimeType: "application/pdf", buffer: pdf("Liability of the quokka supplier") });
  await expect(files.getByText("contract.pdf")).toBeVisible();

  const field = page.getByRole("navigation", { name: "Workspace" }).getByRole("combobox", { name: "Search the workspace" });
  await field.fill("quokka");
  await field.press("Enter");
  const results = page.getByRole("list", { name: "Search results" });
  const hit = results.locator("[data-hit=file]");
  await expect(hit).toHaveCount(1);
  await expect(hit).toContainText(key);
  await expect(hit).toContainText("contract.pdf");
  expect(plain(await hit.textContent())).toContain(`In file contract.pdf of ${key}:`);
  await expect(hit.locator("mark")).toHaveText("quokka");

  await page.getByRole("button", { name: "Main menu" }).click();
  await page.getByRole("menuitem", { name: /Preferences/ }).click();
  const prefs = page.getByRole("dialog", { name: "Preferences" });
  await prefs.getByRole("tab", { name: "Data and backups" }).click();
  await expect(prefs.getByText("Search inside files")).toBeVisible();
  await expect(prefs.getByTestId("file-index-waiting")).toHaveText("All files read");
  expect(Number(await prefs.getByTestId("file-index-with-text").textContent())).toBeGreaterThanOrEqual(1);
  await prefs.getByRole("button", { name: "Read again" }).click();
  await expect(page.getByText("Reading the attached files again.")).toBeVisible();
});
