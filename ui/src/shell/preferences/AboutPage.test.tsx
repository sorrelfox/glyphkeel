// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ToastProvider } from "../../components/Toasts";
import { DataProvider } from "../../data/DataProvider";
import { fakeEngine } from "../../data/testing";
import { I18n } from "../../i18n/I18n";
import { AboutPage } from "./AboutPage";

const app = vi.hoisted(() => ({
  inside: false,
  calls: [] as { command: string; args: unknown }[],
  answer: (args: { which: string }): Promise<string> => Promise.resolve(`text of ${args.which}`),
}));

vi.mock("../../ipc/files", async (original) => ({
  ...(await original<typeof import("../../ipc/files")>()),
  inApp: () => app.inside,
}));

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (command: string, args: { which: string }) => {
    app.calls.push({ command, args });
    return app.answer(args);
  },
}));

afterEach(() => {
  cleanup();
  app.inside = false;
  app.calls = [];
  app.answer = (args) => Promise.resolve(`text of ${args.which}`);
});

function setUp() {
  const engine = fakeEngine({});
  render(
    <I18n locale="en-US">
      <ToastProvider>
        <DataProvider client={engine.client}>
          <AboutPage />
        </DataProvider>
      </ToastProvider>
    </I18n>,
  );
}

describe("Preferences › About", () => {
  it("shows the version, the licence and where the source code is", () => {
    setUp();
    expect(screen.getByTestId("about-version").textContent).toBe(__VERSION__);
    expect(screen.getByText(/GNU General Public License, version 3 or later/u)).toBeDefined();
    expect(screen.getByRole("button", { name: "Open in browser" })).toBeDefined();
    expect(screen.getByText(/^Copyright © \S*2026\S* Glyphkeel contributors$/u)).toBeDefined();
  });

  it("reads the notices from the app, names only which text, and goes back", async () => {
    app.inside = true;
    setUp();
    fireEvent.click(screen.getByRole("button", { name: "Show notices" }));
    const text = await screen.findByTestId("legal-text");
    expect(text.textContent).toBe("text of notices");
    expect(text.getAttribute("tabindex")).toBe("0");
    expect(app.calls).toEqual([{ command: "legal_text", args: { which: "notices" } }]);
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    fireEvent.click(screen.getByRole("button", { name: "Show licence" }));
    expect((await screen.findByTestId("legal-text")).textContent).toBe("text of licence");
    expect(screen.getByRole("heading", { name: "GNU General Public License" })).toBeDefined();
  });

  it("says where the file is when the app has no copy, and where a plain browser can't read it", async () => {
    app.inside = true;
    app.answer = () => Promise.reject(new Error("THIRD-PARTY-NOTICES.txt did not come with this copy of the app."));
    setUp();
    fireEvent.click(screen.getByRole("button", { name: "Show notices" }));
    expect((await screen.findByRole("alert")).textContent).toContain("THIRD-PARTY-NOTICES.txt");
    cleanup();

    app.inside = false;
    setUp();
    fireEvent.click(screen.getByRole("button", { name: "Show notices" }));
    expect(await screen.findByText(/^Shown in the desktop app\./u)).toBeDefined();
    expect(app.calls).toHaveLength(1);
  });
});
