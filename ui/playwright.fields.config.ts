// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { defineConfig, devices } from "@playwright/test";

// Custom fields on sample data: a `--mode fake` build of the UI keeps a
// workspace in memory. A port of its own, so these run beside the other
// flows; `GK_FIELDS_PORT` moves it.
const PORT = Number(process.env.GK_FIELDS_PORT ?? 5311);
const FAKE_DIST = "node_modules/.cache/fields-fake-dist";

export default defineConfig({
  testDir: "./tests/fields",
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
    command: `pnpm exec vite build --mode fake --outDir ${FAKE_DIST} && pnpm exec vite preview --mode fake --outDir ${FAKE_DIST} --port ${String(PORT)} --strictPort`,
    url: `http://127.0.0.1:${String(PORT)}`,
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
