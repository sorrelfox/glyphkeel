// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Generated from the engine's friendly-date examples by `cargo xtask gen`.
// Do not edit: change crates/gk-query/src/examples.rs and run it again.

import { type WeekStart } from "./ops";

/** On `today`, with weeks from `week_start`, `text` means `day` (null: no date). */
export interface NaturalDateCase {
  today: string;
  week_start: WeekStart;
  text: string;
  day: string | null;
}

export const NATURAL_DATES: readonly NaturalDateCase[] = [
  {
    "day": "2026-11-23",
    "text": "today",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-24",
    "text": "Tomorrow",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-22",
    "text": "yesterday",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-23",
    "text": "  TODAY  ",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-26",
    "text": "+3d",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-16",
    "text": "-7d",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-07",
    "text": "+2w",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-10-23",
    "text": "-1m",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2027-11-23",
    "text": "+1y",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-24",
    "text": "+1D",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "+",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "+7x",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "7d",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "+1234567d",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-23",
    "text": "this-week",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-16",
    "text": "last-week",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-01",
    "text": "next-month",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2025-01-01",
    "text": "last-year",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "next-decade",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2027-02-28",
    "text": "+2m",
    "today": "2026-12-31",
    "week_start": "monday"
  },
  {
    "day": "2027-01-01",
    "text": "tomorrow",
    "today": "2026-12-31",
    "week_start": "monday"
  },
  {
    "day": "2029-02-28",
    "text": "+1y",
    "today": "2028-02-29",
    "week_start": "monday"
  },
  {
    "day": "2028-02-01",
    "text": "this-month",
    "today": "2028-02-29",
    "week_start": "monday"
  },
  {
    "day": "2026-12-01",
    "text": "2026-12-01",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-02-28",
    "text": "2026-02-28",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "2026-2-30",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "2026-02-30",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "31-12-2026",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "11/27/2026",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-27",
    "text": "fri",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-27",
    "text": "Friday",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-27",
    "text": "frid",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-30",
    "text": "mon",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-29",
    "text": "sun",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-06",
    "text": "sun",
    "today": "2026-11-29",
    "week_start": "monday"
  },
  {
    "day": "2026-11-30",
    "text": "mon",
    "today": "2026-11-29",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "fr",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "fridays",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-27",
    "text": "this fri",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-29",
    "text": "this sun",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-23",
    "text": "this mon",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-04",
    "text": "next fri",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-30",
    "text": "next mon",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-30",
    "text": "next week",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-01",
    "text": "next month",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2027-01-01",
    "text": "next year",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-23",
    "text": "this week",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "next lunch",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "last fri",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-04",
    "text": "next fri",
    "today": "2026-11-23",
    "week_start": "sunday"
  },
  {
    "day": "2026-11-29",
    "text": "next week",
    "today": "2026-11-23",
    "week_start": "sunday"
  },
  {
    "day": "2026-11-22",
    "text": "this sun",
    "today": "2026-11-23",
    "week_start": "sunday"
  },
  {
    "day": "2026-11-22",
    "text": "this-week",
    "today": "2026-11-23",
    "week_start": "sunday"
  },
  {
    "day": "2026-11-21",
    "text": "this-week",
    "today": "2026-11-23",
    "week_start": "saturday"
  },
  {
    "day": "2026-11-21",
    "text": "this sat",
    "today": "2026-11-23",
    "week_start": "saturday"
  },
  {
    "day": "2026-11-28",
    "text": "next sat",
    "today": "2026-11-23",
    "week_start": "saturday"
  },
  {
    "day": "2026-11-27",
    "text": "end of week",
    "today": "2026-11-23",
    "week_start": "saturday"
  },
  {
    "day": "2026-12-06",
    "text": "next sun",
    "today": "2026-11-29",
    "week_start": "sunday"
  },
  {
    "day": "2026-11-26",
    "text": "in 3 days",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-24",
    "text": "in 1 day",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-07",
    "text": "in 2 weeks",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-30",
    "text": "in a week",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "in an hour",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2027-11-23",
    "text": "in one year",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-23",
    "text": "in a month",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-26",
    "text": "in 3 d",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-26",
    "text": "in 3d",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-07",
    "text": "in 2w",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-23",
    "text": "in 1m",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2027-11-23",
    "text": "in 1y",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-03",
    "text": "in 10days",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "in d",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "in 3x",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "in 10001 days",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "in",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2029-02-28",
    "text": "in 1 year",
    "today": "2028-02-29",
    "week_start": "monday"
  },
  {
    "day": "2026-12-01",
    "text": "dec 1",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-01",
    "text": "1 Dec",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-01",
    "text": "Dec. 1st",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-24",
    "text": "december 24th",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-23",
    "text": "nov 23",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2027-11-01",
    "text": "nov 1",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2028-12-01",
    "text": "december 1 2028",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2027-12-01",
    "text": "december 1, 2027",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2027-12-01",
    "text": "1 dec 2027",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2028-02-29",
    "text": "feb 29",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "feb 29 2027",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "feb 30",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "de 1",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "dec dec",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "dec 1 0",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "dec 1 year",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2027-01-01",
    "text": "jan 1",
    "today": "2026-12-31",
    "week_start": "monday"
  },
  {
    "day": "2026-11-30",
    "text": "end of month",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-30",
    "text": "end of the month",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-11-29",
    "text": "end of week",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-31",
    "text": "end of year",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2026-12-31",
    "text": "end of the year",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "end of day",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": "2028-02-29",
    "text": "end of month",
    "today": "2028-02-29",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "   ",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "someday",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "soon",
    "today": "2026-11-23",
    "week_start": "monday"
  },
  {
    "day": null,
    "text": "the day after tomorrow",
    "today": "2026-11-23",
    "week_start": "monday"
  }
];
