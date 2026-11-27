// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ToastProvider } from "../../components/Toasts";
import { DataProvider } from "../../data/DataProvider";
import { fakeEngine } from "../../data/testing";
import { I18n } from "../../i18n/I18n";
import { changeEncryption, resetFakeLock } from "../../ipc/lock";
import { ExportDialog } from "./ExportDialog";
import { ImportDialog } from "./ImportDialog";

const app = vi.hoisted(() => ({
  inside: false,
  calls: [] as { command: string; args: Record<string, unknown> }[],
}));

vi.mock("../../ipc/files", async (original) => ({
  ...(await original<typeof import("../../ipc/files")>()),
  inApp: () => app.inside,
}));

const CHECKED = {
  format: "glyphkeel-archive",
  format_version: 1,
  app_version: "0.1.9",
  dry_run: true,
  projects: [{ key: "ZZ", from_key: "ZZ", name: "Files", id: "p-1" }],
  counts: { projects: 1, tasks: 2, comments: 0, time_entries: 0, revisions: 0, wiki_pages: 0, attachments: 1, files: 1, activity: 0, agent_sessions: 0 },
};

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (command: string, args: Record<string, unknown> = {}) => {
    app.calls.push({ command, args });
    if (command === "choose_import") {
      return Promise.resolve({ token: "t-1", name: "export.zip" });
    }
    if (command === "import_archive") {
      if (args.passphrase !== "the passphrase") {
        // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors -- the engine's errors are plain objects
        return Promise.reject({
          code: "forbidden",
          message: "export.zip is an encrypted export.",
          details: { passphrase: "export", message_id: "transfer-passphrase-needed" },
        });
      }
      return Promise.resolve(CHECKED);
    }
    return Promise.reject(new Error(`unexpected command ${command}`));
  },
}));

afterEach(() => {
  cleanup();
  resetFakeLock();
  app.inside = false;
  app.calls = [];
});

function show(dialog: React.ReactNode) {
  const engine = fakeEngine({});
  render(
    <I18n locale="en-US">
      <ToastProvider>
        <DataProvider client={engine.client}>{dialog}</DataProvider>
      </ToastProvider>
    </I18n>,
  );
}

describe("exports of an encrypted workspace", () => {
  it("are encrypted unless the person turns it off, and only then may be a folder", async () => {
    await changeEncryption({ encrypt: true, passphrase: "a test passphrase" });
    show(<ExportDialog isOpen onOpenChange={() => undefined} />);
    const encrypt = await screen.findByRole<HTMLInputElement>("checkbox", { name: "Encrypt this export" });
    expect(encrypt.checked).toBe(true);
    expect(screen.queryByText("A folder")).toBeNull();
    fireEvent.click(encrypt);
    expect(encrypt.checked).toBe(false);
    expect(await screen.findByText("A folder")).toBeDefined();
  });

  it("offer no encryption in a plain workspace", async () => {
    show(<ExportDialog isOpen onOpenChange={() => undefined} />);
    expect(await screen.findByText("A folder")).toBeDefined();
    expect(screen.queryByRole("checkbox", { name: "Encrypt this export" })).toBeNull();
  });
});

describe("importing an encrypted export", () => {
  it("asks for its passphrase, says when it is wrong, and checks it with the right one", async () => {
    app.inside = true;
    show(<ImportDialog isOpen onOpenChange={() => undefined} />);
    fireEvent.click(await screen.findByRole("button", { name: /\.zip/ }));
    const field = await screen.findByLabelText("Passphrase");
    expect(screen.queryByRole("alert")).toBeNull();
    fireEvent.change(field, { target: { value: "not it" } });
    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(await screen.findByRole("alert")).toBeDefined();
    fireEvent.change(await screen.findByLabelText("Passphrase"), { target: { value: "the passphrase" } });
    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    await waitFor(() => {
      expect(screen.queryByLabelText("Passphrase")).toBeNull();
    });
    const sent = app.calls.filter((call) => call.command === "import_archive").map((call) => call.args.passphrase);
    expect(sent).toEqual([null, "not it", "the passphrase"]);
  });
});
