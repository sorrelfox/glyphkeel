// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Changed } from "../generated/ops";
import { bridgeAddress, bridgeTransport, type Socket } from "./bridge";
import { changed } from "./testing";

class FakeSocket implements Socket {
  sent: { id: number; op: string; input: unknown }[] = [];
  url = "";
  onopen: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onmessage: ((message: { data: unknown }) => void) | null = null;

  send(data: string) {
    this.sent.push(JSON.parse(data) as FakeSocket["sent"][number]);
  }

  reply(message: unknown) {
    this.onmessage?.({ data: JSON.stringify(message) });
  }
}

function open() {
  const socket = new FakeSocket();
  const transport = bridgeTransport({ port: 5321, token: "t o/k" }, (url) => {
    socket.url = url;
    return socket;
  });
  return { socket, transport };
}

describe("bridgeAddress", () => {
  it("reads the port and token from the page's URL, within the bridge's ports", () => {
    expect(bridgeAddress("?bridge=5321&token=abc")).toEqual({ port: 5321, token: "abc" });
    expect(bridgeAddress("?bridge=8080&token=abc")).toBeNull();
    expect(bridgeAddress("?bridge=5321")).toBeNull();
    expect(bridgeAddress("")).toBeNull();
  });
});

describe("the bridge's transport", () => {
  it("connects to loopback with the token", async () => {
    const { socket, transport } = open();
    socket.onopen?.();
    await transport;
    expect(socket.url).toBe("ws://127.0.0.1:5321/?token=t%20o%2Fk");
  });

  it("matches answers to calls, and passes the engine's errors and events on", async () => {
    const { socket, transport: pending } = open();
    socket.onopen?.();
    const transport = await pending;
    const events: Changed[] = [];
    await transport.subscribe((event) => events.push(event));

    const first = transport.call("workspace.get", undefined);
    const second = transport.call("settings.update", { trash_days: 0 });
    expect(socket.sent).toEqual([
      { id: 1, op: "workspace.get", input: {} },
      { id: 2, op: "settings.update", input: { trash_days: 0 } },
    ]);
    socket.reply({ event: changed(7, "settings", ["week_start"]) });
    socket.reply({ id: 2, err: { code: "invalid", message: "Too few." } });
    socket.reply({ id: 1, ok: { name: "Personal" } });
    await expect(first).resolves.toEqual({ name: "Personal" });
    await expect(second).rejects.toEqual({ code: "invalid", message: "Too few." });
    expect(events.map((event) => event.seq)).toEqual([7]);
  });

  it("fails calls in flight when the bridge goes away, and later calls at once", async () => {
    const { socket, transport: pending } = open();
    socket.onopen?.();
    const transport = await pending;
    const inFlight = transport.call("workspace.get", undefined);
    socket.onclose?.();
    await expect(inFlight).rejects.toBe("the development bridge closed the connection");
    await expect(transport.call("workspace.get", undefined)).rejects.toThrow("not connected");
  });

  it("rejects when the bridge refuses the page", async () => {
    const { socket, transport } = open();
    socket.onerror?.();
    await expect(transport).rejects.toThrow("refused");
  });
});
