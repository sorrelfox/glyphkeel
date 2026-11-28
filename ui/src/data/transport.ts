// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { makeOps, type Call, type CallError, type Changed, type ErrorCode, type OpError, type Ops } from "../generated/ops";

/**
 * How the page reaches the engine: the app's `op` command, the development
 * bridge, or nothing. The rest of the UI sees only the typed client on top.
 */
export interface Transport {
  /** Runs an operation; rejects with whatever the transport got back. */
  call(op: string, input: unknown): Promise<unknown>;
  /**
   * Starts the change events of this window. Resolves once they flow, with a
   * function that stops them. Subscribing again (after a reload) replaces the
   * earlier subscription.
   */
  subscribe(onEvent: (event: Changed) => void): Promise<() => void>;
}

/**
 * Every code the engine sends; anything else came from the transport itself.
 * A record, so that a code added to the catalog fails to compile here.
 */
const ENGINE_CODES: Readonly<Record<ErrorCode, true>> = {
  invalid: true,
  not_found: true,
  conflict: true,
  forbidden: true,
  limit: true,
  rate_limited: true,
  pending_approval: true,
  busy: true,
  workspace_upgraded: true,
  workspace_too_new: true,
  storage: true,
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isOpError(value: unknown): value is OpError {
  return (
    isRecord(value) &&
    typeof value.message === "string" &&
    typeof value.code === "string" &&
    Object.hasOwn(ENGINE_CODES, value.code)
  );
}

/**
 * The engine's errors pass as they are. Everything else (Tauri's own
 * refusals, which arrive as plain strings, a closed bridge, a page without
 * an engine) becomes the client-only code `ipc`.
 */
export function toCallError(error: unknown): CallError {
  if (isOpError(error)) {
    return error;
  }
  if (typeof error === "string") {
    return { code: "ipc", message: error };
  }
  if (error instanceof Error) {
    return { code: "ipc", message: error.message };
  }
  if (isRecord(error) && typeof error.message === "string") {
    return { code: "ipc", message: error.message };
  }
  return { code: "ipc", message: error === undefined ? "no answer" : JSON.stringify(error) };
}

/** A failed operation, as the UI throws and catches it. */
export class OpFailure extends Error {
  readonly op: string;
  readonly error: CallError;

  constructor(op: string, error: CallError) {
    super(`${op}: ${error.code}: ${error.message}`);
    this.name = "OpFailure";
    this.op = op;
    this.error = error;
  }
}

/** The typed client: `call` for any operation, `ops` with one function per operation. */
export interface Client {
  call: Call;
  ops: Ops;
  subscribe: Transport["subscribe"];
}

export function createClient(transport: Transport): Client {
  const call: Call = async (op, input) => {
    try {
      // The engine checks the output's shape; the generated types describe it.
      return (await transport.call(op, input)) as never;
    } catch (error: unknown) {
      throw new OpFailure(op, toCallError(error));
    }
  };
  return { call, ops: makeOps(call), subscribe: (onEvent) => transport.subscribe(onEvent) };
}

/** A transport that arrives later, such as one loaded on demand. */
export function deferredTransport(pending: Promise<Transport>): Transport {
  // A failure surfaces in every call; it isn't an unhandled rejection.
  pending.catch(() => undefined);
  return {
    call: async (op, input) => (await pending).call(op, input),
    subscribe: async (onEvent) => (await pending).subscribe(onEvent),
  };
}

/** No engine to talk to: a plain browser without the bridge. */
export function unavailableTransport(reason: string): Transport {
  return {
    call: () => Promise.reject(new Error(reason)),
    subscribe: () => Promise.resolve(() => undefined),
  };
}
