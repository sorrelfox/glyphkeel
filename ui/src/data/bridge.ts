// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Development only. The page reaches this file only behind `__BRIDGE__`, which
// is true in `vite --mode bridge` alone; every other build drops it, and
// `pnpm check:bundle` fails if a WebSocket ever reaches the release bundle.

import { type Changed } from "../generated/ops";
import { type Transport } from "./transport";

/** Where the development bridge listens, from the page's URL: `?bridge=<port>&token=<token>`. */
export interface BridgeAddress {
  port: number;
  token: string;
}

export function bridgeAddress(search: string, ports: readonly [number, number] = __BRIDGE_PORTS__): BridgeAddress | null {
  const params = new URLSearchParams(search);
  const port = Number(params.get("bridge"));
  const token = params.get("token");
  if (!Number.isInteger(port) || port < ports[0] || port > ports[1] || !token) {
    return null;
  }
  return { port, token };
}

/** The parts of a WebSocket the transport uses, so tests can stand in for it. */
export interface Socket {
  send(data: string): void;
  onopen: (() => void) | null;
  onerror: (() => void) | null;
  onclose: (() => void) | null;
  onmessage: ((message: { data: unknown }) => void) | null;
}

interface Reply {
  id?: unknown;
  ok?: unknown;
  err?: unknown;
  ipc?: unknown;
  event?: Changed;
}

function openSocket(url: string): Socket {
  // eslint-disable-next-line no-restricted-globals -- the bridge is a loopback WebSocket, in development only
  const ws = new WebSocket(url);
  const socket: Socket = {
    send: (data) => {
      ws.send(data);
    },
    onopen: null,
    onerror: null,
    onclose: null,
    onmessage: null,
  };
  ws.onopen = () => socket.onopen?.();
  ws.onerror = () => socket.onerror?.();
  ws.onclose = () => socket.onclose?.();
  ws.onmessage = (message) => socket.onmessage?.({ data: message.data });
  return socket;
}

/**
 * The bridge: the same operations and change events as the app, over a
 * WebSocket on loopback. Resolves once connected; rejects if the bridge
 * refuses the page (wrong token or origin) or isn't running.
 */
export function bridgeTransport(address: BridgeAddress, open: (url: string) => Socket = openSocket): Promise<Transport> {
  const socket = open(`ws://127.0.0.1:${String(address.port)}/?token=${encodeURIComponent(address.token)}`);
  const pending = new Map<number, { resolve: (value: unknown) => void; reject: (error: unknown) => void }>();
  const listeners = new Set<(event: Changed) => void>();
  let closed = false;
  let next = 1;

  socket.onmessage = ({ data }) => {
    if (typeof data !== "string") {
      return;
    }
    const reply = JSON.parse(data) as Reply;
    if (reply.event) {
      for (const listener of listeners) {
        listener(reply.event);
      }
      return;
    }
    const waiting = typeof reply.id === "number" ? pending.get(reply.id) : undefined;
    if (!waiting || typeof reply.id !== "number") {
      return;
    }
    pending.delete(reply.id);
    if ("ok" in reply) {
      waiting.resolve(reply.ok);
    } else if ("err" in reply) {
      waiting.reject(reply.err);
    } else {
      waiting.reject(typeof reply.ipc === "string" ? reply.ipc : "the bridge sent an unreadable answer");
    }
  };
  socket.onclose = () => {
    closed = true;
    for (const waiting of pending.values()) {
      waiting.reject("the development bridge closed the connection");
    }
    pending.clear();
  };

  const transport: Transport = {
    call(op, input) {
      if (closed) {
        return Promise.reject(new Error("the development bridge is not connected"));
      }
      const id = next++;
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        socket.send(JSON.stringify({ id, op, input: input ?? {} }));
      });
    },
    subscribe(onEvent) {
      listeners.add(onEvent);
      return Promise.resolve(() => {
        listeners.delete(onEvent);
      });
    },
  };

  return new Promise((resolve, reject) => {
    socket.onopen = () => {
      resolve(transport);
    };
    socket.onerror = () => {
      reject(new Error("the development bridge refused the page or isn't running"));
    };
  });
}
