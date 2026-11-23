// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { afterEach, describe, expect, it } from "vitest";

import { isBrowserKey, suppressBrowserBehaviour } from "./native";

const key = (k: string, mods: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {}) => ({
  key: k,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe("isBrowserKey", () => {
  it("catches reload, print, caret browsing, history and zoom", () => {
    for (const event of [
      key("F5"),
      key("r", { ctrlKey: true }),
      key("R", { ctrlKey: true, shiftKey: true }),
      key("p", { ctrlKey: true }),
      key("F7"),
      key("ArrowLeft", { altKey: true }),
      key("ArrowRight", { altKey: true }),
      key("+", { ctrlKey: true }),
      key("-", { ctrlKey: true }),
      key("0", { ctrlKey: true }),
    ]) {
      expect(isBrowserKey(event), JSON.stringify(event)).toBe(true);
    }
  });

  it("leaves the app's own keys alone", () => {
    for (const event of [key("k", { ctrlKey: true }), key("F9"), key("n"), key("ArrowLeft"), key("z", { ctrlKey: true })]) {
      expect(isBrowserKey(event), JSON.stringify(event)).toBe(false);
    }
  });
});

describe("suppressBrowserBehaviour", () => {
  let undo: () => void = () => undefined;
  afterEach(() => {
    undo();
    document.body.replaceChildren();
  });

  it("blocks the browser's context menu, except in text", () => {
    undo = suppressBrowserBehaviour(window);
    const button = document.body.appendChild(document.createElement("button"));
    const field = document.body.appendChild(document.createElement("input"));
    const onButton = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    const onField = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    button.dispatchEvent(onButton);
    field.dispatchEvent(onField);
    expect(onButton.defaultPrevented).toBe(true);
    expect(onField.defaultPrevented).toBe(false);
  });

  it("blocks reload keys and zooming with the wheel", () => {
    undo = suppressBrowserBehaviour(window);
    const reload = new KeyboardEvent("keydown", { key: "F5", bubbles: true, cancelable: true });
    document.body.dispatchEvent(reload);
    expect(reload.defaultPrevented).toBe(true);
    const zoom = new WheelEvent("wheel", { ctrlKey: true, bubbles: true, cancelable: true });
    document.body.dispatchEvent(zoom);
    expect(zoom.defaultPrevented).toBe(true);
    const scroll = new WheelEvent("wheel", { bubbles: true, cancelable: true });
    document.body.dispatchEvent(scroll);
    expect(scroll.defaultPrevented).toBe(false);
  });
});
