// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { defineConfig, devices } from "@playwright/test";

// Flows run against the web build in Chromium. Playwright's own WebKit build
// doesn't run on every Linux distribution; the checks that depend on WebKitGTK
// itself run in the desktop app instead.
const PORT = 5310;

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
    command: "pnpm exec vite build && pnpm exec vite preview",
    url: `http://127.0.0.1:${String(PORT)}`,
    reuseExistingServer: false,
    timeout: 60_000,
  },
});
