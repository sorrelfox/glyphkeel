// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The list's cells. Title, status, priority and due date edit in place: a
// double click (or F2) on the title, a click on the others' values.

import { Check } from "lucide-react";
import { createContext, use, useRef, useState, type ReactNode } from "react";
import { Menu, MenuItem, Popover, Text, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { PriorityGlyph, RecurringMark, TypeGlyph, usePriorityName } from "../../components/Glyphs";
import { PRIORITIES, type Priority } from "../../data/catalog";
import { useProject } from "../../data/board";
import { fieldsOfCard } from "../../data/rules";
import { useDurations } from "../../data/time";
import { useFieldText } from "../../data/fields";
import { useEntryReason } from "../../data/useRules";
import { type CustomField, type ListedTask } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { dueState } from "../board/model";
import { isCustomColumn, type ColumnId } from "./model";
import styles from "./List.module.css";

/** What a cell can change, and which cell is being edited. */
export interface CellActions {
  today: string;
  /** The colour of a task's status. */
  hueOf: (task: ListedTask) => string;
  editing: { key: string; column: ColumnId } | null;
  setEditing: (editing: { key: string; column: ColumnId } | null) => void;
  rename: (task: ListedTask, title: string) => void;
  /** Moves the task to status `status` (an ID); `closing` when that status closes it. */
  setStatus: (task: ListedTask, status: string, closing: boolean) => void;
  setPriority: (task: ListedTask, priority: Priority) => void;
  setDue: (task: ListedTask, day: string | null) => void;
}

export const CellContext = createContext<CellActions | null>(null);

function useCellActions(): CellActions {
  const actions = use(CellContext);
  if (!actions) {
    throw new Error("list cells need a CellContext");
  }
  return actions;
}

/** Formats are costly to make and rows are many: one of each per locale. */
const formats = new Map<string, Intl.DateTimeFormat | Intl.NumberFormat>();

function dateFormat(locale: string, utc: boolean): Intl.DateTimeFormat {
  const key = `${locale}|date|${String(utc)}`;
  let format = formats.get(key);
  if (!(format instanceof Intl.DateTimeFormat)) {
    format = new Intl.DateTimeFormat(locale, { month: "short", day: "numeric", ...(utc ? { timeZone: "UTC" } : {}) });
    formats.set(key, format);
  }
  return format;
}

function percentFormat(locale: string): Intl.NumberFormat {
  const key = `${locale}|percent`;
  let format = formats.get(key);
  if (!(format instanceof Intl.NumberFormat)) {
    format = new Intl.NumberFormat(locale, { style: "percent" });
    formats.set(key, format);
  }
  return format;
}

/** A day (`YYYY-MM-DD`, no time zone) as "Nov 24". */
export function useDayFormat(): (day: string) => string {
  const { locale } = useLocale();
  const format = dateFormat(locale, true);
  return (day) => format.format(new Date(`${day}T00:00:00Z`));
}

/** A date-time as a short date, in the person's locale and time zone. */
function useMomentFormat(): (moment: string) => string {
  const { locale } = useLocale();
  const format = dateFormat(locale, false);
  return (moment) => format.format(new Date(moment));
}

function Empty() {
  return (
    <span className="faint" aria-hidden="true">
      —
    </span>
  );
}

function TitleCell({ task }: { task: ListedTask }) {
  const t = useT();
  const actions = useCellActions();
  const editing = actions.editing?.key === task.key && actions.editing.column === "title";
  if (editing) {
    return <TitleEditor task={task} />;
  }
  return (
    <span
      className={styles.title}
      onDoubleClick={(event) => {
        event.stopPropagation();
        actions.setEditing({ key: task.key, column: "title" });
      }}
    >
      <TypeGlyph glyph={task.type.glyph} label={task.type.name} />
      <span className={classes("truncate", task.closed_at !== undefined && styles.closed)}>{task.title}</span>
      {task.recurring === true && <RecurringMark />}
      {task.blocked && <span className="chip hue-red">{t("list-blocked")}</span>}
    </span>
  );
}

function TitleEditor({ task }: { task: ListedTask }) {
  const t = useT();
  const actions = useCellActions();
  const [text, setText] = useState(task.title);
  const finish = (save: boolean) => {
    actions.setEditing(null);
    const title = text.trim();
    if (save && title !== "" && title !== task.title) {
      actions.rename(task, title);
    }
  };
  return (
    <span className={classes("field", styles.inlineField)}>
      <input
        aria-label={t("list-edit-title", { key: task.key })}
        autoFocus
        value={text}
        onChange={(event) => {
          setText(event.target.value);
        }}
        onClick={(event) => {
          event.stopPropagation();
        }}
        onDoubleClick={(event) => {
          event.stopPropagation();
        }}
        onBlur={() => {
          finish(true);
        }}
        onKeyDown={(event) => {
          event.stopPropagation();
          if (event.key === "Enter") {
            finish(true);
          } else if (event.key === "Escape") {
            event.preventDefault();
            finish(false);
          }
        }}
      />
    </span>
  );
}

/**
 * A value that opens a menu of choices. Rows are many, so the cell is a
 * plain button; the menu exists only while it is open.
 */
function ChoiceMenu({ label, value, children }: { label: string; value: ReactNode; children: (close: () => void) => ReactNode }) {
  const trigger = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        ref={trigger}
        type="button"
        className={classes(styles.cellButton)}
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={(event) => {
          event.stopPropagation();
          setOpen(true);
        }}
      >
        {value}
      </button>
      {open && (
        <Popover className="pop menuPopover" triggerRef={trigger} isOpen onOpenChange={setOpen} placement="bottom start" offset={4}>
          {children(() => {
            setOpen(false);
          })}
        </Popover>
      )}
    </>
  );
}

function StatusMenu({ task, close }: { task: ListedTask; close: () => void }) {
  const actions = useCellActions();
  const project = useProject(task.project);
  const statuses = project.data?.statuses ?? [];
  const entryReason = useEntryReason();
  // The statuses whose rules keep the task out, with the reason under the name.
  const reasons = new Map(statuses.flatMap((status) => {
    const reason = status.id === task.status.id ? undefined : entryReason(project.data, status.id, fieldsOfCard(task));
    return reason === undefined ? [] : [[status.id, reason] as const];
  }));
  return (
    <Menu
      className="menu"
      aria-label={task.key}
      items={statuses}
      disabledKeys={[...reasons.keys()]}
      autoFocus="first"
      onAction={(id) => {
        close();
        if (id !== task.status.id) {
          const category = statuses.find((status) => status.id === id)?.category;
          actions.setStatus(task, String(id), category === "done" || category === "cancelled");
        }
      }}
    >
      {(status) => (
        <MenuItem id={status.id} className="item" textValue={status.name}>
          <span className={`dot hue-${status.hue}`} />
          {reasons.has(status.id) ? (
            <span className="grow menuText">
              <Text slot="label" className="truncate">
                {status.name}
              </Text>
              <Text slot="description" className="menuDetail">
                {reasons.get(status.id)}
              </Text>
            </span>
          ) : (
            <span className="grow truncate">{status.name}</span>
          )}
          {status.id === task.status.id && <Check className="ic s" aria-hidden="true" />}
        </MenuItem>
      )}
    </Menu>
  );
}

function StatusCell({ task }: { task: ListedTask }) {
  const t = useT();
  const hue = useCellActions().hueOf(task);
  return (
    <ChoiceMenu
      label={t("list-edit-status", { key: task.key, value: task.status.name })}
      value={
        <span className={`pill hue-${hue}`}>
          <span className="dot" />
          {task.status.name}
        </span>
      }
    >
      {(close) => <StatusMenu task={task} close={close} />}
    </ChoiceMenu>
  );
}

function PriorityCell({ task }: { task: ListedTask }) {
  const t = useT();
  const name = usePriorityName();
  const actions = useCellActions();
  const items = PRIORITIES.toReversed().map((priority) => ({ id: priority }));
  return (
    <ChoiceMenu
      label={t("list-edit-priority", { key: task.key, value: name(task.priority) })}
      value={
        <span className={styles.priority}>
          <PriorityGlyph priority={task.priority} />
          <span className="truncate">{name(task.priority)}</span>
        </span>
      }
    >
      {(close) => (
        <Menu
          className="menu"
          aria-label={task.key}
          items={items}
          autoFocus="first"
          onAction={(id) => {
            close();
            const priority = PRIORITIES.find((candidate) => candidate === id);
            if (priority !== undefined && priority !== task.priority) {
              actions.setPriority(task, priority);
            }
          }}
        >
          {(item) => (
            <MenuItem id={item.id} className="item" textValue={name(item.id)}>
              <PriorityGlyph priority={item.id} />
              <span className="grow truncate">{name(item.id)}</span>
              {item.id === task.priority && <Check className="ic s" aria-hidden="true" />}
            </MenuItem>
          )}
        </Menu>
      )}
    </ChoiceMenu>
  );
}

function DueCell({ task }: { task: ListedTask }) {
  const t = useT();
  const day = useDayFormat();
  const actions = useCellActions();
  const editing = actions.editing?.key === task.key && actions.editing.column === "due";
  const due = task.due_date ?? null;
  if (editing) {
    return (
      <span className={classes("field", styles.inlineField)}>
        <input
          type="date"
          aria-label={t("list-edit-due", { key: task.key })}
          autoFocus
          defaultValue={due ?? ""}
          onClick={(event) => {
            event.stopPropagation();
          }}
          onBlur={(event) => {
            const next = event.target.value === "" ? null : event.target.value;
            actions.setEditing(null);
            if (next !== due) {
              actions.setDue(task, next);
            }
          }}
          onKeyDown={(event) => {
            event.stopPropagation();
            if (event.key === "Enter") {
              event.currentTarget.blur();
            } else if (event.key === "Escape") {
              event.preventDefault();
              actions.setEditing(null);
            }
          }}
        />
      </span>
    );
  }
  const state = dueState(due, task.closed_at !== undefined, actions.today);
  return (
    <button
      type="button"
      className={classes(styles.cellButton)}
      aria-label={t("list-edit-due-value", { key: task.key, value: due === null ? t("list-none") : day(due) })}
      onClick={(event) => {
        event.stopPropagation();
        actions.setEditing({ key: task.key, column: "due" });
      }}
    >
      {due === null ? <Empty /> : <span className={classes("meta", styles.date, state === "overdue" && "overdue", (state === "soon" || state === "today") && "soon")}>{day(due)}</span>}
    </button>
  );
}

/** A custom field's value in its cell: numbers and dates in the person's locale, links as written. */
function FieldCell({ column, task, field }: { column: ColumnId; task: ListedTask; field: CustomField | undefined }) {
  const text = useFieldText();
  const value = task.fields?.[column.slice(3)];
  if (value === undefined || field === undefined) {
    return <Empty />;
  }
  if (field.kind === "boolean") {
    return value === true ? <Check className="ic s" aria-label={text("boolean", true)} /> : <span className="muted">{text("boolean", false)}</span>;
  }
  const shown = text(field.kind, value);
  return (
    <span className={classes("truncate", (field.kind === "integer" || field.kind === "decimal") && "num")} title={shown}>
      {shown}
    </span>
  );
}

/** The content of one cell. */
export function Cell({ column, task, field }: { column: ColumnId; task: ListedTask; field?: CustomField | undefined }) {
  if (isCustomColumn(column)) {
    return <FieldCell column={column} task={task} field={field} />;
  }
  return <BuiltinCell column={column} task={task} />;
}

function BuiltinCell({ column, task }: { column: Exclude<ColumnId, `cf.${string}`>; task: ListedTask }) {
  const { format } = useDurations();
  const { locale } = useLocale();
  const day = useDayFormat();
  const moment = useMomentFormat();
  const duration = (minutes: number | undefined) => (minutes === undefined ? <Empty /> : <span className="num">{format(minutes)}</span>);
  switch (column) {
    case "key":
      return <span className="key">{task.key}</span>;
    case "title":
      return <TitleCell task={task} />;
    case "project":
      return <span className="key">{task.project}</span>;
    case "type":
      return (
        <span className={styles.priority}>
          <TypeGlyph glyph={task.type.glyph} />
          <span className="truncate">{task.type.name}</span>
        </span>
      );
    case "status":
      return <StatusCell task={task} />;
    case "priority":
      return <PriorityCell task={task} />;
    case "labels":
      return (
        <span className={styles.labels}>
          {task.labels.map((label) => (
            <span key={label.id} className={`chip hue-${label.hue}`}>
              {label.name}
            </span>
          ))}
        </span>
      );
    case "version":
      return task.fix_version === undefined ? (
        <Empty />
      ) : (
        <span className="truncate" title={task.fix_version.name}>
          {task.fix_version.name}
        </span>
      );
    case "start":
      return task.start_date === undefined ? <Empty /> : <span className="meta">{day(task.start_date)}</span>;
    case "due":
      return <DueCell task={task} />;
    case "estimate":
      return duration(task.estimate_minutes);
    case "spent":
      return task.spent_minutes === 0 ? <Empty /> : duration(task.spent_minutes);
    case "remaining":
      return duration(task.remaining_minutes);
    case "progress":
      return <span className="num">{percentFormat(locale).format(task.progress_percent / 100)}</span>;
    case "created":
      return <span className="muted num">{moment(task.created_at)}</span>;
    case "updated":
      return <span className="muted num">{moment(task.updated_at)}</span>;
  }
}
