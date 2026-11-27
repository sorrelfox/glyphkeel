// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Runs one Playwright configuration on ports of its own, so that several can
// run at the same time, from one checkout or from several:
//
//   pnpm test:flow <name> [--port N] [--port-2 N] [--bridge-port M]
//                         [--bridge-port-2 M] [-- <playwright args>]
//   pnpm test:flow --flows
//
// The names and the ports each configuration needs come from
// tests/flows.json, the table `cargo xtask ci` runs too. Ports that aren't
// given are picked: free on loopback, within GK_PORT_RANGE (the bridge's
// range, 5300–5399 by default), none of the configurations' own defaults,
// and not held by another run of this script (a lock file per port, in
// $XDG_RUNTIME_DIR or the temporary folder). The script prints the process
// IDs of Playwright and of the servers it starts, and on exit, Ctrl-C or
// `kill <its pid>` stops whatever of them is still running. It only ever
// signals processes it saw start under its own Playwright.

import { spawn } from "node:child_process";
import { mkdirSync, readdirSync, readFileSync, readlinkSync, unlinkSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";

import { type FlowEntry, type FlowTable, flowTable, PORT_VARIABLES, type PortKind } from "../tests/ports.ts";

const UI_DIR = fileURLToPath(new URL("..", import.meta.url));

/** The option for each kind of port, and what the printout calls its server. */
export const PORT_OPTIONS: Record<PortKind, { option: string; server: string }> = {
  ui: { option: "--port", server: "page server" },
  ui2: { option: "--port-2", server: "second page server" },
  bridge: { option: "--bridge-port", server: "bridge" },
  bridge2: { option: "--bridge-port-2", server: "second bridge" },
};

/** Made fresh by each configuration unless set; one left in the environment would be shared. */
const FRESH_PER_RUN = ["GK_BRIDGE_WORKSPACE", "GK_BRIDGE_FRESH_WORKSPACE", "GK_BRIDGE_TOKEN"];

const USAGE = `usage: pnpm test:flow [<name>] [--port N] [--port-2 N] [--bridge-port M] [--bridge-port-2 M] [-- <playwright args>]
       pnpm test:flow --flows
Without a name it runs \`flow\`. Arguments it doesn't know go to Playwright.`;

export interface Options {
  name: string;
  ports: Partial<Record<PortKind, number>>;
  playwright: string[];
  list: boolean;
  help: boolean;
}

function kinds(): PortKind[] {
  return Object.keys(PORT_VARIABLES) as PortKind[];
}

function parsePort(option: string, value: string | undefined): number {
  const port = Number(value);
  if (value === undefined || !/^\d+$/u.test(value) || port < 1 || port > 65_535) {
    throw new Error(`${option} needs a port, not ${value ?? "nothing"}`);
  }
  return port;
}

/** Reads the command line: the name first, if any, then the options; everything else goes to Playwright. */
export function parseArgs(args: readonly string[]): Options {
  const options: Options = { name: "flow", ports: {}, playwright: [], list: false, help: false };
  let rest = [...args];
  const first = rest[0];
  if (first !== undefined && !first.startsWith("-")) {
    options.name = first;
    rest = rest.slice(1);
  }
  for (let i = 0; i < rest.length; i += 1) {
    const arg = rest[i] ?? "";
    if (arg === "--") {
      options.playwright.push(...rest.slice(i + 1));
      break;
    }
    const [flag = "", inline] = arg.split(/=(.*)/su, 2);
    const kind = kinds().find((k) => PORT_OPTIONS[k].option === flag);
    if (kind !== undefined) {
      if (inline === undefined) {
        i += 1;
      }
      options.ports[kind] = parsePort(flag, inline ?? rest[i]);
    } else if (arg === "--flows") {
      options.list = true;
    } else if (arg === "--help" || arg === "-h") {
      options.help = true;
    } else {
      options.playwright.push(arg);
    }
  }
  return options;
}

/** The range of ports to pick from: GK_PORT_RANGE, or the table's. */
export function portRange(table: FlowTable, text = process.env.GK_PORT_RANGE): [number, number] {
  if (text === undefined || text.trim() === "") {
    return table.port_range;
  }
  const range = /^(\d+)-(\d+)$/u.exec(text.trim());
  const first = Number(range?.[1]);
  const last = Number(range?.[2]);
  if (range === null || first < 1024 || first > last || last > 65_535) {
    throw new Error(`GK_PORT_RANGE=${text} is not a range of ports such as 5300-5399`);
  }
  return [first, last];
}

/**
 * The order to try ports in: the range from a random place onwards, without
 * the configurations' own defaults (a run without this script uses those),
 * then the defaults within the range, as a last resort.
 */
export function candidates(table: FlowTable, range: [number, number], random = Math.random()): number[] {
  const defaults = new Set(table.flows.flatMap((flow) => Object.values(flow.ports)));
  const all: number[] = [];
  for (let port = range[0]; port <= range[1]; port += 1) {
    all.push(port);
  }
  const start = Math.floor(random * all.length);
  const turned = [...all.slice(start), ...all.slice(0, start)];
  return [...turned.filter((port) => !defaults.has(port)), ...turned.filter((port) => defaults.has(port))];
}

// Processes, from /proc (Linux). Elsewhere the script still runs Playwright
// on its ports, but finds no process IDs to print or stop.

interface Proc {
  pid: number;
  ppid: number;
  /** When it started, in clock ticks since boot: with the ID, it names one process even after the ID is reused. */
  start: string;
}

function readProc(pid: number): Proc | null {
  try {
    const stat = readFileSync(`/proc/${String(pid)}/stat`, "utf8");
    // The name in brackets may hold spaces; the fields after it are fixed.
    const fields = stat.slice(stat.lastIndexOf(")") + 2).split(" ");
    return { pid, ppid: Number(fields[1]), start: fields[19] ?? "" };
  } catch {
    return null;
  }
}

function allProcs(): Proc[] {
  try {
    return readdirSync("/proc")
      .filter((entry) => /^\d+$/u.test(entry))
      .map((entry) => readProc(Number(entry)))
      .filter((proc): proc is Proc => proc !== null);
  } catch {
    return [];
  }
}

function descendants(root: number): Proc[] {
  const children = new Map<number, Proc[]>();
  for (const proc of allProcs()) {
    children.set(proc.ppid, [...(children.get(proc.ppid) ?? []), proc]);
  }
  const found: Proc[] = [];
  const queue = [root];
  for (let pid = queue.shift(); pid !== undefined; pid = queue.shift()) {
    for (const child of children.get(pid) ?? []) {
      found.push(child);
      queue.push(child.pid);
    }
  }
  return found;
}

/** The process's command line, with programs by their file name only. */
function commandLine(pid: number): string {
  const words = readOr(`/proc/${String(pid)}/cmdline`)
    .split("\0")
    .filter((word) => word !== "")
    .map((word) => (word.startsWith("/") ? basename(word) : word));
  // Not the bridge's token.
  const text = words.join(" ").replace(/(--token[= ])\S+/u, "$1…");
  return text === "" ? "?" : text.length > 110 ? `${text.slice(0, 109)}…` : text;
}

function readOr(file: string): string {
  try {
    return readFileSync(file, "utf8");
  } catch {
    return "";
  }
}

/** The sockets listening on TCP, IPv4 and IPv6: their inode, by port. */
function listening(): Map<number, Set<string>> {
  const byPort = new Map<number, Set<string>>();
  for (const file of ["/proc/net/tcp", "/proc/net/tcp6"]) {
    for (const line of readOr(file).split("\n").slice(1)) {
      const fields = line.trim().split(/\s+/u);
      const [, local, , state, , , , , , inode] = fields;
      if (state !== "0A" || local === undefined || inode === undefined) {
        continue;
      }
      const port = Number.parseInt(local.slice(local.lastIndexOf(":") + 1), 16);
      byPort.set(port, (byPort.get(port) ?? new Set()).add(inode));
    }
  }
  return byPort;
}

/** Which of `pids` holds one of the socket `inodes`. */
function holder(pids: readonly number[], inodes: Set<string>): number | undefined {
  for (const pid of pids) {
    try {
      for (const fd of readdirSync(`/proc/${String(pid)}/fd`)) {
        const target = readlinkSync(`/proc/${String(pid)}/fd/${fd}`);
        if (target.startsWith("socket:[") && inodes.has(target.slice("socket:[".length, -1))) {
          return pid;
        }
      }
    } catch {
      // Gone, or not ours to read.
    }
  }
  return undefined;
}

function isFree(port: number): Promise<boolean> {
  if (listening().has(port)) {
    return Promise.resolve(false);
  }
  return new Promise((resolve) => {
    const server = createServer();
    server.once("error", () => {
      resolve(false);
    });
    server.listen({ host: "127.0.0.1", port, exclusive: true }, () => {
      server.close(() => {
        resolve(true);
      });
    });
  });
}

// Lock files: one per port held by a run of this script.

const LOCKS = join(process.env.XDG_RUNTIME_DIR ?? tmpdir(), "glyphkeel-test-ports");

interface Lock {
  pid: number;
  start: string;
}

function lockFile(port: number): string {
  return join(LOCKS, String(port));
}

/** The live run that holds `port`, if any. */
function lockHolder(port: number): Lock | null {
  try {
    const lock = JSON.parse(readFileSync(lockFile(port), "utf8")) as Lock;
    return readProc(lock.pid)?.start === lock.start ? lock : null;
  } catch {
    return null;
  }
}

const held: number[] = [];

function reserve(port: number): boolean {
  mkdirSync(LOCKS, { recursive: true, mode: 0o700 });
  const me = JSON.stringify({ pid: process.pid, start: readProc(process.pid)?.start ?? "" });
  for (let attempt = 0; attempt < 2; attempt += 1) {
    try {
      writeFileSync(lockFile(port), me, { flag: "wx" });
      held.push(port);
      return true;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "EEXIST" || lockHolder(port) !== null) {
        return false;
      }
      // Left by a run that has ended.
      try {
        unlinkSync(lockFile(port));
      } catch {
        // Another run took it first.
      }
    }
  }
  return false;
}

function releaseLocks(): void {
  for (const port of held.splice(0)) {
    try {
      unlinkSync(lockFile(port));
    } catch {
      // Already gone.
    }
  }
}

function log(text: string): void {
  console.error(`test-flow: ${text}`);
}

function findFlow(table: FlowTable, name: string): FlowEntry {
  const flow = table.flows.find((entry) => entry.name === name);
  if (flow === undefined) {
    throw new Error(`unknown flow \`${name}\`; \`pnpm test:flow --flows\` names them`);
  }
  return flow;
}

/** The flow's ports: those given, checked, and free ones for the rest, each held until the script ends. */
async function choosePorts(table: FlowTable, flow: FlowEntry, given: Options["ports"]): Promise<Partial<Record<PortKind, number>>> {
  const range = portRange(table);
  const needs = kinds().filter((kind) => flow.ports[kind] !== undefined);
  const bridged = flow.ports.bridge !== undefined;
  const chosen: Partial<Record<PortKind, number>> = {};
  for (const kind of kinds()) {
    const port = given[kind];
    if (port === undefined) {
      continue;
    }
    if (!needs.includes(kind)) {
      throw new Error(`\`${flow.name}\` has no ${PORT_OPTIONS[kind].server}, so ${PORT_OPTIONS[kind].option} doesn't apply`);
    }
    if (bridged && (port < range[0] || port > range[1])) {
      throw new Error(`${PORT_OPTIONS[kind].option} ${String(port)} is outside ${String(range[0])}–${String(range[1])} (GK_PORT_RANGE), which the bridge refuses`);
    }
    const other = lockHolder(port);
    if (other !== null && other.pid !== process.pid) {
      throw new Error(`port ${String(port)} is held by another test-flow run (pid ${String(other.pid)})`);
    }
    if (!(await isFree(port))) {
      const inodes = listening().get(port);
      const pid = inodes === undefined ? undefined : holder(allProcs().map((proc) => proc.pid), inodes);
      throw new Error(`port ${String(port)} is in use${pid === undefined ? "" : ` by pid ${String(pid)} (${commandLine(pid)})`}`);
    }
    if (!reserve(port)) {
      throw new Error(`port ${String(port)} was taken by another test-flow run`);
    }
    chosen[kind] = port;
  }
  const order = candidates(table, range);
  for (const kind of needs) {
    if (chosen[kind] !== undefined) {
      continue;
    }
    for (const port of order) {
      if (Object.values(chosen).includes(port) || lockHolder(port) !== null || !(await isFree(port))) {
        continue;
      }
      if (reserve(port)) {
        chosen[kind] = port;
        break;
      }
    }
    if (chosen[kind] === undefined) {
      throw new Error(`no free port within ${String(range[0])}–${String(range[1])} (GK_PORT_RANGE)`);
    }
  }
  return chosen;
}

function printList(table: FlowTable): void {
  const lines = table.flows.map((flow) => {
    const ports = kinds()
      .filter((kind) => flow.ports[kind] !== undefined)
      .map((kind) => `${PORT_VARIABLES[kind]}=${String(flow.ports[kind])}`)
      .join(" ");
    return `${flow.name.padEnd(11)} ${flow.config.padEnd(32)} ${ports}\n    ${flow.about}`;
  });
  console.log(lines.join("\n"));
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Stops the processes seen under Playwright that are still running: SIGTERM, then SIGKILL. */
async function stopLeftovers(seen: Map<number, Proc>): Promise<void> {
  const alive = () => [...seen.values()].filter((proc) => readProc(proc.pid)?.start === proc.start);
  const left = alive();
  for (const proc of left) {
    log(`stopping pid ${String(proc.pid)} (${commandLine(proc.pid)})`);
    try {
      process.kill(proc.pid, "SIGTERM");
    } catch {
      // Gone meanwhile.
    }
  }
  for (let waited = 0; waited < 3000 && alive().length > 0; waited += 100) {
    await sleep(100);
  }
  for (const proc of alive()) {
    log(`killing pid ${String(proc.pid)}`);
    try {
      process.kill(proc.pid, "SIGKILL");
    } catch {
      // Gone meanwhile.
    }
  }
}

async function main(args: readonly string[]): Promise<number> {
  const options = parseArgs(args);
  const table = flowTable();
  if (options.help) {
    console.log(USAGE);
    return 0;
  }
  if (options.list) {
    printList(table);
    return 0;
  }
  const flow = findFlow(table, options.name);
  const ports = await choosePorts(table, flow, options.ports);

  const env: NodeJS.ProcessEnv = { ...process.env };
  for (const name of [...Object.values(PORT_VARIABLES), ...Object.values(flow.deprecated ?? {}).flat(), ...FRESH_PER_RUN]) {
    // eslint-disable-next-line @typescript-eslint/no-dynamic-delete -- a copy of the environment
    delete env[name];
  }
  for (const kind of kinds()) {
    const port = ports[kind];
    if (port !== undefined) {
      env[PORT_VARIABLES[kind]] = String(port);
    }
  }
  const playwrightArgs = ["test", "--config", flow.config, ...options.playwright];
  if (!options.playwright.some((arg) => arg === "--output" || arg.startsWith("--output=") || arg === "-o")) {
    // Playwright empties its output folder when it starts: one per run.
    playwrightArgs.push("--output", join("test-results", `${flow.name}-${String(ports.ui)}`));
  }

  const shown = kinds()
    .filter((kind) => ports[kind] !== undefined)
    .map((kind) => `${PORT_OPTIONS[kind].server} 127.0.0.1:${String(ports[kind])}`)
    .join(", ");
  log(`${flow.name} (${flow.config}): ${shown}`);
  const child = spawn(join(UI_DIR, "node_modules", ".bin", "playwright"), playwrightArgs, { cwd: UI_DIR, env, stdio: "inherit" });
  const root = child.pid;
  if (root === undefined) {
    releaseLocks();
    throw new Error("Playwright did not start");
  }
  log(`Playwright is pid ${String(root)}; \`kill ${String(process.pid)}\` stops this run and its servers`);

  const seen = new Map<number, Proc>();
  const announced = new Set<number>();
  const watch = () => {
    const below = descendants(root);
    for (const proc of below) {
      seen.set(proc.pid, proc);
    }
    const sockets = listening();
    for (const kind of kinds()) {
      const port = ports[kind];
      const inodes = port === undefined ? undefined : sockets.get(port);
      if (port === undefined || inodes === undefined || announced.has(port)) {
        continue;
      }
      const pid = holder(
        below.map((proc) => proc.pid),
        inodes,
      );
      if (pid !== undefined) {
        announced.add(port);
        log(`${PORT_OPTIONS[kind].server} on 127.0.0.1:${String(port)} is pid ${String(pid)} (${commandLine(pid)})`);
      }
    }
  };
  const timer = setInterval(watch, 500);

  let signals = 0;
  const onSignal = (signal: NodeJS.Signals) => {
    signals += 1;
    if (signals === 1) {
      log(`${signal}: stopping Playwright`);
      child.kill(signal);
      setTimeout(() => child.kill("SIGKILL"), 20_000).unref();
    } else {
      child.kill("SIGKILL");
    }
  };
  const handled: NodeJS.Signals[] = ["SIGINT", "SIGTERM", "SIGHUP"];
  for (const signal of handled) {
    process.on(signal, onSignal);
  }

  const [code, signal] = await new Promise<[number | null, NodeJS.Signals | null]>((resolve) => {
    child.on("exit", (exitCode, exitSignal) => {
      resolve([exitCode, exitSignal]);
    });
  });
  clearInterval(timer);
  await stopLeftovers(seen);
  releaseLocks();
  log(`${flow.name}: Playwright ended (${signal ?? `exit code ${String(code)}`})`);
  if (signals > 0) {
    return 130;
  }
  return code ?? 1;
}

if (import.meta.main) {
  main(process.argv.slice(2)).then(
    (code) => {
      process.exitCode = code;
    },
    (error: unknown) => {
      releaseLocks();
      log(error instanceof Error ? error.message : String(error));
      process.exitCode = 2;
    },
  );
}
