// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { defineConfig, devices } from "@playwright/test";

import { flowPort } from "./tests/ports";

// The wiki's flows on sample data: a `--mode fake` build of the UI keeps a
// workspace in memory (`?wiki` adds sample pages), so these run without the
// engine. A port of their own (GK_UI_PORT moves
// it), so they run beside the other flows. `GK_SCREENSHOT_DIR` names a folder for
// the screenshots.
const PORT = flowPort("wiki", "ui");
const DIST = "node_modules/.cache/wiki-fake-dist";

export default defineConfig({
  testDir: "./tests/wiki",
  fullyParallel: true,
  forbidOnly: true,
  retries: 0,
  reporter: "list",
  use: {
    baseURL: `http://127.0.0.1:${String(PORT)}`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1400, height: 860 } } }],
  webServer: {
    command: `pnpm exec vite build --mode fake --outDir ${DIST} && pnpm exec vite preview --mode fake --outDir ${DIST} --port ${String(PORT)} --strictPort`,
    url: `http://127.0.0.1:${String(PORT)}`,
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
