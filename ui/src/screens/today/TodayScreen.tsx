// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { getRouteApi, useNavigate } from "@tanstack/react-router";
import { type ReactNode } from "react";

import { useAction } from "../../actions/ActionsProvider";
import { classes } from "../../classes";
import { RecurringMark } from "../../components/Glyphs";
import { ProjectMark } from "../../components/ProjectMark";
import { useProjects } from "../../data/board";
import { useTodayOverview } from "../../data/today";
import { useToday as useDay } from "../../data/zone";
import { type ListedTask, type ProjectSummary, type Today, type TodayCounts } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { EmptyState } from "../../shell/EmptyState";
import { useDayFormat } from "../board/CardView";
import { dueState } from "../board/model";
import { useMomentFormat } from "../task/Activity";
import { TaskPanel } from "../task/TaskPanel";
import { DigestPanel } from "./DigestPanel";
import styles from "./Today.module.css";

const route = getRouteApi("/shell/");

/** One row: a task with what the section says about it. */
interface Row {
  key: string;
  title: string;
  due?: string | undefined;
  closed: boolean;
  /** Whether it is the current occurrence of a recurring task. */
  recurring?: boolean;
  /** The section's own detail: a status, a reminder's time, who changed it. */
  detail?: ReactNode;
}

type SectionId = "overdue" | "due_today" | "in_progress" | "reminders" | "claimed" | "recent" | "timer";

interface Section {
  id: SectionId;
  rows: Row[];
  /** How many there are, also beyond the rows read. */
  count: number;
}

const taskRow = (task: ListedTask, detail?: ReactNode): Row => ({ key: task.key, title: task.title, due: task.due_date, closed: task.closed_at !== undefined, recurring: task.recurring === true, detail });

/** The project key of a task key. */
const projectOf = (key: string) => key.slice(0, key.lastIndexOf("-"));

/**
 * Today: the digest of what changed while the person was away (when it
 * holds anything), then overdue tasks, tasks due today and tasks in
 * progress across the active projects, today's reminders, agents' claims,
 * the last day's changes and the timer. A row opens its task in the panel
 * on the right.
 */
export function TodayScreen() {
  const { task } = route.useSearch();
  const navigate = useNavigate();
  const today = useTodayOverview();
  const open = (key: string | undefined) => {
    void navigate({ to: "/", search: key === undefined ? {} : { task: key } });
  };
  return (
    <div className={styles.layout}>
      <div className={styles.scroll}>
        <DigestPanel onOpen={open} />
        {today.data ? <TodayLists today={today.data} openTask={task} onOpen={open} /> : <div aria-busy="true" />}
      </div>
      {task !== undefined && (
        <TaskPanel
          key={task}
          taskKey={task}
          onClose={() => {
            open(undefined);
            requestAnimationFrame(() => {
              document.querySelector<HTMLElement>(`[data-today-row="${CSS.escape(task)}"]`)?.focus();
            });
          }}
          onOpenTask={open}
          onOpenWindow={(other) => void navigate({ to: "/task/$key", params: { key: other } })}
        />
      )}
    </div>
  );
}

function TodayLists({ today, openTask, onOpen }: { today: Today; openTask: string | undefined; onOpen: (key: string) => void }) {
  const t = useT();
  const moment = useMomentFormat();
  const newTask = useAction("task.new");
  const status = (task: ListedTask) => <span className="faint">{task.status.name}</span>;
  const counts: TodayCounts = today.counts;
  const sections: Section[] = [
    { id: "overdue", rows: today.overdue.map((task) => taskRow(task, status(task))), count: counts.overdue },
    { id: "due_today", rows: today.due_today.map((task) => taskRow(task, status(task))), count: counts.due_today },
    { id: "in_progress", rows: today.in_progress.map((task) => taskRow(task, status(task))), count: counts.in_progress },
    {
      id: "reminders",
      rows: today.reminders.map((reminder) => ({
        key: reminder.task.key,
        title: reminder.task.title,
        due: reminder.due_date,
        closed: reminder.task.closed,
        detail: (
          <span className={classes("faint", reminder.missed && "overdue")}>
            {reminder.note === undefined ? moment(reminder.at) : t("today-reminder-note", { time: moment(reminder.at), note: reminder.note })}
          </span>
        ),
      })),
      count: counts.reminders,
    },
    {
      id: "claimed",
      rows: today.claimed.map((task) => taskRow(task, task.claim && <span className="faint">{t("today-claimed-by", { agent: task.claim.agent, time: moment(task.claim.until) })}</span>)),
      count: counts.claimed,
    },
    {
      id: "recent",
      rows: today.recent.map((recent) =>
        taskRow(recent.task, <span className="faint">{t("today-changed-by", { who: recent.changed_by.kind === "user" ? t("today-me") : recent.changed_by.name, time: moment(recent.changed_at) })}</span>),
      ),
      count: counts.recent,
    },
    {
      id: "timer",
      rows: today.timer
        ? [
            {
              key: today.timer.task.key,
              title: today.timer.task.title,
              closed: today.timer.task.closed,
              detail: <span className="faint">{t(today.timer.state === "running" ? "today-timer-running" : "today-timer-paused", { elapsed: today.timer.elapsed })}</span>,
            },
          ]
        : [],
      count: today.timer ? 1 : 0,
    },
  ];
  const shown = sections.filter((section) => section.rows.length > 0);
  if (shown.length === 0) {
    return <EmptyState text={t("empty-today")} action={newTask} />;
  }
  const nothingDue = counts.today === 0;
  return (
    <div className={styles.column}>
      {nothingDue && <p className={classes("muted", styles.nothing)}>{t("empty-today")}</p>}
      {shown.map((section) => (
        <TodaySection key={section.id} section={section} openTask={openTask} onOpen={onOpen} />
      ))}
    </div>
  );
}

function TodaySection({ section, openTask, onOpen }: { section: Section; openTask: string | undefined; onOpen: (key: string) => void }) {
  const t = useT();
  const projects = useProjects();
  const day = useDayFormat();
  const today = useDay();
  const byKey = new Map<string, ProjectSummary>((projects.data?.items ?? []).map((project) => [project.key, project]));
  const name = t(`today-section-${section.id.replace("_", "-")}`);
  const headingId = `today-${section.id}`;
  const more = section.count - section.rows.length;
  return (
    <section className={styles.section} aria-labelledby={headingId} data-today-section={section.id}>
      <h2 id={headingId} className={classes(styles.sectionHead, section.id === "overdue" && styles.danger)}>
        <span>{name}</span>
        <span className="count">{section.count}</span>
      </h2>
      <ul className={styles.boxed} aria-label={name}>
        {section.rows.map((row) => {
          const project = byKey.get(projectOf(row.key));
          const state = dueState(row.due, row.closed, today);
          return (
            <li key={row.key}>
              <button
                type="button"
                className={styles.row}
                data-today-row={row.key}
                data-open={row.key === openTask ? "" : undefined}
                onClick={() => {
                  onOpen(row.key);
                }}
              >
                {/* Named by its text, all of it, so that the name holds what is on
                    screen; the spaces keep the parts apart in the name. */}
                <span className="key">{row.key}</span>{" "}
                <span className={styles.title}>{row.title}</span>{" "}
                {row.recurring === true && <RecurringMark />}{" "}
                {row.detail}{" "}
                {project && (
                  <span className={classes("meta", styles.project)} title={project.path}>
                    <ProjectMark hue={project.hue} icon={project.icon} />
                    {project.path}
                  </span>
                )}{" "}
                <span className={classes("meta", styles.due, state === "overdue" && "overdue", (state === "today" || state === "soon") && "soon")}>
                  {row.due === undefined ? "" : state === "today" ? t("today-due-today") : day(row.due)}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
      {more > 0 && <p className={classes("faint", styles.more)}>{t("today-more", { count: more })}</p>}
    </section>
  );
}
