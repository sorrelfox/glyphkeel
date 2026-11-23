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
import { Preferences, type PreferencePage } from "./Preferences";
import { StatusBar } from "./StatusBar";

afterEach(cleanup);

afterEach(() => {
  window.localStorage.clear();
});

function setUp(options: { failUpdate?: boolean; onAppearance?: (appearance: Appearance) => void; page?: PreferencePage } = {}) {
  const onAppearance = options.onAppearance ?? (() => undefined);
  let current: Settings = settings();
  const engine = fakeEngine({
    "workspace.get": () => workspaceInfo({ settings: current }),
    "settings.get": () => current,
    "settings.update": (patch) => {
      if (options.failUpdate) {
        return rejectWith({ code: "busy", message: "Another program is writing.", hint: "Try again in a moment." });
      }
      const changes = Object.fromEntries(Object.entries(patch).filter(([name, value]) => name !== "clear" && value !== null));
      current = { ...current, ...changes };
      return { changed: Object.keys(changes), group: "g-1", settings: current };
    },
    "history.undo": () => ({ group: "g-2", op: "settings.update", target: "g-1" }),
  });
  render(
    <AppearanceProvider onChange={onAppearance}>
      <I18n locale="en-US">
        <ToastProvider>
          <DataProvider client={engine.client}>
            <StatusBar info={null} />
            <Preferences isOpen onOpenChange={() => undefined} page={options.page ?? "time"} />
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
    setUp({ page: "workspace" });
    expect(await screen.findByText("Personal", { selector: "[data-testid=status-workspace] *, [data-testid=status-workspace]" })).toBeDefined();
    const trash = await screen.findByRole("spinbutton", { name: "Keep items in the trash for" });
    expect((trash as HTMLInputElement).value).toBe("30");
    // Outside the desktop app the page can't know the folder.
    expect(screen.getByText("Shown in the desktop app")).toBeDefined();
    // The stand-in data reports the product's own version.
    expect(__VERSION__).toMatch(/^\d+\.\d+\.\d+/u);
    expect(screen.getByText(__VERSION__)).toBeDefined();
    fireEvent.click(screen.getByRole("tab", { name: "Time" }));
    await waitFor(() => {
      expect(selectedDay()).toBe("Monday");
    });
  });

  it("changes the working day and the timer's reminder, and keeps what is out of range", async () => {
    const { engine } = setUp();
    const hours = await screen.findByRole("spinbutton", { name: "Working hours per day" });
    expect((hours as HTMLInputElement).value).toBe("8");
    fireEvent.change(hours, { target: { value: "6" } });
    fireEvent.blur(hours);
    await waitFor(() => {
      expect(engine.calls.at(-1)).toEqual({ op: "settings.update", input: { hours_per_day: 6 } });
    });
    const days = screen.getByRole("spinbutton", { name: "Working days per week" });
    fireEvent.change(days, { target: { value: "9" } });
    fireEvent.keyDown(days, { key: "Enter" });
    expect((days as HTMLInputElement).value).toBe("5");
    const timer = screen.getByRole("spinbutton", { name: "Remind about a running timer after" });
    fireEvent.change(timer, { target: { value: "0" } });
    fireEvent.keyDown(timer, { key: "Enter" });
    await waitFor(() => {
      expect(engine.calls.at(-1)).toEqual({ op: "settings.update", input: { timer_reminder_hours: 0 } });
    });
    expect(engine.calls.filter((call) => call.op === "settings.update")).toHaveLength(2);
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

describe("time zone", () => {
  it("offers System and every zone with its offset, filters as one types, and keeps the choice", async () => {
    setUp({ page: "time" });
    const field = await screen.findByRole("combobox", { name: "Time zone" });
    expect((field as HTMLInputElement).value).toMatch(/^System/u);
    act(() => {
      field.focus();
    });
    fireEvent.change(field, { target: { value: "tokyo" } });
    const option = await screen.findByRole("option", { name: /Asia\/Tokyo/u });
    expect(option.textContent).toContain("+09:00");
    fireEvent.click(option);
    await waitFor(() => {
      expect(JSON.parse(window.localStorage.getItem("glyphkeel.app") ?? "{}")).toMatchObject({ time_zone: "Asia/Tokyo" });
    });
  });
});

describe("reminders and background", () => {
  it("keeps the app's own switches, outside the app in the page's storage", async () => {
    setUp({ page: "reminders" });
    const background = await screen.findByRole("switch", { name: "Keep running in the background" });
    await waitFor(() => {
      expect((background as HTMLInputElement).disabled).toBe(false);
    });
    expect((background as HTMLInputElement).checked).toBe(true);
    fireEvent.click(background);
    await waitFor(() => {
      expect((background as HTMLInputElement).checked).toBe(false);
    });
    const notifications = screen.getByRole("switch", { name: "Show reminders as desktop notifications" });
    fireEvent.click(notifications);
    await waitFor(() => {
      expect(JSON.parse(window.localStorage.getItem("glyphkeel.app") ?? "{}")).toEqual({ background: false, notifications: false, time_zone: "system" });
    });
  });

  it("changes the time of due-date reminders", async () => {
    const { engine } = setUp({ page: "reminders" });
    const time = await screen.findByLabelText("Reminders for due dates at");
    fireEvent.change(time, { target: { value: "08:30" } });
    fireEvent.blur(time);
    await waitFor(() => {
      expect(engine.calls.at(-1)).toEqual({ op: "settings.update", input: { reminder_time: "08:30" } });
    });
  });
});

describe("the appearance", () => {
  it("changes theme and density at once on <html> and hands them over to be kept", async () => {
    const kept: Appearance[] = [];
    setUp({ onAppearance: (appearance) => kept.push(appearance), page: "appearance" });
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

  it("offers the accent colours as swatches, after the desktop's own", async () => {
    const kept: Appearance[] = [];
    setUp({ onAppearance: (appearance) => kept.push(appearance), page: "appearance" });
    const accents = await screen.findByRole("radiogroup", { name: "Accent colour" });
    const names = within(accents)
      .getAllByRole("radio")
      .map((radio) => radio.getAttribute("aria-label") ?? radio.closest("label")?.textContent);
    expect(names).toEqual(["From the desktop", "Blue", "Teal", "Green", "Amber", "Orange", "Red", "Pink", "Purple", "Slate"]);
    // Each swatch is drawn in its own accent.
    expect(within(accents).getByRole("radio", { name: "Teal" }).closest("[data-accent]")?.getAttribute("data-accent")).toBe("teal");
    fireEvent.click(within(accents).getByRole("radio", { name: "Teal" }));
    await waitFor(() => {
      expect(document.documentElement.dataset.accent).toBe("teal");
    });
    expect(kept.at(-1)).toMatchObject({ accent: "teal" });
    expect(document.querySelector("select")).toBeNull();
  });
});
