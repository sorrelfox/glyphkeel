// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { defineConfig, devices } from "@playwright/test";

// Layout at small window sizes on sample data: every main screen at the
// window's smallest size and at a medium one, with the sidebar open and
// closed. Nothing may scroll the page sideways or clip a toolbar. A
// `--mode fake` build, on a port of its own (GK_LAYOUT_PORT moves it).
const PORT = Number(process.env.GK_LAYOUT_PORT ?? 5325);
const DIST = "node_modules/.cache/layout-fake-dist";

export default defineConfig({
  testDir: "./tests/layout",
  fullyParallel: true,
  forbidOnly: true,
  retries: 0,
  reporter: "list",
  use: {
    baseURL: `http://127.0.0.1:${String(PORT)}`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: `pnpm exec vite build --mode fake --outDir ${DIST} && pnpm exec vite preview --mode fake --outDir ${DIST} --port ${String(PORT)} --strictPort`,
    url: `http://127.0.0.1:${String(PORT)}`,
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
