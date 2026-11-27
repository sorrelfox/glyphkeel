// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { getRouteApi, useNavigate } from "@tanstack/react-router";
import { ChevronDown, ChevronRight, IterationCw, MoreHorizontal, Play, Plus, Square } from "lucide-react";
import { useState } from "react";
import { Button, CheckboxButton, CheckboxField, GridList, GridListItem, Menu, MenuTrigger, Popover, useDragAndDrop, useLocale, type DropItem, type Selection } from "react-aria-components";

import { classes } from "../../classes";
import { MenuEntries, type MenuEntry } from "../../components/ContextMenu";
import { Switch } from "../../components/Switch";
import { formatDuration } from "../../data/duration";
import { failureText } from "../../data/hooks";
import { nextNumber, shownSprints, useSprintBoard } from "../../data/sprints";
import { useDurationUnits } from "../../data/time";
import { useToday } from "../../data/zone";
import { type Sprint, type SprintTask } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { TaskPanel } from "../task/TaskPanel";
import { Burndown } from "./Burndown";
import { CompleteSprintDialog, DeleteSprintDialog, SprintDialog, StartSprintDialog, useSprintCall } from "./SprintDialogs";
import styles from "./Sprints.module.css";

const route = getRouteApi("/shell/projects/$key/sprints");

/** The data a dragged task carries: its key. */
const TASK_TYPE = "application/x-glyphkeel-task";
/** The backlog's place among the lists: tasks in no sprint. */
const BACKLOG = "*backlog";

/** A date-only value as a UTC moment, so formatting never shifts the day. */
function utc(day: string): Date {
  return new Date(`${day}T00:00:00Z`);
}

type Editing = { kind: "new" } | { kind: "edit" | "start" | "complete" | "delete"; sprint: Sprint } | null;

/** A project's sprints, with the open task's panel docked on the right. */
export function SprintsScreen() {
  const { key } = route.useParams();
  const { task } = route.useSearch();
  const navigate = useNavigate();
  const open = (other: string | undefined) => {
    void navigate({ to: "/projects/$key/sprints", params: { key }, search: other === undefined ? {} : { task: other } });
  };
  return (
    <div className={styles.layout}>
      <Sprints key={key} project={key} openTask={task} onOpen={open} />
      {task !== undefined && (
        <TaskPanel
          key={task}
          taskKey={task}
          onClose={() => {
            open(undefined);
          }}
          onOpenTask={open}
          onOpenWindow={(other) => void navigate({ to: "/task/$key", params: { key: other } })}
        />
      )}
    </div>
  );
}

interface SprintsProps {
  project: string;
  openTask: string | undefined;
  onOpen: (key: string) => void;
}

/**
 * The sprints page: the active sprint with its burndown, the planned
 * sprints and the backlog. Tasks move between them by dragging, or by
 * choosing some and "Move to"; completed sprints show on request.
 */
export function Sprints({ project, openTask, onOpen }: SprintsProps) {
  const t = useT();
  const today = useToday();
  const board = useSprintBoard(project);
  const [showClosed, setShowClosed] = useState(false);
  const [editing, setEditing] = useState<Editing>(null);
  // The chosen tasks, by key, across every list.
  const [chosen, setChosen] = useState<ReadonlySet<string>>(new Set());
  const { call, run } = useSprintCall(() => {
    setChosen(new Set());
  });
  const all = board.data?.items ?? [];
  const sprints = shownSprints(all, showClosed);
  const closedCount = all.filter((sprint) => sprint.state === "closed").length;
  const active = all.find((sprint) => sprint.state === "active");
  const planned = all.filter((sprint) => sprint.state === "planned");
  const backlog = board.data?.backlog ?? [];
  const done = () => {
    setEditing(null);
  };
  /** Moves tasks to a sprint (its ID), or back to the backlog. */
  const moveTo = (target: string, keys: readonly string[]) => {
    if (keys.length === 0) {
      return;
    }
    const sprint = all.find((each) => each.id === target);
    if (target === BACKLOG) {
      run(call("sprint.remove_tasks", { tasks: [...keys] }), t("sprints-moved-backlog", { count: keys.length }), false);
    } else if (sprint) {
      run(call("sprint.add_tasks", { sprint: sprint.id, tasks: [...keys] }), t("sprints-moved", { count: keys.length, name: sprint.name }), false);
    }
  };
  const choose = (list: readonly SprintTask[], keys: Selection) => {
    const inList = new Set(list.map((task) => task.key));
    const next = new Set([...chosen].filter((key) => !inList.has(key)));
    for (const task of list) {
      if (keys === "all" || keys.has(task.key)) {
        next.add(task.key);
      }
    }
    setChosen(next);
  };
  const targets = [...(active ? [active] : []), ...planned];
  const listProps = (id: string, tasks: readonly SprintTask[], label: string) => ({
    id,
    label,
    tasks,
    chosen,
    openTask,
    onOpen,
    onChoose: (keys: Selection) => {
      choose(tasks, keys);
    },
    onDrop: (keys: readonly string[]) => {
      const inList = new Set(tasks.map((task) => task.key));
      moveTo(id, keys.filter((key) => !inList.has(key)));
    },
  });
  return (
    <section className={styles.screen} aria-label={t("sprints-title", { project })}>
      <div className={styles.controls}>
        <Button
          className="btn primary"
          onPress={() => {
            setEditing({ kind: "new" });
          }}
        >
          <Plus className="ic s" aria-hidden="true" />
          {t("sprints-new")}
        </Button>
        <Switch isSelected={showClosed} onChange={setShowClosed}>
          {t("sprints-show-closed", { count: closedCount })}
        </Switch>
        {chosen.size > 0 && (
          <div className={styles.moveBar} role="group" aria-label={t("sprints-chosen", { count: chosen.size })}>
            <span>{t("sprints-chosen", { count: chosen.size })}</span>
            <MenuTrigger>
              <Button className="btn">
                {t("sprints-move-to")}
                <ChevronDown className="ic s muted" aria-hidden="true" />
              </Button>
              <Popover className="pop menuPopover" placement="bottom start" offset={4}>
                <Menu className="menu" aria-label={t("sprints-move-to")}>
                  <MenuEntries
                    entries={[
                      ...targets.map((sprint): MenuEntry => ({ id: sprint.id, label: sprint.name, run: () => { moveTo(sprint.id, [...chosen]); } })),
                      { id: BACKLOG, label: t("sprints-backlog"), run: () => { moveTo(BACKLOG, [...chosen]); } },
                    ]}
                  />
                </Menu>
              </Popover>
            </MenuTrigger>
            <Button
              className="btn flat"
              onPress={() => {
                setChosen(new Set());
              }}
            >
              {t("sprints-clear-choice")}
            </Button>
          </div>
        )}
      </div>
      {board.error && (
        <p className={styles.note} role="alert">
          {t("sprints-error", { message: failureText(t, board.error) })}
        </p>
      )}
      {board.isPending && <p className={styles.note} aria-busy="true" />}
      {board.data && all.length === 0 && (
        <div className={styles.empty}>
          <IterationCw className="ic" aria-hidden="true" />
          <p>{t("sprints-empty")}</p>
        </div>
      )}
      <ol className={styles.sprints}>
        {sprints.map((sprint) => (
          <li key={sprint.id}>
            <SprintCard
              sprint={sprint}
              canStart={active === undefined}
              onEdit={(kind) => {
                setEditing({ kind, sprint });
              }}
            >
              <TaskList {...listProps(sprint.id, sprint.tasks ?? [], t("sprints-tasks-of", { name: sprint.name }))} readOnly={sprint.state === "closed"} />
            </SprintCard>
          </li>
        ))}
      </ol>
      {board.data && (
        <section className={styles.backlog} aria-labelledby="sprints-backlog">
          <header className={styles.cardHead}>
            <h2 id="sprints-backlog" className={styles.name}>
              {t("sprints-backlog")}
            </h2>
            <span className="muted">{t("sprints-backlog-count", { count: backlog.length })}</span>
          </header>
          <TaskList {...listProps(BACKLOG, backlog, t("sprints-backlog"))} readOnly={false} />
        </section>
      )}
      {editing?.kind === "new" && <SprintDialog project={project} suggestedName={t("sprints-default-name", { number: nextNumber(all) }).replace(/[\u2068\u2069]/g, "")} suggestedStart={nextStart(all, today)} onClose={done} />}
      {editing?.kind === "edit" && <SprintDialog project={project} sprint={editing.sprint} onClose={done} />}
      {editing?.kind === "start" && <StartSprintDialog sprint={editing.sprint} onClose={done} />}
      {editing?.kind === "complete" && <CompleteSprintDialog sprint={editing.sprint} planned={planned} onClose={done} />}
      {editing?.kind === "delete" && <DeleteSprintDialog sprint={editing.sprint} onClose={done} />}
    </section>
  );
}

/** The day a new sprint is offered to start: the day after the last planned or running one ends, or today. */
function nextStart(sprints: readonly Sprint[], today: string): string {
  const ends = sprints.filter((sprint) => sprint.state !== "closed").flatMap((sprint) => (sprint.end_date === undefined ? [] : [sprint.end_date]));
  const last = ends.sort().at(-1);
  if (last === undefined || last < today) {
    return today;
  }
  const next = new Date(`${last}T00:00:00Z`);
  next.setUTCDate(next.getUTCDate() + 1);
  return next.toISOString().slice(0, 10);
}

interface SprintCardProps {
  sprint: Sprint;
  /** No other sprint of the project is active. */
  canStart: boolean;
  onEdit: (kind: "edit" | "start" | "complete" | "delete") => void;
  children: React.ReactNode;
}

function SprintCard({ sprint, canStart, onEdit, children }: SprintCardProps) {
  const t = useT();
  const { locale } = useLocale();
  const units = useDurationUnits();
  // Completed sprints start folded; the others show their tasks.
  const [expanded, setExpanded] = useState(sprint.state !== "closed");
  const date = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeZone: "UTC" });
  const percent = new Intl.NumberFormat(locale, { style: "percent" });
  const { start_date: start, end_date: end } = sprint;
  const dates =
    start !== undefined && end !== undefined
      ? date.formatRange(utc(start), utc(end))
      : end !== undefined
        ? t("sprints-ends", { date: date.format(utc(end)) })
        : start !== undefined
          ? t("sprints-starts", { date: date.format(utc(start)) })
          : t("sprints-no-dates");
  const progress = sprint.progress;
  const share = progress.total === 0 ? 0 : progress.closed / progress.total;
  const headingId = `sprint-${sprint.id}`;
  const entries: MenuEntry[] = [
    { id: "edit", label: t("sprints-edit"), run: () => { onEdit("edit"); } },
    ...(sprint.state === "active" ? [] : (["separator", { id: "delete", label: t("sprints-delete"), run: () => { onEdit("delete"); } }] as MenuEntry[])),
  ];
  return (
    <article className={classes(styles.card, sprint.state === "active" && styles.active)} aria-labelledby={headingId}>
      <header className={styles.cardHead}>
        <h2 id={headingId} className={styles.name}>
          {sprint.name}
        </h2>
        <span className={classes("chip", sprint.state === "active" ? "hue-green" : sprint.state === "planned" ? "hue-blue" : "hue-grey")}>{t(`sprint-state-${sprint.state}`)}</span>
        <span className={classes("muted num", styles.dates)}>{dates}</span>
        <span className="muted">{t("sprints-size", { count: progress.total, estimate: formatDuration(progress.estimate_minutes, units) })}</span>
        {sprint.state === "planned" && (
          <Button
            className="btn sm"
            isDisabled={!canStart}
            aria-label={canStart ? t("sprints-start-named", { name: sprint.name }) : t("sprints-start-blocked", { name: sprint.name })}
            onPress={() => {
              onEdit("start");
            }}
          >
            <Play className="ic s" aria-hidden="true" />
            {t("sprints-start")}
          </Button>
        )}
        {sprint.state === "active" && (
          <Button
            className="btn sm"
            aria-label={t("sprints-complete-named", { name: sprint.name })}
            onPress={() => {
              onEdit("complete");
            }}
          >
            <Square className="ic s" aria-hidden="true" />
            {t("sprints-complete")}
          </Button>
        )}
        <MenuTrigger>
          <Button className="btn flat icon sm" aria-label={t("sprints-menu", { name: sprint.name })}>
            <MoreHorizontal className="ic s" />
          </Button>
          <Popover className="pop menuPopover" placement="bottom end" offset={4}>
            <Menu className="menu" aria-label={t("sprints-menu", { name: sprint.name })}>
              <MenuEntries entries={entries} />
            </Menu>
          </Popover>
        </MenuTrigger>
      </header>
      {sprint.goal !== undefined && sprint.goal !== "" && <p className={styles.goal}>{sprint.goal}</p>}
      {sprint.state !== "planned" && (
        <div className={styles.progress}>
          <div className="meter grow" role="progressbar" aria-label={t("sprints-progress-label", { name: sprint.name })} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(share * 100)}>
            <i style={{ inlineSize: `${String(Math.round(share * 100))}%` }} />
          </div>
          <span className="muted num">{t("sprints-progress", { closed: progress.closed, total: progress.total, percent: percent.format(share) })}</span>
        </div>
      )}
      {sprint.state === "active" && <Burndown sprint={sprint} />}
      {sprint.state === "closed" ? (
        <>
          <Button
            className="btn flat sm"
            aria-expanded={expanded}
            onPress={() => {
              setExpanded(!expanded);
            }}
          >
            {expanded ? <ChevronDown className="ic s" aria-hidden="true" /> : <ChevronRight className="ic s" aria-hidden="true" />}
            {t("sprints-details", { count: progress.total })}
          </Button>
          {expanded && (
            <>
              <Burndown sprint={sprint} />
              {children}
            </>
          )}
        </>
      ) : (
        children
      )}
    </article>
  );
}

interface TaskListProps {
  /** The sprint's ID, or the backlog. */
  id: string;
  label: string;
  tasks: readonly SprintTask[];
  chosen: ReadonlySet<string>;
  openTask: string | undefined;
  readOnly: boolean;
  onOpen: (key: string) => void;
  onChoose: (keys: Selection) => void;
  /** Tasks dropped here, by key. */
  onDrop: (keys: readonly string[]) => void;
}

/** Reads the keys of the tasks dropped on a list. */
async function droppedKeys(items: readonly DropItem[]): Promise<string[]> {
  const texts = await Promise.all(items.flatMap((item) => (item.kind === "text" && item.types.has(TASK_TYPE) ? [item.getText(TASK_TYPE)] : [])));
  return texts.filter((text) => text !== "");
}

/**
 * A sprint's tasks or the backlog: tasks can be chosen (for "Move to"),
 * opened, and dragged to another list, which takes them.
 */
function TaskList({ id, label, tasks, chosen, openTask, readOnly, onOpen, onChoose, onDrop }: TaskListProps) {
  const t = useT();
  const units = useDurationUnits();
  const drop = async (items: readonly DropItem[]) => {
    onDrop(await droppedKeys(items));
  };
  const { dragAndDropHooks } = useDragAndDrop({
    isDisabled: readOnly,
    getItems: (keys) => [...keys].map((key) => ({ [TASK_TYPE]: String(key), "text/plain": String(key) })),
    acceptedDragTypes: [TASK_TYPE],
    getDropOperation: () => "move",
    onRootDrop: (event) => {
      void drop(event.items);
    },
    onInsert: (event) => {
      void drop(event.items);
    },
    onItemDrop: (event) => {
      void drop(event.items);
    },
  });
  return (
    <GridList
      aria-label={label}
      className={classes(styles.tasks)}
      data-list={id}
      items={tasks}
      selectionMode={readOnly ? "none" : "multiple"}
      selectionBehavior="toggle"
      selectedKeys={new Set(tasks.filter((task) => chosen.has(task.key)).map((task) => task.key))}
      onSelectionChange={onChoose}
      onAction={(key) => {
        onOpen(String(key));
      }}
      dragAndDropHooks={dragAndDropHooks}
      renderEmptyState={() => <p className={styles.dropHere}>{t("sprints-drop-here")}</p>}
    >
      {(task) => (
        <GridListItem id={task.key} textValue={`${task.key} ${task.title}`} className={classes(styles.task, task.closed && styles.closed, openTask === task.key && styles.open)}>
          {!readOnly && (
            <CheckboxField slot="selection" className={classes(styles.check)}>
              <CheckboxButton>
              {({ isSelected }) => <span className={classes(styles.box, isSelected && styles.on)} aria-hidden="true" />}
            </CheckboxButton>
            </CheckboxField>
          )}
          <span className={classes("key", styles.key)}>{task.key}</span>
          <span className={classes("truncate", styles.title)}>{task.title}</span>
          <span className="muted">{task.type}</span>
          <span className="muted">{task.status.name}</span>
          {task.estimate_minutes !== undefined && <span className="muted num">{formatDuration(task.estimate_minutes, units)}</span>}
        </GridListItem>
      )}
    </GridList>
  );
}
