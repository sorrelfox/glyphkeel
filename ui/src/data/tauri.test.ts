// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { beforeEach, describe, expect, it, vi } from "vitest";

import { type Changed } from "../generated/ops";
import { changed } from "./testing";

const invoke = vi.fn<(command: string, args?: Record<string, unknown>) => Promise<unknown>>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>) => invoke(command, args),
  Channel: class<T> {
    constructor(readonly onmessage: (message: T) => void) {}
  },
}));

const { tauriTransport } = await import("./tauri");

beforeEach(() => {
  invoke.mockReset();
});

describe("the app's transport", () => {
  it("sends every operation through the one op command, as { op, input }", async () => {
    invoke.mockResolvedValue({ name: "Personal" });
    const transport = tauriTransport();
    await expect(transport.call("workspace.get", undefined)).resolves.toEqual({ name: "Personal" });
    await transport.call("settings.update", { week_start: "sunday" });
    expect(invoke.mock.calls).toEqual([
      ["op", { op: "workspace.get", input: {} }],
      ["op", { op: "settings.update", input: { week_start: "sunday" } }],
    ]);
  });

  it("hands the app a channel and passes its events on until stopped", async () => {
    invoke.mockResolvedValue(false);
    const got: Changed[] = [];
    const stop = await tauriTransport().subscribe((event) => got.push(event));
    const [command, args] = invoke.mock.calls[0] ?? [];
    expect(command).toBe("subscribe");
    const channel = args?.channel as { onmessage: (event: Changed) => void };
    channel.onmessage(changed(1, "settings", ["week_start"]));
    stop();
    channel.onmessage(changed(2, "settings", ["week_start"]));
    expect(got.map((event) => event.seq)).toEqual([1]);
  });
});
