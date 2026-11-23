// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { randomBytes } from "node:crypto";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { defineConfig, devices } from "@playwright/test";

// Flows against the real engine: the development bridge serves a fresh
// workspace on loopback, and a `--mode bridge` build of the UI talks to it
// from Chromium. Playwright starts both and stops them afterwards.
// The bridge takes the UI's development port only. Its own port can be
// another of 5320–5329, so that a bridge left running doesn't stop a run.
const UI_PORT = 5310;
process.env.GK_BRIDGE_PORT ??= "5321";
const BRIDGE_PORT = Number(process.env.GK_BRIDGE_PORT);
const BRIDGE_DIST = "node_modules/.cache/bridge-dist";

// The workers inherit these, so every test sees the same token and folder.
process.env.GK_BRIDGE_TOKEN ??= randomBytes(16).toString("hex");
process.env.GK_BRIDGE_WORKSPACE ??= join(mkdtempSync(join(tmpdir(), "glyphkeel-bridge-")), "Personal");
const token = process.env.GK_BRIDGE_TOKEN;
const workspace = process.env.GK_BRIDGE_WORKSPACE;

export default defineConfig({
  testDir: "./tests/bridge",
  // One workspace for all: the flows run one after another.
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
      command: `cargo run --quiet -p gk-devbridge -- --workspace "${workspace}" --port ${String(BRIDGE_PORT)} --token ${token}`,
      cwd: "..",
      port: BRIDGE_PORT,
      reuseExistingServer: false,
      // The first run compiles the bridge.
      timeout: 600_000,
    },
    {
      command: `pnpm exec vite build --mode bridge --outDir ${BRIDGE_DIST} && pnpm exec vite preview --mode bridge --outDir ${BRIDGE_DIST}`,
      url: `http://127.0.0.1:${String(UI_PORT)}`,
      reuseExistingServer: false,
      timeout: 120_000,
    },
  ],
});
