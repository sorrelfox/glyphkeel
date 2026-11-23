// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { THEMES } from "../appearance/appearance";
import { AppearanceProvider } from "../appearance/AppearanceProvider";
import { I18n } from "../i18n/I18n";
import Gallery from "./Gallery";

afterEach(cleanup);

function renderGallery(onLeave = vi.fn()) {
  render(
    <AppearanceProvider>
      <I18n locale="en-US">
        <Gallery onLeave={onLeave} />
      </I18n>
    </AppearanceProvider>,
  );
  return onLeave;
}

describe("Gallery", () => {
  it("shows every theme side by side, each with its own attributes", () => {
    renderGallery();
    for (const theme of THEMES) {
      const column = screen.getByTestId(`theme-${theme}`);
      expect(column.dataset.theme).toBe(theme);
      expect(column.dataset.density).toBe("regular");
    }
  });

  it("switches the window's theme, density and accent on <html>", () => {
    renderGallery();
    const root = document.documentElement;
    fireEvent.click(within(screen.getByRole("radiogroup", { name: "Theme" })).getByRole("radio", { name: "Slate" }));
    expect(root.dataset.theme).toBe("slate");
    expect(root.dataset.accent).toBe("teal");
    fireEvent.click(within(screen.getByRole("radiogroup", { name: "Density" })).getByRole("radio", { name: "Compact" }));
    expect(root.dataset.density).toBe("compact");
    expect(screen.getByTestId("theme-light").dataset.density).toBe("compact");
    fireEvent.click(within(screen.getByRole("radiogroup", { name: "Accent" })).getByRole("radio", { name: "Pink" }));
    expect(root.dataset.accent).toBe("pink");
    expect(screen.getByTestId("theme-sand").dataset.accent).toBe("pink");
  });

  it("goes back to the app with Escape", () => {
    const onLeave = renderGallery();
    act(() => {
      fireEvent.keyDown(document.body, { key: "Escape" });
    });
    expect(onLeave).toHaveBeenCalledOnce();
  });
});
