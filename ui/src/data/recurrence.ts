// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useMutation } from "@tanstack/react-query";
import { useLocale } from "react-aria-components";

import { useToasts } from "../components/Toasts";
import { type RecurrenceRule, type TemplateApplyInput, type TemplateKind, type Weekday } from "../generated/ops";
import { useT, type Translate } from "../i18n/I18n";
import { errorToast, useCall, useOp } from "./hooks";

/** The weekdays, Monday first, as rules name them. */
export const WEEKDAYS: readonly Weekday[] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

// 2024-01-01 was a Monday.
const weekdayDate = (day: Weekday) => new Date(Date.UTC(2024, 0, 1 + WEEKDAYS.indexOf(day)));

/** A weekday's name in the locale. */
export function weekdayName(locale: string, day: Weekday, width: "long" | "short" = "long"): string {
  return new Intl.DateTimeFormat(locale, { weekday: width, timeZone: "UTC" }).format(weekdayDate(day));
}

/** A month's name (1 to 12) in the locale. */
export function monthName(locale: string, month: number): string {
  return new Intl.DateTimeFormat(locale, { month: "long", timeZone: "UTC" }).format(new Date(Date.UTC(2024, month - 1, 1)));
}

/** A day `YYYY-MM-DD` as the locale writes a full date. */
export function longDay(locale: string, day: string): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeZone: "UTC" }).format(new Date(`${day}T00:00:00Z`));
}

const AFTER_DONE: Record<RecurrenceRule["every"], string> = { day: "repeat-after-done-days", week: "repeat-after-done-weeks", month: "repeat-after-done-months", year: "repeat-after-done-years" };

/** A rule in words, in the person's language: "Every 2 weeks on Monday and Thursday". */
export function ruleSummary(t: Translate, locale: string, rule: RecurrenceRule): string {
  const interval = rule.interval;
  const [, startMonth = 1, startDay = 1] = rule.starts.split("-").map(Number);
  const nth = (n: number, day: Weekday) => t("repeat-nth-weekday", { nth: n, weekday: weekdayName(locale, day) });
  let text: string;
  if (rule.from === "completion") {
    text = t(AFTER_DONE[rule.every], { interval });
  } else if (rule.every === "day") {
    text = t("repeat-daily", { interval });
  } else if (rule.every === "week") {
    const days = new Intl.ListFormat(locale, { style: "long", type: "conjunction" }).format((rule.weekdays ?? []).map((day) => weekdayName(locale, day)));
    text = t("repeat-weekly", { interval, days });
  } else if (rule.every === "month") {
    if (rule.week_of_month !== undefined && rule.weekday !== undefined) {
      text = t("repeat-monthly-weekday", { interval, weekday: nth(rule.week_of_month, rule.weekday) });
    } else if (rule.month_day === -1) {
      text = t("repeat-monthly-last-day", { interval });
    } else {
      text = t("repeat-monthly-day", { interval, day: rule.month_day ?? startDay });
    }
  } else {
    const month = rule.month ?? startMonth;
    if (rule.week_of_month !== undefined && rule.weekday !== undefined) {
      text = t("repeat-yearly-weekday", { interval, weekday: nth(rule.week_of_month, rule.weekday), month: monthName(locale, month) });
    } else if (rule.month_day === -1) {
      text = t("repeat-yearly-last-day", { interval, month: monthName(locale, month) });
    } else {
      const date = new Intl.DateTimeFormat(locale, { month: "long", day: "numeric", timeZone: "UTC" }).format(new Date(Date.UTC(2024, month - 1, rule.month_day ?? startDay)));
      text = t("repeat-yearly-day", { interval, date });
    }
  }
  if (rule.until !== undefined) {
    text = t("repeat-with-until", { rule: text, until: longDay(locale, rule.until) });
  }
  if (rule.count !== undefined) {
    text = t("repeat-with-count", { rule: text, count: rule.count });
  }
  return text;
}

/** `ruleSummary` in the page's language. */
export function useRuleSummary(): (rule: RecurrenceRule) => string {
  const t = useT();
  const { locale } = useLocale();
  return (rule) => ruleSummary(t, locale, rule);
}

/** The templates of one kind: a project's task templates, or the workspace's project templates. */
export function useTemplates(kind: TemplateKind, project?: string, enabled = true) {
  return useOp("template.list", { kind, ...(project === undefined ? {} : { project }) }, { enabled });
}

/**
 * Uses a template. Errors become a toast; so do the names of statuses, types
 * and labels the project lacks, which were left out.
 */
export function useApplyTemplate() {
  const call = useCall();
  const toasts = useToasts();
  const t = useT();
  const { locale } = useLocale();
  return useMutation({
    mutationFn: (input: TemplateApplyInput) => call("template.apply", input),
    onError: (error) => {
      toasts.show(errorToast(t, error));
    },
    onSuccess: (applied) => {
      const leftOut = applied.left_out ?? [];
      if (leftOut.length > 0) {
        toasts.show({ tone: "info", text: t("template-left-out", { count: leftOut.length, names: new Intl.ListFormat(locale, { style: "long", type: "conjunction" }).format(leftOut) }) });
      }
    },
  });
}
