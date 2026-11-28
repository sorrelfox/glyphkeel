// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

// The Approvals inbox on the sample data with proposals (`?proposals`):
// changes agents proposed, by session, with what each changes; approving
// one, approving a stale one anyway, rejecting with a reason, the sidebar's
// count following, the setting in Preferences, and no WCAG 2.2 AA
// violations.

/** The WCAG 2.0, 2.1 and 2.2 rules at levels A and AA. */
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"];

async function axe(page: Page) {
  const results = await new AxeBuilder({ page }).options({ rules: { "target-size": { enabled: true } } }).withTags(TAGS).analyze();
  expect(results.violations.map((violation) => `${violation.id}: ${violation.nodes.map((node) => node.target.join(" ")).join(", ")}`)).toEqual([]);
}

const badge = (page: Page) => page.getByRole("navigation", { name: "Workspace" }).locator("[data-count=approvals]");

test("the inbox lists proposals with their diff, approves and rejects them, and the count follows", async ({ page }) => {
  await page.goto("/?proposals");
  await expect(badge(page)).toHaveText("4");
  await page.getByRole("button", { name: /^Approvals/ }).click();
  const screen = page.getByRole("region", { name: "Approvals" });
  const build = screen.getByRole("region", { name: /Build agent.*session/ });
  await expect(build.locator("[data-proposal]")).toHaveCount(3);
  await expect(build.getByTestId("proposal-diff").first()).toContainText(/Priority\W*:/);
  await axe(page);

  // One without conflicts.
  const comment = build.getByRole("listitem", { name: "Add a comment" });
  await expect(comment.getByTestId("proposal-diff")).toContainText(/New: \W*Comment/);
  await comment.getByRole("button", { name: "Approve", exact: true }).click();
  await expect(comment).toBeHidden();
  await expect(badge(page)).toHaveText("3");

  // The person renamed the task since: approving says so, and runs only "anyway".
  const stale = build.locator("[data-proposal]").filter({ has: page.getByTestId("proposal-conflicts") });
  await expect(stale.getByTestId("proposal-conflicts")).toContainText(/your wording\)\W* now/);
  await stale.getByRole("button", { name: "Approve anyway" }).click();
  await expect(stale).toHaveCount(0);
  await expect(badge(page)).toHaveText("2");

  // Rejecting asks for a reason the agent reads.
  const review = screen.getByRole("region", { name: /Review agent/ });
  await review.getByRole("button", { name: "Reject…" }).click();
  const dialog = page.getByRole("dialog", { name: "Reject the change" });
  await dialog.getByRole("textbox").fill("Keep it for the next release.");
  await axe(page);
  await dialog.getByRole("button", { name: "Reject", exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(review).toBeHidden();
  await expect(badge(page)).toHaveText("1");

  // The decided ones, with the reason.
  await screen.getByRole("radio", { name: "Rejected" }).or(screen.getByRole("button", { name: "Rejected" })).click();
  await expect(screen.getByText(/Reason: \W*Keep it for the next release\./)).toBeVisible();
  await screen.getByRole("radio", { name: "Approved" }).or(screen.getByRole("button", { name: "Approved" })).click();
  await expect(screen.locator("[data-proposal]")).toHaveCount(2);
  await axe(page);

  // The approved change shows in the activity as approved by the person.
  await page.getByRole("button", { name: "Activity", exact: true }).click();
  await expect(page.getByText("You approved this change the agent proposed.").first()).toBeVisible();
});

test("the setting per profile is in Preferences", async ({ page }) => {
  await page.goto("/");
  await expect(badge(page)).toHaveCount(0);
  await page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name: /Preferences/ }).click();
  const dialog = page.getByRole("dialog", { name: "Preferences" });
  await dialog.getByRole("tab", { name: "Agents & CLI" }).click();
  await expect(dialog.getByText("Changes need approval", { exact: true })).toBeVisible();
  const standard = dialog.getByRole("button", { name: /Standard agents/ });
  await standard.click();
  await page.getByRole("option", { name: "Deletions only" }).click();
  await expect(standard).toContainText("Deletions only");
  await axe(page);
});
