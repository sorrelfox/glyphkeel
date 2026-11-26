// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { DEFAULT_KEYMAP } from "../actions/actions";
import { parseChord } from "../actions/keymap";
import { createBundle } from "../i18n/i18n";
import { SCREEN_KEYS, screenOf } from "./screenKeys";

describe("the keys screens handle themselves", () => {
  it("are found for the wiki, the Gantt chart and the calendar", () => {
    expect(screenOf("/projects/TM/wiki")).toBe("wiki");
    expect(screenOf("/projects/TM/gantt")).toBe("gantt");
    expect(screenOf("/projects/TM/calendar")).toBe("calendar");
    expect(screenOf("/projects/TM")).toBeNull();
    expect(screenOf("/projects/TM/list")).toBeNull();
    expect(screenOf("/")).toBeNull();
  });

  it("have messages and keys that parse", () => {
    const bundle = createBundle("en");
    for (const entries of Object.values(SCREEN_KEYS)) {
      expect(entries.length).toBeGreaterThan(0);
      for (const entry of entries) {
        expect(bundle.hasMessage(entry.label), entry.label).toBe(true);
        for (const keys of entry.keys) {
          expect(() => parseChord(keys)).not.toThrow();
        }
      }
    }
    expect(bundle.hasMessage("shortcut-group-screen")).toBe(true);
  });

  it("list the wiki's E, the tree's Alt and arrow keys and the period keys apart from the window's", () => {
    const wiki = SCREEN_KEYS.wiki.flatMap((entry) => entry.keys);
    expect(wiki).toEqual(expect.arrayContaining(["E", "Alt+ArrowUp", "Alt+ArrowDown", "Alt+ArrowLeft", "Alt+ArrowRight"]));
    const gantt = SCREEN_KEYS.gantt.flatMap((entry) => entry.keys);
    expect(gantt).toEqual(expect.arrayContaining(["Alt+ArrowRight", "Alt+Shift+ArrowRight", "F2", "L", "+", "-"]));
    // The calendar's periods are window actions, in the keymap already.
    expect(DEFAULT_KEYMAP["view.previousPeriod"]).toEqual(["Alt+PageUp"]);
  });
});
