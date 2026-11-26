// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { defineConfig, devices } from "@playwright/test";

// Accessibility checks on sample data: axe-core on every screen and main
// dialog, in the light and the dark theme, against the WCAG 2.2 A and AA
// rules; the page's direction from its locale; motion, contrast and forced
// colours from the desktop's media queries. A `--mode fake` build, on a port
// of its own (GK_A11Y_PORT moves it).
const PORT = Number(process.env.GK_A11Y_PORT ?? 5318);
const DIST = "node_modules/.cache/a11y-fake-dist";

export default defineConfig({
  testDir: "./tests/a11y",
  fullyParallel: true,
  forbidOnly: true,
  retries: 0,
  reporter: "list",
  use: {
    baseURL: `http://127.0.0.1:${String(PORT)}`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1440, height: 900 } } }],
  webServer: {
    command: `pnpm exec vite build --mode fake --outDir ${DIST} && pnpm exec vite preview --mode fake --outDir ${DIST} --port ${String(PORT)} --strictPort`,
    url: `http://127.0.0.1:${String(PORT)}`,
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
