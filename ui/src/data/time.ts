// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Time tracking as the screens use it: durations in the workspace's units,
// and the one timer, ticking between the engine's answers.

import { useEffect, useState } from "react";

import { useToasts } from "../components/Toasts";
import { type Timer } from "../generated/ops";
import { useT } from "../i18n/I18n";
import { DEFAULT_UNITS, formatDuration, parseDuration, type DurationUnits } from "./duration";
import { useCommand, useOp } from "./hooks";

/** The workspace's working day and week, which durations are told in. */
export function useDurationUnits(): DurationUnits {
  const settings = useOp("settings.get", {});
  return settings.data ? { hoursPerDay: settings.data.hours_per_day, daysPerWeek: settings.data.days_per_week } : DEFAULT_UNITS;
}

/** Shows and reads durations in the workspace's units: "1d 2h 30m". */
export function useDurations(): { format: (minutes: number) => string; parse: (text: string) => number | null } {
  const units = useDurationUnits();
  return {
    format: (minutes) => formatDuration(minutes, units),
    parse: (text) => parseDuration(text, units),
  };
}

/** The workspace's timer, if one runs or is paused; refreshed by its change events. */
export function useTimer(): { timer: Timer | undefined; fetchedAt: number } {
  const status = useOp("timer.get", {});
  return { timer: status.data?.timer, fetchedAt: status.dataUpdatedAt };
}

/**
 * The seconds a timer has run, now: the engine's count when it answered,
 * plus the time since while it runs. Ticks once a second while running.
 */
export function useElapsedSeconds(timer: Timer | undefined, fetchedAt: number): number {
  const running = timer?.state === "running";
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!running) {
      return;
    }
    const tick = window.setInterval(() => {
      setNow(Date.now());
    }, 1000);
    return () => {
      window.clearInterval(tick);
    };
  }, [running]);
  if (timer === undefined) {
    return 0;
  }
  return timer.elapsed_seconds + (running ? Math.max(0, Math.floor((now - fetchedAt) / 1000)) : 0);
}

/** Whether a timer has run past the hours after which it reminds; 0 hours never. */
export function isLongRunning(seconds: number, reminderHours: number | undefined): boolean {
  return reminderHours !== undefined && reminderHours > 0 && seconds >= reminderHours * 3600;
}

/**
 * Starting and stopping the timer. Stopping logs the time and says how much
 * in a toast. `toggle` is the `T` key: it stops the timer when it times this
 * task, running or paused, and otherwise starts it on this task (the engine
 * stops and logs a timer on another task first).
 */
export function useTimerCommands(): { stopAndLog: () => void; toggle: (key: string) => void; pending: boolean } {
  const t = useT();
  const toasts = useToasts();
  const durations = useDurations();
  const { timer } = useTimer();
  const start = useCommand("timer.start");
  const stop = useCommand("timer.stop");
  const stopAndLog = () => {
    stop.mutate(
      {},
      {
        onSuccess: (change) => {
          toasts.show(
            change.entry
              ? { tone: "info", text: t("timer-logged", { time: durations.format(change.entry.minutes), key: change.entry.task }) }
              : { tone: "info", text: t("timer-nothing-logged") },
          );
        },
      },
    );
  };
  const pending = start.isPending || stop.isPending;
  return {
    stopAndLog,
    toggle: (key) => {
      if (pending) {
        return;
      }
      if (timer?.task.key === key) {
        stopAndLog();
      } else {
        start.mutate({ key });
      }
    },
    pending,
  };
}
