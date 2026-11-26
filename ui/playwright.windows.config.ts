// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { defineConfig, devices } from "@playwright/test";

// The page as it starts on Windows, on sample data: the flows hand it the
// start data the Windows app gives (system title bar, `platform: windows`)
// and check what differs there, next to Linux. A `--mode fake` build keeps
// the workspace in memory, so no engine runs. `GK_WINDOWS_PORT` moves the
// server, so these can run beside the other flows.
const PORT = Number(process.env.GK_WINDOWS_PORT ?? 5320);
const DIST = "node_modules/.cache/windows-dist";

export default defineConfig({
  testDir: "./tests/windows",
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
