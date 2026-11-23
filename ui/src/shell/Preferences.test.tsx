// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { type Appearance } from "../appearance/appearance";
import { AppearanceProvider } from "../appearance/AppearanceProvider";
import { ToastProvider } from "../components/Toasts";
import { DataProvider } from "../data/DataProvider";
import { settings, workspaceInfo } from "../data/fixtures";
import { changed, fakeEngine, rejectWith } from "../data/testing";
import { type Settings } from "../generated/ops";
import { I18n } from "../i18n/I18n";
import { Preferences } from "./Preferences";
import { StatusBar } from "./StatusBar";

afterEach(cleanup);

function setUp(options: { failUpdate?: boolean; onAppearance?: (appearance: Appearance) => void } = {}) {
  const onAppearance = options.onAppearance ?? (() => undefined);
  let current: Settings = settings();
  const engine = fakeEngine({
    "workspace.get": () => workspaceInfo({ settings: current }),
    "settings.get": () => current,
    "settings.update": (patch) => {
      if (options.failUpdate) {
        return rejectWith({ code: "busy", message: "Another program is writing.", hint: "Try again in a moment." });
      }
      current = { ...current, ...(patch.week_start ? { week_start: patch.week_start } : {}) };
      return { changed: ["week_start"], group: "g-1", settings: current };
    },
    "history.undo": () => ({ group: "g-2", op: "settings.update", target: "g-1" }),
  });
  render(
    <AppearanceProvider onChange={onAppearance}>
      <I18n locale="en-US">
        <ToastProvider>
          <DataProvider client={engine.client}>
            <StatusBar info={null} />
            <Preferences isOpen onOpenChange={() => undefined} />
          </DataProvider>
        </ToastProvider>
      </I18n>
    </AppearanceProvider>,
  );
  return {
    engine,
    /** Another program changes the setting and the engine announces it. */
    changeElsewhere: (week_start: Settings["week_start"]) => {
      current = { ...current, week_start };
      act(() => {
        engine.emit(changed(2, "settings", ["week_start"]));
      });
    },
  };
}

function selectedDay() {
  const group = screen.getByRole("radiogroup", { name: "First day of the week" });
  return within(group)
    .getAllByRole("radio")
    .find((radio) => radio.getAttribute("aria-checked") === "true")?.textContent;
}

describe("the workspace in the window", () => {
  it("shows the open workspace's name and settings", async () => {
    setUp();
    expect(await screen.findByText("Personal", { selector: "[data-testid=status-workspace] *, [data-testid=status-workspace]" })).toBeDefined();
    await waitFor(() => {
      expect(selectedDay()).toBe("Monday");
    });
    // Fluent isolates the number with invisible marks.
    expect(screen.getByText(/^\u206830\u2069 days$/u)).toBeDefined();
  });

  it("follows a change made elsewhere, through its change event", async () => {
    const { engine, changeElsewhere } = setUp();
    await waitFor(() => {
      expect(selectedDay()).toBe("Monday");
    });
    const reads = engine.calls.filter((call) => call.op === "settings.get").length;
    changeElsewhere("sunday");
    await waitFor(() => {
      expect(selectedDay()).toBe("Sunday");
    });
    expect(engine.calls.filter((call) => call.op === "settings.get").length).toBe(reads + 1);
  });

  it("changes a setting, and offers Undo", async () => {
    const { engine } = setUp();
    await waitFor(() => {
      expect(selectedDay()).toBe("Monday");
    });
    fireEvent.click(screen.getByRole("radio", { name: "Saturday" }));
    const toast = await screen.findByRole("status");
    expect(toast.textContent).toContain("Settings changed");
    expect(engine.calls.at(-1)).toEqual({ op: "settings.update", input: { week_start: "saturday" } });
    fireEvent.click(within(toast).getByRole("button", { name: "Undo" }));
    await waitFor(() => {
      expect(engine.calls.at(-1)?.op).toBe("history.undo");
    });
  });

  it("shows the engine's error and hint in a toast", async () => {
    setUp({ failUpdate: true });
    await waitFor(() => {
      expect(selectedDay()).toBe("Monday");
    });
    fireEvent.click(screen.getByRole("radio", { name: "Sunday" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("Another program is writing.");
    expect(alert.textContent).toContain("Try again in a moment.");
  });
});

describe("the appearance", () => {
  it("changes theme and density at once on <html> and hands them over to be kept", async () => {
    const kept: Appearance[] = [];
    setUp({ onAppearance: (appearance) => kept.push(appearance) });
    const theme = await screen.findByRole("radiogroup", { name: "Theme" }).catch(() => screen.getByRole("group", { name: "Theme" }));
    fireEvent.click(within(theme).getByRole("radio", { name: "Slate" }));
    await waitFor(() => {
      expect(document.documentElement.dataset.theme).toBe("slate");
    });
    const density = screen.getByRole("radiogroup", { name: "Density" });
    fireEvent.click(within(density).getByRole("radio", { name: "Compact" }));
    await waitFor(() => {
      expect(document.documentElement.dataset.density).toBe("compact");
    });
    expect(kept.at(-1)).toMatchObject({ theme: "slate", density: "compact" });
  });
});
