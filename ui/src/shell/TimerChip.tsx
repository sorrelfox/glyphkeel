// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate } from "@tanstack/react-router";
import { Clock, Pause, Play, Square } from "lucide-react";
import { Button } from "react-aria-components";

import { classes } from "../classes";
import { useToasts } from "../components/Toasts";
import { formatClock } from "../data/duration";
import { useCommand, useOp } from "../data/hooks";
import { isLongRunning, useDurations, useElapsedSeconds, useTimer } from "../data/time";
import { useT } from "../i18n/I18n";
import styles from "./Shell.module.css";

/**
 * The timer chip in the sidebar, only while a timer runs or is paused: the
 * running time and the task's key, which opens the task; pause or go on;
 * stop, which logs the time. It follows the timer wherever it was started,
 * in this window, another one, the command line or an agent. Past the
 * workspace's reminder hours the time turns to the warning colour.
 */
export function TimerChip() {
  const t = useT();
  const navigate = useNavigate();
  const toasts = useToasts();
  const durations = useDurations();
  const settings = useOp("settings.get", {});
  const { timer, fetchedAt } = useTimer();
  const seconds = useElapsedSeconds(timer, fetchedAt);
  const stop = useCommand("timer.stop");
  const pause = useCommand("timer.pause");
  const start = useCommand("timer.start");
  if (timer === undefined) {
    return null;
  }
  const running = timer.state === "running";
  const long = isLongRunning(seconds, settings.data?.timer_reminder_hours);
  const clock = formatClock(seconds);
  return (
    <div className={classes(styles.timerChip, long && styles.timerLong)} role="group" aria-label={t("timer-chip-label")} data-timer-chip={timer.task.key}>
      <Button
        className={classes(styles.timerOpen)}
        aria-label={t(long ? "timer-chip-open-long" : running ? "timer-chip-open" : "timer-chip-open-paused", { time: clock, key: timer.task.key, title: timer.task.title, hours: settings.data?.timer_reminder_hours ?? 8 })}
        onPress={() => void navigate({ to: "/open/$key", params: { key: timer.task.key } })}
      >
        <Clock className={classes("ic", running ? "accent" : "muted")} />
        <span className={styles.timerText}>
          <span className={classes("num", styles.timerClock)} data-timer-clock>
            {clock}
          </span>
          <span className="key truncate">{timer.task.key}</span>
        </span>
      </Button>
      <Button
        className="btn flat icon sm"
        aria-label={t(running ? "timer-pause" : "timer-resume")}
        onPress={() => {
          if (running) {
            pause.mutate({});
          } else {
            start.mutate({});
          }
        }}
      >
        {running ? <Pause className="ic s" /> : <Play className="ic s" />}
      </Button>
      <Button
        className="btn flat icon sm"
        aria-label={t("timer-stop")}
        isDisabled={stop.isPending}
        onPress={() => {
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
        }}
      >
        <Square className="ic s" />
      </Button>
    </div>
  );
}
