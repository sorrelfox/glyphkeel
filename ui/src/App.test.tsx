// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App, initialRoute } from "./App";

afterEach(cleanup);

describe("App", () => {
  it("shows the shell with the app's header bar and its window buttons", () => {
    render(<App start={{ titleBar: "app", locale: "en-US" }} />);
    expect(screen.getByRole("banner").textContent).toContain("Today");
    for (const name of ["Minimize", "Maximize", "Close"]) {
      expect(screen.getByRole("button", { name })).toBeDefined();
    }
    expect(screen.getByRole("navigation", { name: "Workspace" })).toBeDefined();
    expect(screen.getByRole("contentinfo").textContent).toContain("No tasks");
  });

  it("leaves the window buttons to the system title bar", () => {
    render(<App start={{ titleBar: "system", locale: "en-US" }} />);
    expect(screen.queryByRole("button", { name: "Minimize" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Close" })).toBeNull();
  });

  it("names the window after what it shows", () => {
    render(<App start={{ titleBar: "app", locale: "en-US" }} />);
    expect(document.title.replace(/[⁨⁩]/g, "")).toBe("Glyphkeel: Today");
  });
});

describe("initialRoute", () => {
  it("opens the gallery from its path and the app otherwise", () => {
    expect(initialRoute("/dev/themes")).toBe("gallery");
    expect(initialRoute("/")).toBe("app");
    expect(initialRoute("/index.html")).toBe("app");
  });
});
