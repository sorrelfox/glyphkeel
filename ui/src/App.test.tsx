// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "./App";
import { initialPath } from "./router";

afterEach(cleanup);

describe("App", () => {
  it("shows the shell with the app's header bar and its window buttons", async () => {
    render(<App start={{ titleBar: "app", locale: "en-US" }} />);
    expect((await screen.findByRole("banner")).textContent).toContain("Today");
    for (const name of ["Minimize", "Maximize", "Close"]) {
      expect(screen.getByRole("button", { name })).toBeDefined();
    }
    expect(screen.getByRole("navigation", { name: "Workspace" })).toBeDefined();
    expect(screen.getByRole("contentinfo").textContent).toContain("No tasks");
  });

  it("leaves the window buttons to the system title bar", async () => {
    render(<App start={{ titleBar: "system", locale: "en-US" }} />);
    await screen.findByRole("banner");
    expect(screen.queryByRole("button", { name: "Minimize" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Close" })).toBeNull();
  });

  it("names the window after what it shows", async () => {
    render(<App start={{ titleBar: "app", locale: "en-US" }} />);
    await screen.findByRole("banner");
    expect(document.title.replace(/[⁨⁩]/g, "")).toBe("Glyphkeel: Today");
  });
});

describe("initialPath", () => {
  it("opens the gallery from its path and Today otherwise", () => {
    expect(initialPath("/dev/themes")).toBe("/dev/themes");
    expect(initialPath("/")).toBe("/");
    expect(initialPath("/index.html")).toBe("/");
  });

  it("opens a task window's task, or the task of the link the app was started with", () => {
    expect(initialPath("/index.html", { task: "TM-41" })).toBe("/task/TM-41");
    expect(initialPath("/index.html", { open: "TM-41" })).toBe("/open/TM-41");
  });
});
