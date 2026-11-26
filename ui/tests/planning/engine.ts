// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { execFileSync } from "node:child_process";

import { expect, type Page } from "@playwright/test";

// Helpers for the flows against the engine: the page's address, the
// command line on the bridge's workspace, and dates around today.

const BASE = process.env.GK_PLANNING_UI_URL ?? "";
export const PAGE = `${BASE}/?bridge=${process.env.GK_PLANNING_BRIDGE_PORT ?? ""}&token=${process.env.GK_BRIDGE_TOKEN ?? ""}`;

/** Runs `glyphkeel --json <args>` on the bridge's workspace, as another process, and returns its output. */
export function gk(...args: string[]): unknown {
  const out = execFileSync(process.env.GK_PLANNING_CLI ?? "glyphkeel", ["--workspace", process.env.GK_BRIDGE_WORKSPACE ?? "", "--json", ...args], {
    encoding: "utf8",
    env: { ...process.env, NO_COLOR: "1" },
  });
  return out.trim() === "" ? null : JSON.parse(out);
}

/** A task's start and due date as the engine has them. */
export function datesOf(key: string): [string | null, string | null] {
  const shown = gk("task", "show", key) as { items: { start_date?: string; due_date?: string }[] };
  const task = shown.items[0];
  return [task?.start_date ?? null, task?.due_date ?? null];
}

/** Today, `days` on, as `YYYY-MM-DD` in this machine's zone (the engine's too). */
export function day(days = 0): string {
  const at = new Date();
  at.setHours(12, 0, 0, 0);
  at.setDate(at.getDate() + days);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${String(at.getFullYear())}-${pad(at.getMonth() + 1)}-${pad(at.getDate())}`;
}

/** A project made from the command line; returns its key. */
export function project(name: string, key: string): string {
  gk("project", "create", "--name", name, "--key", key);
  return key;
}

/** Opens a project's view, chosen from the sidebar, in a fresh page. */
export async function openView(page: Page, projectName: string, view: "Wiki" | "Gantt" | "Calendar"): Promise<void> {
  await page.goto(PAGE);
  await expect(page.getByTestId("status-workspace")).toHaveText("Personal");
  await page.getByRole("navigation", { name: "Workspace" }).getByText(projectName, { exact: true }).first().click();
  await expect(page.getByRole("group", { name: /Board of/ })).toBeVisible();
  await page.getByRole("group", { name: "Project views" }).getByRole("button", { name: view }).click();
}

/** Fluent wraps placeables in invisible isolation marks. */
export const plain = (text: string | null) => (text ?? "").replace(/[⁨⁩]/g, "");
