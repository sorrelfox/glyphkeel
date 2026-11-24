// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { randomBytes } from "node:crypto";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { defineConfig, devices } from "@playwright/test";

// The trash, the activity feed, backups, export and import, against the real
// engine: the development bridge serves a fresh workspace and confirms what
// can't be undone (the app's native dialog has no stand-in in a browser),
// and a `--mode bridge` build of the UI talks to it. Ports of their own (the
// bridge takes 5320–5329 and pages 5310–5319), so these run beside the other
// flows.
const UI_PORT = Number(process.env.GK_SAFETY_UI_PORT ?? 5314);
const BRIDGE_PORT = Number(process.env.GK_SAFETY_BRIDGE_PORT ?? 5324);

process.env.GK_BRIDGE_TOKEN ??= randomBytes(16).toString("hex");
process.env.GK_BRIDGE_WORKSPACE ??= join(mkdtempSync(join(tmpdir(), "glyphkeel-safety-")), "Personal");
process.env.GK_SAFETY_BRIDGE_PORT = String(BRIDGE_PORT);
process.env.GK_SAFETY_UI_URL = `http://127.0.0.1:${String(UI_PORT)}`;
const token = process.env.GK_BRIDGE_TOKEN;
const workspace = process.env.GK_BRIDGE_WORKSPACE;

export default defineConfig({
  testDir: "./tests/safety",
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
      command: `cargo run --quiet -p gk-devbridge -- --workspace "${workspace}" --port ${String(BRIDGE_PORT)} --ui-port ${String(UI_PORT)} --token ${token} --confirm yes`,
      cwd: "..",
      port: BRIDGE_PORT,
      reuseExistingServer: false,
      timeout: 600_000,
    },
    {
      command: `pnpm exec vite build --mode bridge --outDir node_modules/.cache/safety-bridge-dist && pnpm exec vite preview --mode bridge --outDir node_modules/.cache/safety-bridge-dist --port ${String(UI_PORT)} --strictPort`,
      url: `http://127.0.0.1:${String(UI_PORT)}`,
      reuseExistingServer: false,
      timeout: 120_000,
    },
  ],
});
