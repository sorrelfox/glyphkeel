// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { expect, test, type Locator, type Page } from "@playwright/test";

/** Flips a switch by its visible label: the input itself is visually hidden. */
async function toggle(control: Locator): Promise<void> {
  await control.locator("xpath=ancestor::label[1]").click();
}

async function agentsPage(page: Page): Promise<Locator> {
  // The main window's smallest width: the sidebar folds into the main menu.
  await page.setViewportSize({ width: 760, height: 900 });
  await page.goto("/");
  await page.getByRole("button", { name: "Main menu" }).click();
  await page.getByRole("menuitem", { name: /Preferences/ }).click();
  const dialog = page.getByRole("dialog", { name: "Preferences" });
  await dialog.getByRole("tab", { name: "Agents & CLI" }).click();
  return dialog.getByRole("tabpanel", { name: "Agents & CLI" });
}

// Outside the app nothing listens, but the page shows and keeps the set-up
// the app would use: on and off, the port, the address and the token.
test("the local HTTP endpoint is set up in Preferences", async ({ page }) => {
  const panel = await agentsPage(page);
  const serve = panel.getByRole("switch", { name: "Serve agents over HTTP" });
  await expect(serve).not.toBeChecked();
  await expect(panel.getByTestId("agent-http-url")).toHaveCount(0);
  await expect(panel.getByText("Only the app serves agents over HTTP.", { exact: false })).toBeVisible();

  // On: an address on 127.0.0.1, and a token that stays hidden until shown.
  await toggle(serve);
  await expect(serve).toBeChecked();
  const url = panel.getByTestId("agent-http-url");
  await expect(url).toHaveText(/^http:\/\/127\.0\.0\.1:\d+\/mcp$/u);
  const token = panel.getByTestId("agent-http-token");
  await expect(token).not.toHaveText(/[0-9a-f]{64}/u);
  await panel.getByRole("button", { name: "Show the token" }).click();
  await expect(token).toHaveText(/^[0-9a-f]{64}$/u);
  const first = await token.textContent();
  await expect(panel.getByRole("group", { name: "HTTP set-up" })).toContainText(`Bearer ${first ?? ""}`);

  // A new token replaces the old one.
  await panel.getByRole("button", { name: "New token" }).click();
  await expect(token).not.toHaveText(first ?? "");
  await expect(token).toHaveText(/^[0-9a-f]{64}$/u);

  // A port of the person's choosing.
  await toggle(panel.getByRole("switch", { name: "Any free port" }));
  const port = panel.getByRole("spinbutton", { name: "Port" });
  await port.fill("50123");
  await port.press("Enter");
  await expect(url).toHaveText("http://127.0.0.1:50123/mcp");

  // The set-up stays after a reload; off hides the address, keeps the token.
  await page.reload();
  const again = await agentsPage(page);
  await expect(again.getByTestId("agent-http-url")).toHaveText("http://127.0.0.1:50123/mcp");
  await toggle(again.getByRole("switch", { name: "Serve agents over HTTP" }));
  await expect(again.getByTestId("agent-http-url")).toHaveCount(0);
  await expect(again.getByTestId("agent-http-token")).toBeVisible();
});

test("agents may ask the person, unless that is turned off", async ({ page }) => {
  const panel = await agentsPage(page);
  const ask = panel.getByRole("switch", { name: "Agents may ask you" });
  await expect(ask).toBeChecked();
  await toggle(ask);
  await expect(ask).not.toBeChecked();
  await toggle(ask);
  await expect(ask).toBeChecked();
});
