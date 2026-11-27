// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The ports of the Playwright configurations. Every configuration takes its
// page server's port from GK_UI_PORT and its bridge's from GK_BRIDGE_PORT; a
// configuration with two of either takes the second from GK_UI_PORT_2 or
// GK_BRIDGE_PORT_2. Unset, a port is the default in flows.json, the table
// that `pnpm test:flow` and `cargo xtask ci` read as well. The older names
// per configuration (GK_PALETTE_UI_PORT…) still work, are listed in the
// table as deprecated, and give a warning.

import { readFileSync } from "node:fs";

/** The variable that sets each kind of port. */
export const PORT_VARIABLES = {
  ui: "GK_UI_PORT",
  ui2: "GK_UI_PORT_2",
  bridge: "GK_BRIDGE_PORT",
  bridge2: "GK_BRIDGE_PORT_2",
} as const;

export type PortKind = keyof typeof PORT_VARIABLES;

/** One configuration in flows.json. */
export interface FlowEntry {
  name: string;
  config: string;
  about: string;
  /** "pseudo" for the pseudo-locale screenshots; the browser flows otherwise. */
  suite?: string;
  ports: Partial<Record<PortKind, number>>;
  deprecated?: Partial<Record<PortKind, string[]>>;
}

export interface FlowTable {
  /** The ports the development bridge accepts unless GK_PORT_RANGE says otherwise. */
  port_range: [number, number];
  flows: FlowEntry[];
}

export function flowTable(): FlowTable {
  return JSON.parse(readFileSync(new URL("flows.json", import.meta.url), "utf8")) as FlowTable;
}

function parsePort(variable: string, value: string): number {
  const port = Number(value);
  if (!/^\d+$/u.test(value) || port < 1 || port > 65_535) {
    throw new Error(`${variable}=${value} is not a port`);
  }
  return port;
}

/**
 * The port of `kind` for the configuration `name`: its variable, else a
 * deprecated one, else the default in the table. It is written back to its
 * variable, so the test workers (which load the configuration again) and the
 * specs read the same.
 */
export function flowPort(name: string, kind: PortKind): number {
  const flow = flowTable().flows.find((entry) => entry.name === name);
  const fallback = flow?.ports[kind];
  if (flow === undefined || fallback === undefined) {
    throw new Error(`flows.json gives \`${name}\` no ${kind} port`);
  }
  const variable = PORT_VARIABLES[kind];
  let port = fallback;
  const set = process.env[variable];
  if (set !== undefined && set !== "") {
    port = parsePort(variable, set);
  } else {
    const old = (flow.deprecated?.[kind] ?? []).find((alias) => (process.env[alias] ?? "") !== "");
    if (old !== undefined) {
      port = parsePort(old, process.env[old] ?? "");
      // Once, in the main process: the workers load the configuration too.
      if (process.env.TEST_WORKER_INDEX === undefined) {
        console.warn(`${old} is deprecated; use ${variable}`);
      }
    }
  }
  process.env[variable] = String(port);
  return port;
}
