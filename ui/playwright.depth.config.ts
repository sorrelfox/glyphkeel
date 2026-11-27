// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { randomBytes } from "node:crypto";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { defineConfig, devices } from "@playwright/test";

import { flowPort } from "./tests/ports";

// Versions, custom fields, rules, repeating tasks, the CSV import and the
// reports against the real engine: the development bridge serves a fresh
// workspace, and a `--mode bridge` build of the UI talks to it. The flows
// make their data with the `glyphkeel` command line on the same workspace,
// as another process would, and check the engine's answer the same way.
// Ports of their own (GK_UI_PORT and GK_BRIDGE_PORT move them). The flows
// use the planning flows' helpers, which read the same variables.
const UI_PORT = flowPort("depth", "ui");
const BRIDGE_PORT = flowPort("depth", "bridge");

process.env.GK_BRIDGE_TOKEN ??= randomBytes(16).toString("hex");
process.env.GK_BRIDGE_WORKSPACE ??= join(mkdtempSync(join(tmpdir(), "glyphkeel-depth-")), "Personal");
process.env.GK_PLANNING_UI_URL = `http://127.0.0.1:${String(UI_PORT)}`;
// The command line the flows run; the bridge's server command builds it.
process.env.GK_PLANNING_CLI ??= resolve(process.env.CARGO_TARGET_DIR ?? "../target", "debug", "glyphkeel");
const token = process.env.GK_BRIDGE_TOKEN;
const workspace = process.env.GK_BRIDGE_WORKSPACE;

export default defineConfig({
  testDir: "./tests/depth",
  // One workspace: the flows run one after another.
  workers: 1,
  fullyParallel: false,
  forbidOnly: true,
  retries: 0,
  reporter: "list",
  timeout: 60_000,
  use: { trace: "retain-on-failure" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1400, height: 860 } } }],
  webServer: [
    {
      command: `cargo build --quiet -p gk-cli && cargo run --quiet -p gk-devbridge -- --workspace "${workspace}" --port ${String(BRIDGE_PORT)} --ui-port ${String(UI_PORT)} --token ${token}`,
      cwd: "..",
      port: BRIDGE_PORT,
      reuseExistingServer: false,
      timeout: 600_000,
    },
    {
      command: `pnpm exec vite build --mode bridge --outDir node_modules/.cache/depth-bridge-dist && pnpm exec vite preview --mode bridge --outDir node_modules/.cache/depth-bridge-dist --port ${String(UI_PORT)} --strictPort`,
      url: `http://127.0.0.1:${String(UI_PORT)}`,
      reuseExistingServer: false,
      timeout: 120_000,
    },
  ],
});
