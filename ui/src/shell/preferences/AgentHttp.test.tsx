// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ToastProvider } from "../../components/Toasts";
import { DataProvider } from "../../data/DataProvider";
import { fakeEngine } from "../../data/testing";
import { I18n } from "../../i18n/I18n";
import { httpSnippet, updateAgentHttp } from "../../ipc/agentHttp";
import { HttpGroup } from "./AgentsPage";

const TOKEN = "ab".repeat(32);

const app = vi.hoisted(() => ({
  inside: false,
  enabled: false,
  token: undefined as string | undefined,
  calls: [] as { command: string; args: unknown }[],
}));

vi.mock("../../ipc/files", async (original) => ({
  ...(await original<typeof import("../../ipc/files")>()),
  inApp: () => app.inside,
}));

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (command: string, args: { patch?: { enabled?: boolean } } = {}) => {
    app.calls.push({ command, args });
    if (command === "update_agent_http" && args.patch?.enabled !== undefined) {
      app.enabled = args.patch.enabled;
      app.token ??= TOKEN;
    } else if (command === "regenerate_agent_token") {
      app.token = "cd".repeat(32);
    }
    return Promise.resolve({
      enabled: app.enabled,
      ...(app.token === undefined ? {} : { token: app.token }),
      ...(app.enabled ? { url: "http://127.0.0.1:41000/mcp" } : {}),
    });
  },
}));

afterEach(() => {
  cleanup();
  app.inside = false;
  app.enabled = false;
  app.token = undefined;
  app.calls = [];
  localStorage.clear();
});

function setUp() {
  const engine = fakeEngine({});
  render(
    <I18n locale="en-US">
      <ToastProvider>
        <DataProvider client={engine.client}>
          <HttpGroup />
        </DataProvider>
      </ToastProvider>
    </I18n>,
  );
}

describe("Preferences › Agents & CLI › Local HTTP endpoint", () => {
  it("turns on in the app, shows the address and hides the token until asked", async () => {
    app.inside = true;
    setUp();
    const serve = await screen.findByRole("switch", { name: "Serve agents over HTTP" });
    expect(screen.queryByTestId("agent-http-url")).toBeNull();
    fireEvent.click(serve);
    expect((await screen.findByTestId("agent-http-url")).textContent).toBe("http://127.0.0.1:41000/mcp");
    expect(app.calls.at(-1)).toEqual({ command: "update_agent_http", args: { patch: { enabled: true } } });
    expect(screen.getByTestId("agent-http-token").textContent).not.toContain(TOKEN);
    fireEvent.click(screen.getByRole("button", { name: "Show the token" }));
    expect(screen.getByTestId("agent-http-token").textContent).toBe(TOKEN);
    fireEvent.click(screen.getByRole("button", { name: "New token" }));
    await waitFor(() => {
      expect(screen.getByTestId("agent-http-token").textContent).toBe("cd".repeat(32));
    });
  });

  it("outside the app keeps the set-up in the page and says nothing listens", async () => {
    setUp();
    expect(await screen.findByText("Only the app serves agents over HTTP. Here the set-up is shown, but nothing listens.")).toBeDefined();
    const on = await updateAgentHttp({ enabled: true });
    expect(on.token).toMatch(/^[0-9a-f]{64}$/u);
    expect(on.url).toBe("http://127.0.0.1:49152/mcp");
    const port = await updateAgentHttp({ port: 50000 });
    expect(port.url).toBe("http://127.0.0.1:50000/mcp");
    expect(port.token).toBe(on.token);
    await expect(updateAgentHttp({ port: 80 })).rejects.toThrow();
    expect(app.calls).toEqual([]);
  });

  it("gives clients the address and the token as a bearer header", () => {
    const snippet = JSON.parse(httpSnippet("http://127.0.0.1:41000/mcp", TOKEN)) as {
      mcpServers: { glyphkeel: { type: string; url: string; headers: Record<string, string> } };
    };
    expect(snippet.mcpServers.glyphkeel).toEqual({
      type: "http",
      url: "http://127.0.0.1:41000/mcp",
      headers: { Authorization: `Bearer ${TOKEN}` },
    });
  });
});
