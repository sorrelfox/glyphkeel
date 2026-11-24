// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { randomBytes } from "node:crypto";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { defineConfig, devices } from "@playwright/test";

// Preferences, the first-run set-up, the project tree's menu and the
// pseudo-locale, against the real engine: the development bridge serves a
// fresh workspace and a `--mode bridge` build of the UI talks to it. Ports
// of their own within the bridge's ranges, so these run beside the other
// flows. `GK_SHOTS` names a folder to save screenshots of the new screens in.
const UI_PORT = Number(process.env.GK_PREFS_UI_PORT ?? 5318);
const BRIDGE_PORT = Number(process.env.GK_PREFS_BRIDGE_PORT ?? 5328);

process.env.GK_BRIDGE_TOKEN ??= randomBytes(16).toString("hex");
process.env.GK_BRIDGE_WORKSPACE ??= join(mkdtempSync(join(tmpdir(), "glyphkeel-prefs-")), "Personal");
process.env.GK_PREFS_BRIDGE_PORT = String(BRIDGE_PORT);
const token = process.env.GK_BRIDGE_TOKEN;
const workspace = process.env.GK_BRIDGE_WORKSPACE;

export default defineConfig({
  testDir: "./tests/prefs",
  // One workspace: the flows run one after another.
  workers: 1,
  fullyParallel: false,
  forbidOnly: true,
  retries: 0,
  reporter: "list",
  use: { baseURL: `http://127.0.0.1:${String(UI_PORT)}`, trace: "retain-on-failure" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } } }],
  webServer: [
    {
      command: `cargo run --quiet -p gk-devbridge -- --workspace "${workspace}" --port ${String(BRIDGE_PORT)} --ui-port ${String(UI_PORT)} --token ${token} --confirm yes`,
      cwd: "..",
      port: BRIDGE_PORT,
      reuseExistingServer: false,
      timeout: 600_000,
    },
    {
      command: `pnpm exec vite build --mode bridge --outDir node_modules/.cache/prefs-bridge-dist && pnpm exec vite preview --mode bridge --outDir node_modules/.cache/prefs-bridge-dist --port ${String(UI_PORT)} --strictPort`,
      url: `http://127.0.0.1:${String(UI_PORT)}`,
      reuseExistingServer: false,
      timeout: 120_000,
    },
  ],
});
