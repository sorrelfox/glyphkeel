// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { defineConfig, devices } from "@playwright/test";

// Screenshots of the main screens in a pseudo-locale, so that text missing
// from the message files, clipped text and joined strings show up. Run by
// `cargo xtask ci --suite pseudo`, which sets the locale (GK_PSEUDO_LOCALE,
// default en-XA) and the folder for the pictures (GK_SCREENSHOT_DIR).
//
// The pseudo-locale exists only in development builds, so this runs the
// development server on sample data (`--mode fake`). The tests skip when the
// page doesn't show the locale.
const PORT = 5411;
const LOCALE = process.env["GK_PSEUDO_LOCALE"] ?? "en-XA";

export default defineConfig({
  testDir: "./tests/pseudo",
  fullyParallel: false,
  forbidOnly: true,
  retries: 0,
  reporter: "list",
  use: {
    baseURL: `http://127.0.0.1:${String(PORT)}`,
    locale: LOCALE,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1600, height: 1000 }, locale: LOCALE } }],
  webServer: {
    command: `pnpm exec vite --mode fake --port ${String(PORT)} --strictPort`,
    url: `http://127.0.0.1:${String(PORT)}`,
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
