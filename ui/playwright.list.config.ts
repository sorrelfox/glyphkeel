// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { randomBytes } from "node:crypto";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { defineConfig, devices } from "@playwright/test";

import { flowPort } from "./tests/ports";

// The list's flows. Against the real engine: the development bridge serves a
// fresh workspace, and a `--mode bridge` build of the UI talks to it. On
// sample data: a `--mode fake` build, for the long-list scroll check and the
// screenshot. Ports of their own, so these run beside the other flows:
// GK_UI_PORT for the bridge's page, GK_UI_PORT_2 for the sample data's and
// GK_BRIDGE_PORT for the bridge.
const BRIDGE_UI_PORT = flowPort("list", "ui");
const BRIDGE_PORT = flowPort("list", "bridge");
const FAKE_UI_PORT = flowPort("list", "ui2");

process.env.GK_BRIDGE_TOKEN ??= randomBytes(16).toString("hex");
process.env.GK_BRIDGE_WORKSPACE ??= join(mkdtempSync(join(tmpdir(), "glyphkeel-list-")), "Personal");
process.env.GK_LIST_UI_URL = `http://127.0.0.1:${String(BRIDGE_UI_PORT)}`;
process.env.GK_LIST_FAKE_URL = `http://127.0.0.1:${String(FAKE_UI_PORT)}`;
const token = process.env.GK_BRIDGE_TOKEN;
const workspace = process.env.GK_BRIDGE_WORKSPACE;

export default defineConfig({
  testDir: "./tests/list",
  // One workspace for the engine's flows: they run one after another.
  workers: 1,
  fullyParallel: false,
  forbidOnly: true,
  retries: 0,
  reporter: "list",
  use: { trace: "retain-on-failure", acceptDownloads: true },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1400, height: 860 } } }],
  webServer: [
    {
      command: `cargo run --quiet -p gk-devbridge -- --workspace "${workspace}" --port ${String(BRIDGE_PORT)} --ui-port ${String(BRIDGE_UI_PORT)} --token ${token}`,
      cwd: "..",
      port: BRIDGE_PORT,
      reuseExistingServer: false,
      timeout: 600_000,
    },
    {
      command: `pnpm exec vite build --mode bridge --outDir node_modules/.cache/list-bridge-dist && pnpm exec vite preview --mode bridge --outDir node_modules/.cache/list-bridge-dist --port ${String(BRIDGE_UI_PORT)} --strictPort`,
      url: `http://127.0.0.1:${String(BRIDGE_UI_PORT)}`,
      reuseExistingServer: false,
      timeout: 120_000,
    },
    {
      command: `pnpm exec vite build --mode fake --outDir node_modules/.cache/list-fake-dist && pnpm exec vite preview --mode fake --outDir node_modules/.cache/list-fake-dist --port ${String(FAKE_UI_PORT)} --strictPort`,
      url: `http://127.0.0.1:${String(FAKE_UI_PORT)}`,
      reuseExistingServer: false,
      timeout: 120_000,
    },
  ],
});
