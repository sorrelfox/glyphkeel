// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors
// @vitest-environment node

import { readdirSync, readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { flowTable, PORT_VARIABLES, type PortKind } from "../tests/ports.ts";
import { candidates, parseArgs, portRange } from "./test-flow.ts";

declare const __BRIDGE_PORTS__: readonly [number, number];

const UI = new URL("..", import.meta.url);
const table = flowTable();

describe("the flows' table", () => {
  it("names every Playwright configuration once, and each takes its ports from it", () => {
    const configs = readdirSync(UI)
      .filter((name) => name.startsWith("playwright") && name.endsWith(".config.ts"))
      .sort();
    expect(table.flows.map((flow) => flow.config).sort()).toEqual(configs);
    expect(new Set(table.flows.map((flow) => flow.name)).size).toBe(table.flows.length);
    for (const flow of table.flows) {
      const text = readFileSync(new URL(flow.config, UI), "utf8");
      const calls = [...text.matchAll(/flowPort\("([^"]+)", "([^"]+)"\)/gu)];
      expect(
        calls.map((call) => call[1]),
        flow.config,
      ).toEqual(calls.map(() => flow.name));
      expect(calls.map((call) => call[2]).sort(), flow.config).toEqual(Object.keys(flow.ports).sort());
      // No port written into the configuration itself.
      expect(text, flow.config).not.toMatch(/\b5[34]\d\d\b|process\.env(\.|\[")GK_\w*PORT/u);
    }
  });

  it("knows only the four kinds of port", () => {
    const known = Object.keys(PORT_VARIABLES);
    for (const flow of table.flows) {
      expect(known).toEqual(expect.arrayContaining(Object.keys(flow.ports)));
      expect(known).toEqual(expect.arrayContaining(Object.keys(flow.deprecated ?? {})));
      for (const names of Object.values(flow.deprecated ?? {})) {
        for (const name of names) {
          expect(Object.values(PORT_VARIABLES)).not.toContain(name);
        }
      }
    }
  });

  it("has the range the bridge page is built with", () => {
    if ((process.env.GK_PORT_RANGE ?? "") === "") {
      expect(__BRIDGE_PORTS__).toEqual(table.port_range);
    }
  });
});

describe("pnpm test:flow", () => {
  it("reads the name first, its port options, and hands the rest to Playwright", () => {
    expect(parseArgs([])).toEqual({ name: "flow", ports: {}, playwright: [], list: false, help: false });
    const options = parseArgs(["palette", "--port", "5341", "--bridge-port=5342", "tests/palette/a.spec.ts", "--", "--grep", "quick add", "--port", "1"]);
    expect(options.name).toBe("palette");
    expect(options.ports).toEqual({ ui: 5341, bridge: 5342 });
    expect(options.playwright).toEqual(["tests/palette/a.spec.ts", "--grep", "quick add", "--port", "1"]);
    expect(parseArgs(["list", "--port-2", "5343", "--bridge-port-2", "5344"]).ports).toEqual({ ui2: 5343, bridge2: 5344 } satisfies Partial<Record<PortKind, number>>);
    expect(parseArgs(["--grep", "x"]).name).toBe("flow");
    expect(parseArgs(["--flows"]).list).toBe(true);
    expect(() => parseArgs(["editor", "--port"])).toThrow(/--port/u);
    expect(() => parseArgs(["editor", "--port", "70000"])).toThrow(/--port/u);
    expect(() => parseArgs(["editor", "--bridge-port=x"])).toThrow(/--bridge-port/u);
  });

  it("takes its range from GK_PORT_RANGE or the table", () => {
    expect(portRange(table, "")).toEqual(table.port_range);
    expect(portRange(table, "6000-6009")).toEqual([6000, 6009]);
    for (const wrong of ["6000", "6009-6000", "80-90", "6000-70000", "x-y"]) {
      expect(() => portRange(table, wrong), wrong).toThrow(/GK_PORT_RANGE/u);
    }
  });

  it("picks ports away from the configurations' defaults first", () => {
    const range = table.port_range;
    const order = candidates(table, range, 0.5);
    const size = range[1] - range[0] + 1;
    expect(new Set(order).size).toBe(size);
    const defaults = new Set(table.flows.flatMap((flow) => Object.values(flow.ports)));
    const firstDefault = order.findIndex((port) => defaults.has(port));
    expect(firstDefault).toBeGreaterThan(0);
    expect(order.slice(firstDefault).every((port) => defaults.has(port))).toBe(true);
    expect(candidates(table, range, 0)[0]).toBe(5300);
  });
});
