// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { defineConfig, devices } from "@playwright/test";

import { flowPort } from "./tests/ports";

// Flows run against the web build in Chromium. Playwright's own WebKit build
// doesn't run on every Linux distribution; the checks that depend on WebKitGTK
// itself run in the desktop app instead. The build has a folder of its own,
// apart from `pnpm build`'s dist/, and GK_UI_PORT moves the server.
const PORT = flowPort("flow", "ui");
const DIST = "node_modules/.cache/flow-dist";

export default defineConfig({
  testDir: "./tests/flow",
  fullyParallel: true,
  forbidOnly: true,
  retries: 0,
  reporter: "list",
  use: {
    baseURL: `http://127.0.0.1:${String(PORT)}`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1600, height: 1000 } } }],
  webServer: {
    command: `pnpm exec vite build --outDir ${DIST} && pnpm exec vite preview --outDir ${DIST} --port ${String(PORT)} --strictPort`,
    url: `http://127.0.0.1:${String(PORT)}`,
    reuseExistingServer: false,
    timeout: 60_000,
  },
});
