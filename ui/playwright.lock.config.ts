// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { defineConfig, devices } from "@playwright/test";

// The app lock on sample data: Preferences, Security, locking by hand, by
// the shortcut and after idle time, the lock screen (with axe), and the
// encryption settings. A `--mode fake` build, on a port of its own
// (GK_LOCK_PORT moves it).
const PORT = Number(process.env.GK_LOCK_PORT ?? 5325);
const DIST = "node_modules/.cache/lock-fake-dist";

export default defineConfig({
  testDir: "./tests/lock",
  fullyParallel: true,
  forbidOnly: true,
  retries: 0,
  reporter: "list",
  use: {
    baseURL: `http://127.0.0.1:${String(PORT)}`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } } }],
  webServer: {
    command: `pnpm exec vite build --mode fake --outDir ${DIST} && pnpm exec vite preview --mode fake --outDir ${DIST} --port ${String(PORT)} --strictPort`,
    url: `http://127.0.0.1:${String(PORT)}`,
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
