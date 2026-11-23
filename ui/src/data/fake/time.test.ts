// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type DueReminders, type ReminderChange, type TaskChange, type TaskList, type TimeEntryChange, type TimerChange, type TimerStatus, type TimeReport } from "../../generated/ops";
import { FakeError, FakeWorkspace } from "./engine";
import { seed } from "./seed";

const START = new Date(2026, 10, 23, 9, 0);

function workspace() {
  let now = START.getTime();
  const ws = new FakeWorkspace(seed(START), () => new Date(now));
  // Answers come back untyped; the tests read them as the generated client types them.
  // eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
  const call = <T,>(op: string, input: unknown = {}): T => ws.call(op, input).output as T;
  const advance = (minutes: number) => {
    now += minutes * 60_000;
  };
  const key = call<TaskChange>("task.create", { project: "PER", title: "Write the report" }).task.key;
  return { ws, call, advance, key };
}

describe("the stand-in's time tracking", () => {
  it("logs, edits and deletes entries, keeping the task's spent time", () => {
    const { ws, call, key } = workspace();
    const logged = call<TimeEntryChange>("time.log", { key, duration: "+30m" });
    expect(logged.entry).toMatchObject({ minutes: 30, duration: "30m", date: "2026-11-23", task: key });
    call<TimeEntryChange>("time.log", { key, duration: "1,5h", date: "yesterday", note: "Drafting" });
    const task = () => call<TaskList>("task.get", { keys: [key], include: ["time"] }).items[0];
    expect(task()?.spent_minutes).toBe(120);
    expect(task()?.time_entries?.map((entry) => entry.date)).toEqual(["2026-11-23", "2026-11-22"]);

    const edited = call<TimeEntryChange>("time.update", { entry: logged.entry.id, duration: "1d", expected_version: 1 });
    expect(edited.entry).toMatchObject({ minutes: 480, duration: "1d", version: 2 });
    expect(task()?.spent_minutes).toBe(570);
    expect(() => call("time.update", { entry: logged.entry.id, note: "x", expected_version: 1 })).toThrow(FakeError);

    const answer = ws.call("time.delete", { entry: logged.entry.id });
    expect(answer.changes?.changes).toEqual([{ entity: "time_entry", id: logged.entry.id, fields: ["trashed"], project: "PER" }]);
    expect(task()?.spent_minutes).toBe(90);
    expect(() => call("time.log", { key, duration: "25h" })).toThrow(FakeError);
    expect(() => call("time.log", { key, duration: "soon" })).toThrow(FakeError);
  });

  it("runs one timer: pausing keeps its time, starting another stops and logs the first", () => {
    const { call, advance, key } = workspace();
    const other = call<TaskChange>("task.create", { project: "PER", title: "Call the bank" }).task.key;
    call<TimerChange>("timer.start", { key });
    advance(20);
    call("timer.pause");
    advance(60);
    expect(call<TimerStatus>("timer.get").timer).toMatchObject({ state: "paused", elapsed_seconds: 1200 });
    call("timer.start");
    advance(10);
    const switched = call<TimerChange>("timer.start", { key: other });
    expect(switched.timer?.task.key).toBe(other);
    const first = call<TaskList>("task.get", { keys: [key], include: ["time"] }).items[0];
    expect(first?.time_entries?.map((entry) => entry.minutes)).toEqual([30]);
    advance(1);
    const stopped = call<TimerChange>("timer.stop", { note: "Short call" });
    expect(stopped.entry).toMatchObject({ minutes: 1, note: "Short call", task: other });
    expect(call<TimerStatus>("timer.get").timer).toBeUndefined();
  });

  it("reports logged time by task, day and project over a range", () => {
    const { call, key } = workspace();
    call("time.log", { key, duration: "2h", date: "2026-11-23" });
    call("time.log", { key, duration: "1h", date: "2026-11-24" });
    const report = call<TimeReport>("time.report", { from: "2026-11-23", to: "2026-11-24", group_by: "day", task: key });
    expect(report.items.map((row) => [row.key, row.minutes])).toEqual([["2026-11-23", 120], ["2026-11-24", 60]]);
    expect(report.total_duration).toBe("3h");
    const byTask = call<TimeReport>("time.report", { from: "2026-11-23", to: "2026-11-23", task: key });
    expect(byTask.items).toEqual([{ key, title: "Write the report", minutes: 120, duration: "2h", entry_count: 1 }]);
  });

  it("sets a reminder, lists it when due, and forgets it once acknowledged", () => {
    const { ws, call, advance, key } = workspace();
    call("task.update", { key, due_date: "2026-11-25" });
    const set = call<ReminderChange>("reminder.set", { key, days_before_due: 1, time: "10:00" });
    expect(set.reminder?.at).toBe(new Date(2026, 10, 24, 10, 0).toISOString());
    expect(call<DueReminders>("reminder.due").items).toEqual([]);
    expect(call<DueReminders>("reminder.due").next_at).toBe(set.reminder?.at);
    advance(25 * 60 + 30);
    const due = call<DueReminders>("reminder.due", { since: START.toISOString() });
    expect(due.items.map((item) => [item.kind, item.task.key, item.missed])).toEqual([["reminder", key, false]]);
    call("reminder.acknowledge", { ids: due.items.map((item) => item.id) });
    expect(call<DueReminders>("reminder.due").items).toEqual([]);
    const cleared = ws.call("reminder.clear", { key });
    expect(cleared.changes?.changes[0]).toMatchObject({ entity: "task", fields: ["reminder"] });
    expect(call<TaskList>("task.get", { keys: [key], include: ["reminder"] }).items[0]?.reminder).toBeUndefined();
  });

  it("reminds once of a timer that ran for the workspace's hours", () => {
    const { call, advance, key } = workspace();
    call("timer.start", { key });
    advance(8 * 60 + 1);
    const due = call<DueReminders>("reminder.due");
    expect(due.items.map((item) => item.kind)).toEqual(["timer"]);
    call("reminder.acknowledge", { ids: [due.items[0]?.id] });
    expect(call<DueReminders>("reminder.due").items).toEqual([]);
  });
});
