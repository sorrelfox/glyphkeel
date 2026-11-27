// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { defineConfig, devices } from "@playwright/test";

import { flowPort } from "./tests/ports";

// The Markdown editor's flows, on its development page, against the web
// build in Chromium. Its own port (GK_UI_PORT) and build folder, so it can
// run next to the other flows.
const PORT = flowPort("editor", "ui");
const DIST = "node_modules/.cache/editor-dist";

export default defineConfig({
  testDir: "./tests/editor",
  fullyParallel: true,
  forbidOnly: true,
  retries: 0,
  reporter: "list",
  use: {
    baseURL: `http://127.0.0.1:${String(PORT)}`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1400, height: 1000 } } }],
  webServer: {
    command: `pnpm exec vite build --outDir ${DIST} && pnpm exec vite preview --outDir ${DIST} --port ${String(PORT)} --strictPort`,
    url: `http://127.0.0.1:${String(PORT)}`,
    reuseExistingServer: false,
    timeout: 60_000,
  },
});
