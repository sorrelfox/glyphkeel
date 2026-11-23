// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { zoneOffsetSeconds } from "../ipc/appSettings";
import { PROJECT_ICONS, suggestHue } from "./catalog";
import { dayIn, momentFrom, momentIn } from "./zone";

describe("times in the person's zone", () => {
  const at = Date.UTC(2026, 10, 23, 23, 30);

  it("reads a moment's day and time in the zone", () => {
    expect(dayIn(at, "UTC")).toBe("2026-11-23");
    expect(dayIn(at, "Asia/Tokyo")).toBe("2026-11-24");
    expect(momentIn(at, "America/New_York")).toBe("2026-11-23T18:30");
    expect(zoneOffsetSeconds("Asia/Tokyo", at)).toBe(9 * 3600);
    expect(zoneOffsetSeconds("America/St_Johns", at)).toBe(-(3 * 3600 + 30 * 60));
  });

  it("turns a local moment back into the same instant, across a clock change", () => {
    expect(momentFrom("2026-11-24T08:30", "Asia/Tokyo")).toBe(at);
    // Berlin in summer and in winter.
    expect(momentFrom("2026-07-01T12:00", "Europe/Berlin")).toBe(Date.UTC(2026, 6, 1, 10));
    expect(momentFrom("2026-12-01T12:00", "Europe/Berlin")).toBe(Date.UTC(2026, 11, 1, 11));
    // 02:30 doesn't exist on the morning clocks go forward; it lands after the gap.
    expect(momentIn(momentFrom("2026-03-29T02:30", "Europe/Berlin"), "Europe/Berlin")).toBe("2026-03-29T03:30");
    expect(momentFrom("not a time", "UTC")).toBeNaN();
    for (const zone of ["UTC", "Asia/Kathmandu", "Pacific/Kiritimati", "America/Los_Angeles"]) {
      expect(momentFrom(momentIn(at, zone), zone)).toBe(at);
    }
  });
});

describe("suggested colours", () => {
  it("takes the first unused hue in palette order, else the least-used one", () => {
    expect(suggestHue([])).toBe("red");
    expect(suggestHue(["red", "orange"])).toBe("yellow");
    expect(suggestHue(["orange", "red"])).toBe("yellow");
    const all = ["red", "orange", "yellow", "green", "teal", "blue", "purple", "pink", "grey"] as const;
    expect(suggestHue([...all, "red"])).toBe("orange");
    expect(suggestHue([...all, ...all, "red", "orange"])).toBe("yellow");
  });

  it("offers forty project icons, each once", () => {
    expect(PROJECT_ICONS).toHaveLength(40);
    expect(new Set(PROJECT_ICONS).size).toBe(40);
  });
});
