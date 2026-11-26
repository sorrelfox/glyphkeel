// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ToastProvider } from "../../components/Toasts";
import { DataProvider } from "../../data/DataProvider";
import { fakeEngine } from "../../data/testing";
import { I18n } from "../../i18n/I18n";
import { FilesGroup } from "./AgentsPage";

const EXPORTS = "/home/me/.local/share/glyphkeel/workspaces/Personal/exports";

const app = vi.hoisted(() => ({
  inside: false,
  folders: [] as string[],
  calls: [] as { command: string; args: unknown }[],
}));

vi.mock("../../ipc/files", async (original) => ({
  ...(await original<typeof import("../../ipc/files")>()),
  inApp: () => app.inside,
}));

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (command: string, args: { folder?: string } = {}) => {
    app.calls.push({ command, args });
    if (command === "add_agent_folder") {
      app.folders = [...app.folders, "/home/me/For agents"];
    } else if (command === "remove_agent_folder") {
      app.folders = app.folders.filter((folder) => folder !== args.folder);
    }
    return Promise.resolve({
      background: true,
      notifications: true,
      autostart: false,
      safe_rendering: false,
      safe_rendering_in_use: false,
      time_zone: "system",
      exports_folder: EXPORTS,
      agent_folders: app.folders,
    });
  },
}));

afterEach(() => {
  cleanup();
  app.inside = false;
  app.folders = [];
  app.calls = [];
});

function setUp() {
  const engine = fakeEngine({});
  render(
    <I18n locale="en-US">
      <ToastProvider>
        <DataProvider client={engine.client}>
          <FilesGroup />
        </DataProvider>
      </ToastProvider>
    </I18n>,
  );
}

describe("Preferences › Agents & CLI › Files agents may use", () => {
  it("shows the exports folder and adds folders only through the app's chooser", async () => {
    app.inside = true;
    setUp();
    expect(await screen.findByText(EXPORTS)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Add folder…" }));
    expect(await screen.findByText("/home/me/For agents")).toBeDefined();
    // The page never names a folder to add.
    expect(app.calls.find((call) => call.command === "add_agent_folder")?.args).toEqual({});
    fireEvent.click(screen.getByRole("button", { name: /^Stop agents from using \W?\/home\/me\/For agents\W?$/u }));
    await waitFor(() => {
      expect(screen.queryByText("/home/me/For agents")).toBeNull();
    });
    expect(app.calls.at(-1)).toEqual({ command: "remove_agent_folder", args: { folder: "/home/me/For agents" } });
  });

  it("can't add folders outside the app", async () => {
    setUp();
    expect(await screen.findByText("Folders can be added in the app, which has the folder chooser.")).toBeDefined();
    expect(screen.queryByRole("button", { name: "Add folder…" })).toBeNull();
    expect(screen.queryAllByTestId("agent-folder")).toEqual([]);
  });
});
