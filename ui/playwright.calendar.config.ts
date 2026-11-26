// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { defineConfig, devices } from "@playwright/test";

// The calendar on the sample data: a production build in the fake mode,
// served on a port of its own.
const PORT = Number(process.env.GK_CALENDAR_PORT ?? 5321);
const DIST = "node_modules/.cache/calendar-fake-dist";

export default defineConfig({
  testDir: "./tests/calendar",
  fullyParallel: true,
  forbidOnly: true,
  retries: 0,
  reporter: "list",
  use: {
    baseURL: `http://127.0.0.1:${String(PORT)}`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1400, height: 900 } } }],
  webServer: {
    command: `pnpm exec vite build --mode fake --outDir ${DIST} && pnpm exec vite preview --mode fake --outDir ${DIST} --port ${String(PORT)} --strictPort`,
    url: `http://127.0.0.1:${String(PORT)}`,
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
