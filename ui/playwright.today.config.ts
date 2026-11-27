// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { randomBytes } from "node:crypto";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { defineConfig, devices } from "@playwright/test";

import { flowPort } from "./tests/ports";

// Today, the search page and links in the editor, against the real engine:
// the development bridge serves a workspace of their own, so the changes
// they make (projects, an Undo stack) never meet the other bridge flows,
// which expect a fresh one. Ports of their own (GK_UI_PORT and
// GK_BRIDGE_PORT move them), so these run beside the other flows.
// `GK_SCREENSHOT_DIR` names a folder for the screenshots.
const UI_PORT = flowPort("today", "ui");
const BRIDGE_PORT = flowPort("today", "bridge");
const DIST = "node_modules/.cache/today-bridge-dist";

process.env.GK_BRIDGE_TOKEN ??= randomBytes(16).toString("hex");
process.env.GK_BRIDGE_WORKSPACE ??= join(mkdtempSync(join(tmpdir(), "glyphkeel-today-")), "Personal");
const token = process.env.GK_BRIDGE_TOKEN;
const workspace = process.env.GK_BRIDGE_WORKSPACE;

export default defineConfig({
  testDir: "./tests/today",
  // One workspace: the flows run one after another.
  workers: 1,
  fullyParallel: false,
  forbidOnly: true,
  retries: 0,
  reporter: "list",
  use: {
    baseURL: `http://127.0.0.1:${String(UI_PORT)}`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } } }],
  webServer: [
    {
      command: `cargo run --quiet -p gk-devbridge -- --workspace "${workspace}" --port ${String(BRIDGE_PORT)} --ui-port ${String(UI_PORT)} --token ${token}`,
      cwd: "..",
      port: BRIDGE_PORT,
      reuseExistingServer: false,
      timeout: 600_000,
    },
    {
      command: `pnpm exec vite build --mode bridge --outDir ${DIST} && pnpm exec vite preview --mode bridge --outDir ${DIST} --port ${String(UI_PORT)} --strictPort`,
      url: `http://127.0.0.1:${String(UI_PORT)}`,
      reuseExistingServer: false,
      timeout: 120_000,
    },
  ],
});
