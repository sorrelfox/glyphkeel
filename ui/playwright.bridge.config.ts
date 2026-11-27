// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { randomBytes } from "node:crypto";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { defineConfig, devices } from "@playwright/test";

// Flows against the real engine: the development bridge serves a fresh
// workspace on loopback, and a `--mode bridge` build of the UI talks to it
// from Chromium. Playwright starts both and stops them afterwards.
// The bridge takes the UI's port only: 5310 unless GK_BRIDGE_UI_PORT names
// another of 5310–5319. Its own port can be another of 5320–5329, so that a
// bridge left running doesn't stop a run.
//
// Two bridges, each on a workspace of its own. workspace.spec.ts checks
// what the whole workspace holds (an empty Undo stack, the first change of
// a setting), so it gets a workspace that no other file has touched, on
// GK_BRIDGE_FRESH_PORT. The other files make their own projects and share
// the second workspace, on GK_BRIDGE_PORT, in whatever order they run.
const UI_PORT = Number(process.env.GK_BRIDGE_UI_PORT ?? 5310);
process.env.GK_BRIDGE_PORT ??= "5321";
process.env.GK_BRIDGE_FRESH_PORT ??= "5322";
const BRIDGE_PORT = Number(process.env.GK_BRIDGE_PORT);
const FRESH_PORT = Number(process.env.GK_BRIDGE_FRESH_PORT);
const BRIDGE_DIST = "node_modules/.cache/bridge-dist";

// The workers inherit these, so every test sees the same token and folders.
process.env.GK_BRIDGE_TOKEN ??= randomBytes(16).toString("hex");
process.env.GK_BRIDGE_WORKSPACE ??= join(mkdtempSync(join(tmpdir(), "glyphkeel-bridge-")), "Personal");
process.env.GK_BRIDGE_FRESH_WORKSPACE ??= join(mkdtempSync(join(tmpdir(), "glyphkeel-bridge-")), "Personal");
// The command line, for flows that set up and check the workspace as
// another process; built with the first bridge.
process.env.GK_BRIDGE_CLI ??= resolve(process.env.CARGO_TARGET_DIR ?? "../target", "debug", "glyphkeel");
const token = process.env.GK_BRIDGE_TOKEN;
const workspace = process.env.GK_BRIDGE_WORKSPACE;
const fresh = process.env.GK_BRIDGE_FRESH_WORKSPACE;

export default defineConfig({
  testDir: "./tests/bridge",
  // The flows that share a workspace run one after another.
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
      command: `cargo build --quiet -p gk-cli && cargo run --quiet -p gk-devbridge -- --workspace "${workspace}" --port ${String(BRIDGE_PORT)} --ui-port ${String(UI_PORT)} --token ${token}`,
      cwd: "..",
      port: BRIDGE_PORT,
      reuseExistingServer: false,
      // The first run compiles the bridge.
      timeout: 600_000,
    },
    {
      command: `cargo run --quiet -p gk-devbridge -- --workspace "${fresh}" --port ${String(FRESH_PORT)} --ui-port ${String(UI_PORT)} --token ${token}`,
      cwd: "..",
      port: FRESH_PORT,
      reuseExistingServer: false,
      timeout: 600_000,
    },
    {
      command: `pnpm exec vite build --mode bridge --outDir ${BRIDGE_DIST} && pnpm exec vite preview --mode bridge --outDir ${BRIDGE_DIST} --port ${String(UI_PORT)} --strictPort`,
      url: `http://127.0.0.1:${String(UI_PORT)}`,
      reuseExistingServer: false,
      timeout: 120_000,
    },
  ],
});
