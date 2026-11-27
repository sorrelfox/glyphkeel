// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { randomBytes } from "node:crypto";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { defineConfig, devices } from "@playwright/test";

import { flowPort } from "./tests/ports";

// The command palette's and the quick-add page's flows, against the real
// engine: the development bridge serves a fresh workspace, and a
// `--mode bridge` build of the UI talks to it. Ports of their own
// (GK_UI_PORT and GK_BRIDGE_PORT move them), so these run beside the other
// flows. `GK_SCREENSHOT_DIR` names a folder for the two screenshots.
const UI_PORT = flowPort("palette", "ui");
const BRIDGE_PORT = flowPort("palette", "bridge");

process.env.GK_BRIDGE_TOKEN ??= randomBytes(16).toString("hex");
process.env.GK_BRIDGE_WORKSPACE ??= join(mkdtempSync(join(tmpdir(), "glyphkeel-palette-")), "Personal");
process.env.GK_PALETTE_UI_URL = `http://127.0.0.1:${String(UI_PORT)}`;
const token = process.env.GK_BRIDGE_TOKEN;
const workspace = process.env.GK_BRIDGE_WORKSPACE;

export default defineConfig({
  testDir: "./tests/palette",
  // One workspace: the flows run one after another.
  workers: 1,
  fullyParallel: false,
  forbidOnly: true,
  retries: 0,
  reporter: "list",
  use: { trace: "retain-on-failure" },
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
      command: `pnpm exec vite build --mode bridge --outDir node_modules/.cache/palette-bridge-dist && pnpm exec vite preview --mode bridge --outDir node_modules/.cache/palette-bridge-dist --port ${String(UI_PORT)} --strictPort`,
      url: `http://127.0.0.1:${String(UI_PORT)}`,
      reuseExistingServer: false,
      timeout: 120_000,
    },
  ],
});
