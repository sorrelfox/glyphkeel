// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// For tests: an engine stand-in that answers from handlers and sends change
// events on demand. Nothing in the app imports it.

import { type Changed, type OpId, type OpMap } from "../generated/ops";
import { createClient, type Client, type Transport } from "./transport";

type Handlers = { [K in OpId]?: (input: OpMap[K]["input"]) => OpMap[K]["output"] | Promise<OpMap[K]["output"]> };

export interface FakeEngine {
  client: Client;
  /** Every call, in order. */
  calls: { op: string; input: unknown }[];
  /** Sends a change event to every subscriber. */
  emit: (event: Changed) => void;
  subscribers: () => number;
}

/**
 * A rejection as the transports deliver it: the engine's errors are plain
 * objects, Tauri's own are strings.
 */
export function rejectWith(reason: unknown): Promise<never> {
  // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors -- the engine's errors are plain objects
  return Promise.reject(reason);
}

export function fakeEngine(handlers: Handlers): FakeEngine {
  const calls: FakeEngine["calls"] = [];
  const listeners = new Set<(event: Changed) => void>();
  const transport: Transport = {
    call(op, input) {
      calls.push({ op, input });
      const handler = handlers[op as OpId] as ((input: unknown) => unknown) | undefined;
      if (!handler) {
        return rejectWith({ code: "not_found", message: `There is no operation "${op}".` });
      }
      return Promise.resolve(handler(input));
    },
    subscribe(onEvent) {
      listeners.add(onEvent);
      return Promise.resolve(() => {
        listeners.delete(onEvent);
      });
    },
  };
  return {
    client: createClient(transport),
    calls,
    emit: (event) => {
      for (const listener of listeners) {
        listener(event);
      }
    },
    subscribers: () => listeners.size,
  };
}

/** A change event for `entity`. */
export function changed(seq: number, entity: string, fields: string[]): Changed {
  return {
    seq,
    group: `group-${String(seq)}`,
    origin: { via: "cli", actor: { kind: "user", name: "You" } },
    changes: [{ entity, id: "w-1", fields }],
  };
}
