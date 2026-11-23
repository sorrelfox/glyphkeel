// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { defineConfig, devices } from "@playwright/test";

// Flows of the project screens on sample data: a `--mode fake` build of the
// UI keeps a workspace in memory, so these run without the engine. The same
// flows against the engine run through the development bridge.
const PORT = 5310;
const FAKE_DIST = "node_modules/.cache/fake-dist";

export default defineConfig({
  testDir: "./tests/board",
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
    command: `pnpm exec vite build --mode fake --outDir ${FAKE_DIST} && pnpm exec vite preview --mode fake --outDir ${FAKE_DIST}`,
    url: `http://127.0.0.1:${String(PORT)}`,
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
