// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { execFileSync } from "node:child_process";

import { expect, test, type Locator, type Page } from "@playwright/test";

// The sidebar's project count on the real engine: open tasks only. The
// tasks are made and closed from the command line, another process on the
// same workspace, and the count follows through the change events.

const token = process.env.GK_BRIDGE_TOKEN ?? "";
const PAGE = `/?bridge=${process.env.GK_BRIDGE_PORT ?? ""}&token=${token}`;

/** Runs `glyphkeel --json <args>` on the bridge's workspace and returns its output. */
function gk(...args: string[]): unknown {
  const out = execFileSync(process.env.GK_BRIDGE_CLI ?? "glyphkeel", ["--workspace", process.env.GK_BRIDGE_WORKSPACE ?? "", "--json", "--tz", "UTC", ...args], {
    encoding: "utf8",
    env: { ...process.env, NO_COLOR: "1" },
  });
  return out.trim() === "" ? null : JSON.parse(out);
}

function task(project: string, title: string): string {
  const made = gk("task", "create", "--project", project, "--title", title) as { task: { key: string } };
  return made.task.key;
}

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null) => (text ?? "").replace(/[⁨⁩]/g, "");

function countOf(page: Page, name: string): Locator {
  return page.getByRole("treegrid", { name: "Projects" }).getByRole("row", { name }).locator('[data-count="project"]');
}

test("a project's count in the sidebar is its open tasks, and follows closing and reopening", async ({ page }) => {
  gk("project", "create", "--name", "Lighthouse", "--key", "LHS");
  const keys = Array.from({ length: 10 }, (_, n) => task("LHS", `Lamp ${String(n + 1)}`));
  for (const key of keys.slice(0, 3)) {
    gk("task", "move", key, "--status", "Done");
  }
  gk("task", "delete", keys[3] ?? "");

  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).not.toHaveText("Workspace unavailable");
  const count = countOf(page, "Lighthouse");
  // Ten tasks, three done and one in the trash.
  await expect(count).toHaveText("6");
  expect(plain(await count.getAttribute("aria-label"))).toBe("6 open tasks");
  expect(plain(await count.getAttribute("title"))).toBe("6 open tasks");

  gk("task", "move", keys[4] ?? "", "--status", "Done");
  await expect(count).toHaveText("5");
  gk("task", "move", keys[0] ?? "", "--status", "To do");
  await expect(count).toHaveText("6");
  await expect.poll(async () => plain(await count.getAttribute("aria-label"))).toBe("6 open tasks");
});
