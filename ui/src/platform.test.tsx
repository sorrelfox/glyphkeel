// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { parseChord } from "./actions/keymap";
import { chordLabel } from "./components/Kbd";
import MarkdownEditor from "./editor/MarkdownEditor";
import { I18n, useT } from "./i18n/I18n";
import { applyPlatform, isPlatform, platform, webViewChecksSpelling } from "./platform";
import { autostartMessages, shortcutStateMessage } from "./shell/Preferences";
import { cliMessages } from "./shell/preferences/AgentsPage";
import { clientFile, MCP_CLIENTS } from "./shell/preferences/clients";
import { type ShortcutState } from "./ipc/quickAdd";
import { dragRegion, readStart } from "./start";

afterEach(() => {
  cleanup();
  applyPlatform("linux", document.documentElement);
});

function windowWith(start: unknown): Window {
  return { __GLYPHKEEL_START__: start, navigator: { language: "en-US" } } as unknown as Window;
}

describe("the platform", () => {
  it("comes from the start data, and counts as Linux when absent or unknown", () => {
    expect(readStart(windowWith({ platform: "windows" })).platform).toBe("windows");
    expect(readStart(windowWith({ platform: "linux" })).platform).toBe("linux");
    expect(readStart(windowWith({ platform: "macos" })).platform).toBeUndefined();
    expect(readStart(windowWith(undefined)).platform).toBeUndefined();
    expect(isPlatform("windows")).toBe(true);
    expect(isPlatform(7)).toBe(false);
    expect(platform()).toBe("linux");
  });

  it("marks <html> with one platform class", () => {
    const root = document.createElement("html");
    applyPlatform("windows", root);
    expect([...root.classList]).toEqual(["platform-windows"]);
    expect(platform()).toBe("windows");
    applyPlatform("linux", root);
    expect([...root.classList]).toEqual(["platform-linux"]);
  });

  it("lets only the app's own header bar move the window", () => {
    expect(dragRegion("app")).toEqual({ "data-tauri-drag-region": true });
    expect(dragRegion("system")).toEqual({});
  });
});

function Label({ chord }: { chord: string }) {
  const t = useT();
  return <span data-testid="label">{chordLabel(parseChord(chord), t)}</span>;
}

function labelOf(chord: string): string | null {
  render(
    <I18n locale="en-US">
      <Label chord={chord} />
    </I18n>,
  );
  const text = screen.getByTestId("label").textContent;
  cleanup();
  return text;
}

describe("key names", () => {
  it("say Ctrl and Alt everywhere, and Win instead of Super on Windows", () => {
    expect(labelOf("Ctrl+Alt+Space")).toBe("Ctrl Alt Space");
    expect(labelOf("Super+Alt+N")).toBe("Alt Super N");
    applyPlatform("windows");
    expect(labelOf("Ctrl+Alt+Space")).toBe("Ctrl Alt Space");
    expect(labelOf("Super+Alt+N")).toBe("Alt Win N");
    expect(labelOf("Meta+K")).toBe("Win K");
  });
});

describe("Preferences on Windows", () => {
  const states: readonly ShortcutState[] = ["off", "waiting", "portal", "grab", "manual", "refused"];

  it("talks of signing in, and keeps Linux's wording elsewhere", () => {
    expect(autostartMessages("windows")).toEqual({ label: "preferences-autostart-windows", about: "preferences-autostart-windows-about" });
    expect(autostartMessages("linux")).toEqual({ label: "preferences-autostart", about: "preferences-autostart-about" });
  });

  it("never mentions the desktop's portal or keyboard settings for the shortcut", () => {
    for (const state of states) {
      const message = shortcutStateMessage(state, "windows");
      expect(message.needsCommand).toBe(false);
      expect(["preferences-shortcut-first-time-windows", "preferences-shortcut-state-grab", "preferences-shortcut-state-taken"]).toContain(message.id);
    }
    expect(shortcutStateMessage("manual", "linux")).toEqual({ id: "preferences-shortcut-state-manual", needsCommand: true });
    expect(shortcutStateMessage("off", "linux").id).toBe("preferences-shortcut-first-time");
  });

  it("names the installer, not a package manager, for the command-line tool", () => {
    expect(cliMessages("windows")).toEqual({ bundledNotOnPath: "agents-cli-bundled-not-on-path-windows", missing: "agents-cli-missing-windows" });
    expect(cliMessages("linux")).toEqual({ bundledNotOnPath: "agents-cli-bundled-not-on-path", missing: "agents-cli-missing" });
  });

  it("names clients' files as Windows writes them", () => {
    const codex = MCP_CLIENTS.find((client) => client.id === "codex");
    const claudeCode = MCP_CLIENTS.find((client) => client.id === "claude-code");
    expect(codex && clientFile(codex, "windows")).toBe("%USERPROFILE%\\.codex\\config.toml");
    expect(codex && clientFile(codex, "linux")).toBe("~/.codex/config.toml");
    expect(claudeCode && clientFile(claudeCode, "windows")).toBeUndefined();
  });

  it("has every Windows message in the English bundle", () => {
    const ids = [
      ...Object.values(autostartMessages("windows")),
      ...states.map((state) => shortcutStateMessage(state, "windows").id),
      ...Object.values(cliMessages("windows")),
      "key-windows",
    ];
    function Probe() {
      const t = useT();
      return <span data-testid="missing">{ids.filter((id) => t.has?.(id) !== true).join(",")}</span>;
    }
    render(
      <I18n locale="en-US">
        <Probe />
      </I18n>,
    );
    expect(screen.getByTestId("missing").textContent).toBe("");
  });
});

describe("spelling", () => {
  it("leaves the web view's checker off on Linux and turns it on on Windows", () => {
    expect(webViewChecksSpelling()).toBe(false);
    render(
      <I18n locale="en-US">
        <MarkdownEditor label="Linux" value="Teh text" />
      </I18n>,
    );
    expect(screen.getByRole("textbox", { name: "Linux" }).getAttribute("spellcheck")).toBe("false");
    cleanup();
    applyPlatform("windows");
    expect(webViewChecksSpelling()).toBe(true);
    render(
      <I18n locale="en-US">
        <MarkdownEditor label="Windows" value="Teh text" />
      </I18n>,
    );
    expect(screen.getByRole("textbox", { name: "Windows" }).getAttribute("spellcheck")).toBe("true");
  });

  it("keeps a checker a screen passes, on every platform", () => {
    applyPlatform("windows");
    render(
      <I18n locale="en-US">
        <MarkdownEditor label="Given" value="text" spellChecker={null} />
      </I18n>,
    );
    expect(screen.getByRole("textbox", { name: "Given" }).getAttribute("spellcheck")).toBe("false");
  });
});
