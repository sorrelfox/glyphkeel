// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it, vi } from "vitest";

import { DEFAULT_KEYMAP, dispatchKey, keysOf, type Action } from "./actions";
import { parseChord } from "./keymap";

function press(key: string, target: Element, mods: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...mods });
  target.dispatchEvent(event);
  return event;
}

function action(id: string, extra: Partial<Action> = {}): Action {
  return { id, label: id, group: "app", keys: keysOf(id), run: vi.fn(), ...extra };
}

describe("the default keymap", () => {
  it("parses", () => {
    for (const chords of Object.values(DEFAULT_KEYMAP)) {
      for (const chord of chords) {
        expect(() => parseChord(chord)).not.toThrow();
      }
    }
  });
});

describe("dispatchKey", () => {
  it("runs the matching action and prevents the key's default", () => {
    const toggle = action("sidebar.toggle");
    const button = document.body.appendChild(document.createElement("button"));
    const event = new KeyboardEvent("keydown", { key: "F9", cancelable: true });
    button.dispatchEvent(event);
    expect(dispatchKey([toggle], event)).toBe(true);
    expect(toggle.run).toHaveBeenCalledOnce();
    expect(event.defaultPrevented).toBe(true);
  });

  it("keeps single-key shortcuts out of text fields, but not chords", () => {
    const newTask = action("task.new");
    const field = document.body.appendChild(document.createElement("input"));
    const listener = (event: KeyboardEvent) => dispatchKey([newTask], event);
    document.addEventListener("keydown", listener);
    try {
      press("n", field);
      expect(newTask.run).not.toHaveBeenCalled();
      press("n", field, { ctrlKey: true });
      expect(newTask.run).toHaveBeenCalledOnce();
      press("n", document.body);
      expect(newTask.run).toHaveBeenCalledTimes(2);
    } finally {
      document.removeEventListener("keydown", listener);
    }
  });

  it("leaves keys to text fields for actions that say so", () => {
    const undo = action("app.undo", { inText: false });
    const field = document.body.appendChild(document.createElement("textarea"));
    const listener = (event: KeyboardEvent) => dispatchKey([undo], event);
    document.addEventListener("keydown", listener);
    try {
      const event = press("z", field, { ctrlKey: true });
      expect(undo.run).not.toHaveBeenCalled();
      expect(event.defaultPrevented).toBe(false);
    } finally {
      document.removeEventListener("keydown", listener);
    }
  });

  it("takes the keys of a disabled action without running it", () => {
    const palette = action("palette.open", { disabled: true });
    const listener = (event: KeyboardEvent) => dispatchKey([palette], event);
    document.addEventListener("keydown", listener);
    try {
      const event = press("k", document.body, { ctrlKey: true });
      expect(palette.run).not.toHaveBeenCalled();
      expect(event.defaultPrevented).toBe(true);
    } finally {
      document.removeEventListener("keydown", listener);
    }
  });

  it("ignores keys a component already handled", () => {
    const shortcuts = action("help.shortcuts");
    const event = new KeyboardEvent("keydown", { key: "?", cancelable: true });
    event.preventDefault();
    expect(dispatchKey([shortcuts], event)).toBe(false);
    expect(shortcuts.run).not.toHaveBeenCalled();
  });
});
