// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Fails when the release UI bundle (dist/) can reach the network or contains
// the development bridge's transport. Run after `pnpm build`.

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const ASSETS = join(import.meta.dirname, "..", "dist", "assets");
const BANNED = [/\bWebSocket\b/, /\bEventSource\b/, /ws:\/\//, /development bridge/];

let files;
try {
  files = readdirSync(ASSETS).filter((name) => name.endsWith(".js"));
} catch {
  console.error(`check-bundle: ${ASSETS} is missing; run \`pnpm build\` first`);
  process.exit(2);
}
if (files.length === 0) {
  console.error("check-bundle: the bundle has no scripts");
  process.exit(2);
}

let failed = false;
for (const name of files) {
  const text = readFileSync(join(ASSETS, name), "utf8");
  for (const pattern of BANNED) {
    if (pattern.test(text)) {
      console.error(`check-bundle: ${name} contains ${String(pattern)}`);
      failed = true;
    }
  }
}
if (failed) {
  process.exit(1);
}
console.log(`check-bundle: ${String(files.length)} scripts, no network code and no bridge`);
