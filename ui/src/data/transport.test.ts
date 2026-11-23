// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { rejectWith } from "./testing";
import { createClient, deferredTransport, OpFailure, toCallError, unavailableTransport, type Transport } from "./transport";

function rejecting(reason: unknown): Transport {
  return { call: () => rejectWith(reason), subscribe: () => Promise.resolve(() => undefined) };
}

async function failureOf(promise: Promise<unknown>): Promise<OpFailure> {
  try {
    await promise;
  } catch (error: unknown) {
    if (error instanceof OpFailure) {
      return error;
    }
    throw error;
  }
  throw new Error("expected the call to fail");
}

describe("toCallError", () => {
  it("passes the engine's errors through with their details", () => {
    const error = { code: "not_found", message: "There is no operation.", hint: "Check the name.", details: { choices: ["workspace.get"] } };
    expect(toCallError(error)).toBe(error);
    for (const code of ["invalid", "conflict", "forbidden", "limit", "busy", "storage"]) {
      expect(toCallError({ code, message: "m" }).code).toBe(code);
    }
  });

  it("turns everything else into the client-only code ipc", () => {
    expect(toCallError("Command op not allowed by ACL")).toEqual({ code: "ipc", message: "Command op not allowed by ACL" });
    expect(toCallError(new Error("socket closed"))).toEqual({ code: "ipc", message: "socket closed" });
    expect(toCallError({ code: "made_up", message: "x" })).toEqual({ code: "ipc", message: "x" });
    expect(toCallError({ code: "invalid" }).code).toBe("ipc");
    expect(toCallError(undefined).code).toBe("ipc");
    expect(toCallError(42)).toEqual({ code: "ipc", message: "42" });
  });
});

describe("createClient", () => {
  it("calls the transport with the operation and its input", async () => {
    const seen: unknown[] = [];
    const client = createClient({
      call: (op, input) => {
        seen.push([op, input]);
        return Promise.resolve({ name: "Personal" });
      },
      subscribe: () => Promise.resolve(() => undefined),
    });
    const info = await client.ops.workspace.get();
    expect(info.name).toBe("Personal");
    await client.ops.settings.update({ week_start: "sunday" });
    expect(seen).toEqual([
      ["workspace.get", {}],
      ["settings.update", { week_start: "sunday" }],
    ]);
  });

  it("rejects with a typed failure naming the operation", async () => {
    const engine = await failureOf(createClient(rejecting({ code: "invalid", message: "Too many.", details: { path: "trash_days" } })).call("settings.update", { trash_days: 0 }));
    expect(engine.op).toBe("settings.update");
    expect(engine.error).toEqual({ code: "invalid", message: "Too many.", details: { path: "trash_days" } });

    const tauri = await failureOf(createClient(rejecting("invalid args `input` for command `op`")).ops.workspace.get());
    expect(tauri.error.code).toBe("ipc");
  });

  it("fails every call when there is no engine", async () => {
    const failure = await failureOf(createClient(unavailableTransport("no engine")).ops.workspace.get());
    expect(failure.error).toEqual({ code: "ipc", message: "no engine" });
  });

  it("waits for a transport that arrives later, and reports one that never does", async () => {
    let arrive: (transport: Transport) => void = () => undefined;
    const later = createClient(deferredTransport(new Promise((resolve) => (arrive = resolve))));
    const answer = later.ops.workspace.get();
    arrive({ call: () => Promise.resolve({ name: "Later" }), subscribe: () => Promise.resolve(() => undefined) });
    expect((await answer).name).toBe("Later");

    const never = createClient(deferredTransport(Promise.reject(new Error("bridge refused"))));
    expect((await failureOf(never.ops.workspace.get())).error).toEqual({ code: "ipc", message: "bridge refused" });
  });
});
