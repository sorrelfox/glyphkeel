// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { readFileSync } from "node:fs";

import babel from "@rolldown/plugin-babel";
import react, { reactCompilerPreset } from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

import { frozenPrototype } from "./scripts/frozen-prototype";

// The development server listens on loopback only. The desktop app loads it
// in development; release builds embed the files from `dist/`.
const DEV_PORT = 5310;

// The product's version, from the workspace's Cargo.toml, for the sample data
// that stands in for the engine.
function workspaceVersion(): string {
  const manifest = readFileSync(new URL("../Cargo.toml", import.meta.url), "utf8");
  const version = /^\[workspace\.package\][^[]*?^version = "([^"]+)"/mu.exec(manifest)?.[1];
  if (version === undefined) {
    throw new Error("no [workspace.package] version in Cargo.toml");
  }
  return version;
}

export default defineConfig(({ mode }) => ({
  plugins: [frozenPrototype(), react(), babel({ presets: [reactCompilerPreset()] })],
  // `vite --mode bridge` builds a page for an ordinary browser that talks to
  // the development bridge on loopback. In every other build this is false,
  // and the bridge's code is dropped as dead code.
  // `vite --mode fake` works on sample data in memory instead of the engine,
  // for work on the screens and their browser flows.
  define: {
    __BRIDGE__: JSON.stringify(mode === "bridge"),
    __FAKE__: JSON.stringify(mode === "fake"),
    __VERSION__: JSON.stringify(workspaceVersion()),
  },
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: DEV_PORT,
    strictPort: true,
  },
  preview: {
    host: "127.0.0.1",
    port: DEV_PORT,
    strictPort: true,
  },
  build: {
    // WebKitGTK 2.4x and later, on every supported distribution.
    target: "safari16",
    // No source maps: they would carry local paths into the release.
    sourcemap: false,
    // Never inline assets as data URLs into scripts; keep them as files.
    assetsInlineLimit: 0,
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}", "scripts/*.test.ts"],
  },
}));
