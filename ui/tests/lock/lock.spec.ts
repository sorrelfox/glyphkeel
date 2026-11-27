// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

// The app lock on the sample workspace: a lock passphrase set in
// Preferences, Security; locking by hand, by the shortcut and after idle
// time; the lock screen shows nothing of the workspace and opens only with
// the passphrase; and encrypting the workspace from the same page.

const PASSPHRASE = "orchard lantern";
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];

async function openSecurity(page: Page): Promise<void> {
  await page.keyboard.press("Control+,");
  const dialog = page.getByRole("dialog", { name: "Preferences" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("tab", { name: "Security" }).click();
}

async function setLockPassphrase(page: Page): Promise<void> {
  await openSecurity(page);
  const dialog = page.getByRole("dialog", { name: "Preferences" });
  await dialog.getByRole("button", { name: "Set…" }).click();
  await dialog.getByLabel("New passphrase").fill(PASSPHRASE);
  await dialog.getByLabel("The same passphrase again").fill(PASSPHRASE);
  await dialog.getByRole("button", { name: "Save" }).click();
  await expect(dialog.getByText("A passphrase is set.")).toBeVisible();
}

async function start(page: Page): Promise<void> {
  await page.goto("/");
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeVisible();
}

const lockScreen = (page: Page) => page.getByRole("heading", { name: "Glyphkeel is locked" });

test("the lock screen hides the workspace and opens only with the passphrase", async ({ page }) => {
  await start(page);
  // Without a passphrase there is nothing to unlock with: Lock is off.
  await openSecurity(page);
  const dialog = page.getByRole("dialog", { name: "Preferences" });
  await expect(dialog.getByRole("button", { name: "Lock", exact: true })).toBeDisabled();
  await page.keyboard.press("Escape");

  await setLockPassphrase(page);
  await dialog.getByRole("button", { name: "Lock", exact: true }).click();
  await expect(lockScreen(page)).toBeVisible();
  await expect(page.getByRole("navigation", { name: "Workspace" })).toHaveCount(0);
  await expect(page.getByRole("dialog", { name: "Preferences" })).toHaveCount(0);
  // The field has the focus, ready to type.
  await expect(page.getByLabel("Passphrase", { exact: true })).toBeFocused();

  const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
  expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`)).toEqual([]);

  await page.getByLabel("Passphrase", { exact: true }).fill("not it");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("alert")).toHaveText("That passphrase is not right.");
  await expect(page.getByRole("navigation", { name: "Workspace" })).toHaveCount(0);

  await page.getByLabel("Passphrase", { exact: true }).fill(PASSPHRASE);
  await page.getByRole("button", { name: "Unlock" }).click();
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeVisible();
});

test("the shortcut and the command palette lock the app", async ({ page }) => {
  await start(page);
  await setLockPassphrase(page);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "Preferences" })).toHaveCount(0);
  await page.keyboard.press("Control+Shift+L");
  await expect(lockScreen(page)).toBeVisible();
  await page.getByLabel("Passphrase", { exact: true }).fill(PASSPHRASE);
  await page.keyboard.press("Enter");
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeVisible();

  await page.keyboard.press("Control+K");
  await page.keyboard.type("Lock Glyphkeel");
  await page.keyboard.press("Enter");
  await expect(lockScreen(page)).toBeVisible();
});

test("the app locks itself after the idle time", async ({ page }) => {
  await page.clock.install();
  await start(page);
  await setLockPassphrase(page);
  const dialog = page.getByRole("dialog", { name: "Preferences" });
  await dialog.getByRole("button", { name: /^Lock when idle/ }).click();
  await page.getByRole("option", { name: "After one minute" }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeVisible();
  await page.clock.runFor(30_000);
  await expect(lockScreen(page)).toHaveCount(0);
  await page.clock.runFor(45_000);
  await expect(lockScreen(page)).toBeVisible();
});

test("the workspace is encrypted from Preferences, and its passphrase unlocks", async ({ page }) => {
  await start(page);
  await openSecurity(page);
  const dialog = page.getByRole("dialog", { name: "Preferences" });
  await expect(dialog.getByText("The workspace is not encrypted.")).toBeVisible();
  await dialog.getByRole("button", { name: "Encrypt…" }).click();
  await dialog.getByLabel("New passphrase").fill("sea glass");
  await dialog.getByLabel("The same passphrase again").fill("sea glass");
  await dialog.getByRole("button", { name: "Encrypt", exact: true }).click();
  await expect(dialog.getByText("The workspace is encrypted.")).toBeVisible();
  // An encrypted workspace unlocks with its own passphrase: no lock passphrase needed.
  await expect(dialog.getByRole("button", { name: "Lock", exact: true })).toBeEnabled();
  await dialog.getByRole("button", { name: "Lock", exact: true }).click();
  await expect(lockScreen(page)).toBeVisible();
  await page.getByLabel("Passphrase", { exact: true }).fill("sea glass");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeVisible();
});
