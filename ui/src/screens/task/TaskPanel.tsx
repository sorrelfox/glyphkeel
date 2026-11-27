// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { AppWindow, Bot, ChevronDown, ChevronRight, Link2, MoreHorizontal, Plus, Search, X } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type ReactNode, type RefObject } from "react";
import {
  Autocomplete,
  Button,
  Dialog as AriaDialog,
  DialogTrigger,
  Input,
  Menu,
  MenuItem,
  MenuTrigger,
  SearchField,
  useFilter,
  useLocale,
  type Selection,
} from "react-aria-components";

import { contextOf, matches, parseChord } from "../../actions/keymap";
import { classes } from "../../classes";
import { DayPopover } from "../../components/DayPicker";
import { FitPopover } from "../../components/FitPopover";
import { TASK_KEYS, type TaskKeyId } from "../../shell/screenKeys";
import { MenuEntries } from "../../components/ContextMenu";
import { useAskCloseDuplicates } from "../../components/DuplicatePrompt";
import { MoveTasksDialog } from "./MoveTasksDialog";
import { PriorityGlyph, TypeGlyph } from "../../components/Glyphs";
import { Select } from "../../components/Select";
import { useToasts } from "../../components/Toasts";
import { useProject, useUpdateTask } from "../../data/board";
import { HUES, PRIORITIES, type Hue, type Project, type Task, type TaskField, type TaskUpdateInput } from "../../data/catalog";
import { errorToast, failureText, useCall, useCommand } from "../../data/hooks";
import { useTrashTasks } from "../../data/safety";
import { useTaskDetails } from "../../data/task";
import { useT } from "../../i18n/I18n";
import { useCustomFields } from "../../data/fields";
import { customFieldId, fieldsOfTask, readOnlyFields } from "../../data/rules";
import { useEntryReason } from "../../data/useRules";
import { openTaskWindow } from "../../ipc/taskWindow";
import { useDayFormat } from "../board/CardView";
import { useDurations } from "../../data/time";
import { useToday } from "../../data/zone";
import { dueState } from "../board/model";
import { ActivityTabs, Composer, useMomentFormat } from "./Activity";
import { BacklinksSection } from "./Backlinks";
import { DescriptionSection, type DescriptionView } from "./Description";
import { FieldsSection } from "./FieldsSection";
import { FilesSection, useFileDrop } from "./Files";
import { LinksSection, SubtasksSection } from "./Sections";
import { RecurrenceNote, RepeatValue } from "./Recurrence";
import { SaveTemplateDialog } from "./SaveTemplate";
import { QuickLog, ReminderValue, TimerButton } from "./Time";
import { SprintPicker } from "./SprintPicker";
import { VersionPicker } from "./VersionPicker";
import styles from "./TaskPanel.module.css";

interface TaskPanelProps {
  taskKey: string;
  onClose: () => void;
  onOpenTask: (key: string) => void;
  /** Docked beside the board, or filling a task window of its own. */
  mode?: "docked" | "window";
  /** Shows the task window's screen in this page, where there are no other windows. */
  onOpenWindow?: (key: string) => void;
}

/**
 * The task panel, docked on the right of the board, which stays live beside
 * it, or filling a task window. Every field edits in place through the
 * engine. The description and comments show as rendered Markdown and edit
 * in the Markdown editor. Files dropped on the panel are attached to the
 * task.
 */
export function TaskPanel({ taskKey, onClose, onOpenTask, mode = "docked", onOpenWindow }: TaskPanelProps) {
  const t = useT();
  const task = useTaskDetails(taskKey);
  const loaded = task.data?.items[0];
  const project = useProject(loaded?.project);
  const readOnly = project.data?.state === "archived";
  const drop = useFileDrop(taskKey, loaded !== undefined && !readOnly);
  // The open task's own keys (E, S, P, L, D, Ctrl+Enter), set by its loaded view.
  const taskKeys = useRef<((event: ReactKeyboardEvent<HTMLElement>) => void) | null>(null);
  return (
    <aside
      className={classes(styles.panel, mode === "window" && styles.window, drop.over && styles.dropping)}
      aria-label={t("panel-label", { key: taskKey })}
      data-region="panel"
      data-drop-task={taskKey}
      tabIndex={-1}
      {...drop.handlers}
      onKeyDownCapture={(event) => {
        // Before the focused control: Ctrl+Enter on a button must not also press it.
        taskKeys.current?.(event);
      }}
      onKeyDown={(event) => {
        const target = event.target as HTMLElement;
        if (event.key === "Escape" && !event.defaultPrevented && !target.closest("input, textarea, select")) {
          event.preventDefault();
          onClose();
        }
      }}
    >
      {task.isError && (
        <div className={styles.bar}>
          <span className="grow">{failureText(t, task.error)}</span>
          <Button className="btn flat icon" aria-label={t("panel-close")} onPress={onClose}>
            <X className="ic" />
          </Button>
        </div>
      )}
      {loaded && project.data && (
        <Loaded key={loaded.id} task={loaded} project={project.data} mode={mode} onClose={onClose} onOpenTask={onOpenTask} onOpenWindow={onOpenWindow} keysRef={taskKeys} />
      )}
      {drop.over && (
        <div className={styles.dropNote} role="status">
          {t("files-drop-here", { key: taskKey })}
        </div>
      )}
    </aside>
  );
}

function Loaded({
  task,
  project,
  mode,
  onClose,
  onOpenTask,
  onOpenWindow,
  keysRef,
}: {
  task: Task;
  project: Project;
  mode: "docked" | "window";
  onClose: () => void;
  onOpenTask: (key: string) => void;
  onOpenWindow: ((key: string) => void) | undefined;
  /** Where the panel finds the handler of the task's own keys. */
  keysRef: RefObject<((event: ReactKeyboardEvent<HTMLElement>) => void) | null>;
}) {
  const t = useT();
  const toasts = useToasts();
  const update = useUpdateTask();
  const call = useCall();
  const askDuplicates = useAskCloseDuplicates();
  const durations = useDurations();
  const moment = useMomentFormat();
  const duplicate = useCommand("task.duplicate", { undoable: t("panel-duplicated") });
  const release = useCommand("task.release");
  const trash = useTrashTasks();
  const readOnly = project.state === "archived";
  const [descriptionView, setDescriptionView] = useState<DescriptionView>({ kind: "read" });
  const [moving, setMoving] = useState(false);
  const [savingTemplate, setSavingTemplate] = useState(false);
  const blockers = (task.links ?? []).filter((link) => link.kind === "blocked_by" && !link.task.closed).map((link) => link.task.key);
  const copy = (withoutSubtasks: boolean) => {
    duplicate.mutate(
      { key: task.key, without_subtasks: withoutSubtasks, client_request_id: crypto.randomUUID() },
      {
        onSuccess: (change) => {
          onOpenTask(change.task.key);
        },
      },
    );
  };
  const patch = (change: Omit<TaskUpdateInput, "key">) => {
    update.mutate({ key: task.key, ...change });
  };
  /** Sets a field, or clears it when the value is null. */
  const setOrClear = <F extends TaskField & keyof TaskUpdateInput>(field: F, value: TaskUpdateInput[F] | null) => {
    patch(value === null ? { clear: [field] } : { [field]: value });
  };
  const setStatus = (status: string) => {
    const category = project.statuses.find((each) => each.id === status)?.category;
    const closing = (category === "done" || category === "cancelled") && task.status.category !== "done" && task.status.category !== "cancelled";
    askDuplicates(task.key, closing)
      .then((close) => call("task.move", { key: task.key, status, ...(close === undefined ? {} : { close_duplicates: close }) }))
      .catch((error: unknown) => {
        toasts.show(errorToast(t, error));
      });
  };
  const estimate = task.estimate_minutes ?? null;
  const spent = task.spent_minutes;
  const left = estimate === null ? null : estimate - spent;
  const statusHue = project.statuses.find((status) => status.id === task.status.id)?.hue ?? "grey";
  const entryReason = useEntryReason();
  // Fields the task's status keeps from changing: shown disabled, with the reason below.
  const locked = readOnlyFields(project, task.status.id);
  const lockedOr = (field: string) => readOnly || locked.has(field);
  const openOwnWindow = () => {
    openTaskWindow(task.key, () => onOpenWindow?.(task.key)).catch((error: unknown) => {
      toasts.show(errorToast(t, error));
    });
  };

  // The task's own keys, from the table the shortcut sheet shows, taken
  // before the focused control sees them. They act when the focus is in
  // the panel itself, not in a text field or in a menu or dialog it opened.
  const onTaskKey = (event: ReactKeyboardEvent<HTMLElement>) => {
    if (event.defaultPrevented || event.nativeEvent.isComposing) {
      return;
    }
    const panel = event.currentTarget;
    const target = event.target as HTMLElement;
    if (!panel.contains(target) || target.closest("[role=menu], [role=listbox], [role=dialog], [role=grid], [role=treegrid]")) {
      return;
    }
    const inText = contextOf(target) === "text";
    const pressed = (id: TaskKeyId) => TASK_KEYS.some((entry) => entry.id === id && entry.keys.some((chord) => matches(parseChord(chord), event.nativeEvent)));
    if (pressed("window")) {
      // In the editors, Ctrl+Enter saves or sends; they take it first.
      if (!inText && mode === "docked") {
        event.preventDefault();
        event.stopPropagation();
        openOwnWindow();
      }
      return;
    }
    if (inText) {
      return;
    }
    if (pressed("edit")) {
      if (!lockedOr("description") && descriptionView.kind !== "edit") {
        event.preventDefault();
        event.stopPropagation();
        setDescriptionView({ kind: "edit" });
      }
      return;
    }
    const field = (["status", "priority", "labels", "due"] as const).find(pressed);
    const trigger = field && panel.querySelector<HTMLElement>(`[data-task-key="${field}"]`);
    if (trigger) {
      event.preventDefault();
      event.stopPropagation();
      if (!trigger.hasAttribute("disabled") && trigger.getAttribute("aria-disabled") !== "true") {
        trigger.focus();
        trigger.click();
      }
    }
  };
  useEffect(() => {
    keysRef.current = onTaskKey;
  });

  return (
    <>
      <div className={styles.bar}>
        <TypeGlyph glyph={task.type.glyph} label={task.type.name} />
        {task.parent !== undefined && (
          <>
            <Button className="val key" onPress={() => { if (task.parent !== undefined) onOpenTask(task.parent); }}>
              {task.parent}
            </Button>
            <ChevronRight className="ic s faint" />
          </>
        )}
        <span className={classes("key", styles.ownKey)}>{task.key}</span>
        <div className="grow" />
        <Button
          className="btn flat icon"
          aria-label={t("panel-copy-key")}
          onPress={() => {
            void navigator.clipboard.writeText(task.key).then(
              () => { toasts.show({ tone: "info", text: t("panel-copied", { key: task.key }) }); },
              () => undefined,
            );
          }}
        >
          <Link2 className="ic" />
        </Button>
        {mode === "docked" && (
          <Button className="btn flat icon" aria-label={t("panel-own-window")} onPress={openOwnWindow}>
            <AppWindow className="ic" />
          </Button>
        )}
        <MenuTrigger>
          <Button className="btn flat icon" aria-label={t("panel-menu")}>
            <MoreHorizontal className="ic" />
          </Button>
          <FitPopover className="menuPopover" placement="bottom end" offset={4}>
            <Menu className="menu" aria-label={t("panel-menu")}>
              <MenuEntries
                entries={[
                  { id: "duplicate", label: t("panel-duplicate"), disabled: readOnly, run: () => { copy(false); } },
                  ...(task.subtask_count.total > 0
                    ? [{ id: "duplicate-alone", label: t("panel-duplicate-alone"), disabled: readOnly, run: () => { copy(true); } }]
                    : []),
                  ...(task.claim
                    ? [{ id: "release", label: t("panel-release"), disabled: readOnly, run: () => { release.mutate({ key: task.key, override_claim: true }); } }]
                    : []),
                  { id: "move", label: t("panel-move"), disabled: readOnly, run: () => { setMoving(true); } },
                  { id: "save-template", label: t("panel-save-template"), disabled: readOnly, run: () => { setSavingTemplate(true); } },
                  "separator",
                  { id: "trash", label: t("trash-move", { count: 1 }), disabled: readOnly, run: () => { trash([task.key], onClose); } },
                ]}
              />
            </Menu>
          </FitPopover>
        </MenuTrigger>
        <Button className="btn flat icon" aria-label={t(mode === "window" ? "panel-close-window" : "panel-close")} onPress={onClose}>
          <X className="ic" />
        </Button>
      </div>
      {savingTemplate && (
        <SaveTemplateDialog
          task={task}
          onClose={() => {
            setSavingTemplate(false);
          }}
        />
      )}
      {moving && (
        <MoveTasksDialog
          keys={[task.key]}
          from={project.key}
          onClose={(moved) => {
            setMoving(false);
            if (moved !== undefined) {
              onOpenTask(moved);
            }
          }}
        />
      )}

      <div className={styles.body}>
        <Title task={task} readOnly={lockedOr("title")} onSave={(title) => { patch({ title }); }} />

        <div className={styles.actions}>
          <MenuTrigger>
            <Button className="btn" isDisabled={readOnly} aria-label={t("panel-status", { name: task.status.name })} data-task-key="status">
              <span className={`dot hue-${statusHue}`} />
              <span>{task.status.name}</span>
              <ChevronDown className="ic s muted" />
            </Button>
            <FitPopover className="menuPopover" placement="bottom start" offset={4}>
              <Menu className="menu" aria-label={t("panel-status-menu")}>
                <MenuEntries
                  entries={project.statuses.map((status) => {
                    const reason = status.id === task.status.id ? undefined : entryReason(project, status.id, fieldsOfTask(task));
                    return {
                      id: status.id,
                      label: status.name,
                      checked: status.id === task.status.id,
                      icon: <span className={`dot hue-${status.hue}`} />,
                      ...(reason === undefined ? {} : { disabled: true, detail: reason }),
                      run: () => { setStatus(status.id); },
                    };
                  })}
                />
              </Menu>
            </FitPopover>
          </MenuTrigger>
          {task.blocked && (
            <span className="chip hue-red" role="note" aria-label={t("panel-blocked-by", { keys: blockers.join(", "), count: blockers.length })}>
              {t("card-blocked")}
            </span>
          )}
          <TimerButton task={task} readOnly={readOnly} />
          <QuickLog task={task} readOnly={readOnly} />
          {task.claim && (
            <span className={classes("chip hue-purple", styles.claim)} role="note" aria-label={t("panel-claimed", { agent: task.claim.agent, time: moment(task.claim.until) })}>
              <Bot className="ic s" />
              {t("card-claimed", { agent: task.claim.agent })}
            </span>
          )}
        </div>

        <div className={styles.props}>
          <Prop label={t("prop-priority")}>
            <MenuTrigger>
              <Button className="val" isDisabled={lockedOr("priority")} aria-label={t("prop-priority-value", { value: t(`priority-${task.priority}`) })} data-task-key="priority">
                <PriorityGlyph priority={task.priority} />
                <Whole text={t(`priority-${task.priority}`)} />
              </Button>
              <FitPopover className="menuPopover" placement="bottom start" offset={4}>
                <Menu className="menu" aria-label={t("prop-priority")}>
                  <MenuEntries
                    entries={[...PRIORITIES].reverse().map((priority) => ({
                      id: priority,
                      label: t(`priority-${priority}`),
                      checked: priority === task.priority,
                      icon: <PriorityGlyph priority={priority} />,
                      run: () => { patch({ priority }); },
                    }))}
                  />
                </Menu>
              </FitPopover>
            </MenuTrigger>
          </Prop>
          <Prop label={t("prop-type")}>
            <MenuTrigger>
              <Button className="val" isDisabled={lockedOr("type")} aria-label={t("prop-type-value", { value: task.type.name })}>
                <TypeGlyph glyph={task.type.glyph} />
                <Whole text={task.type.name} />
              </Button>
              <FitPopover className="menuPopover" placement="bottom start" offset={4}>
                <Menu className="menu" aria-label={t("prop-type")}>
                  <MenuEntries
                    entries={project.types.map((type) => ({
                      id: type.id,
                      label: type.name,
                      checked: type.id === task.type.id,
                      icon: <TypeGlyph glyph={type.glyph} />,
                      run: () => { patch({ type: type.id }); },
                    }))}
                  />
                </Menu>
              </FitPopover>
            </MenuTrigger>
          </Prop>
          <Prop label={t("prop-start")}>
            <DateValue label={t("prop-start")} value={task.start_date ?? null} readOnly={lockedOr("start_date")} onChange={(day) => { setOrClear("start_date", day); }} />
          </Prop>
          <Prop label={t("prop-due")}>
            <DateValue label={t("prop-due")} value={task.due_date ?? null} readOnly={lockedOr("due_date")} closed={task.closed_at !== undefined} relative keyTarget="due" onChange={(day) => { setOrClear("due_date", day); }} />
          </Prop>
          <Prop label={t("prop-estimate")}>
            <DurationValue
              label={t("prop-estimate")}
              value={estimate}
              readOnly={lockedOr("estimate")}
              onChange={(minutes) => { setOrClear("estimate", minutes === null ? null : `${String(minutes)}m`); }}
            />
          </Prop>
          <Prop label={t("prop-spent")}>
            <span className={classes("num", styles.static)}>
              {spent === 0 && left === null ? <span className="faint">{t("prop-none")}</span> : <Whole text={durations.format(spent)} />}
              {left !== null && <Whole className={left < 0 ? styles.over : "faint"} text={left < 0 ? t("prop-over-by", { time: durations.format(-left) }) : t("prop-left", { time: durations.format(left) })} />}
            </span>
          </Prop>
          <Prop label={t("prop-version")}>
            <VersionPicker task={task} readOnly={lockedOr("fix_version")} onChange={(version) => { setOrClear("fix_version", version); }} />
          </Prop>
          <Prop label={t("prop-sprint")}>
            <SprintPicker task={task} readOnly={lockedOr("sprint")} onChange={(sprint) => { setOrClear("sprint", sprint); }} />
          </Prop>
          <Prop label={t("prop-labels")} wide>
            <Labels task={task} project={project} readOnly={lockedOr("labels")} onChange={(labels) => { setOrClear("labels", labels.length === 0 ? null : labels); }} />
          </Prop>
          <Prop label={t("prop-progress")}>
            {task.checklist.total > 0 ? (
              <>
                <div className="meter grow" role="img" aria-label={t("card-checklist", { done: task.checklist.done, total: task.checklist.total })}>
                  <i style={{ width: `${String(Math.round((task.checklist.done / task.checklist.total) * 100))}%` }} />
                </div>
                <span className={classes("num muted", styles.small)}>
                  {task.checklist.done}/{task.checklist.total}
                </span>
              </>
            ) : (
              <span className="faint">{t("prop-none")}</span>
            )}
          </Prop>
          <Prop label={t("prop-reminder")} full>
            <ReminderValue task={task} readOnly={readOnly} />
          </Prop>
          <Prop label={t("prop-repeats")} full>
            <RepeatValue task={task} readOnly={readOnly} />
          </Prop>
        </div>

        <LockedNote locked={locked} status={task.status.name} project={task.project} />

        <RecurrenceNote task={task} readOnly={readOnly} />

        <FieldsSection task={task} readOnly={readOnly} locked={locked} />

        <DescriptionSection task={task} readOnly={lockedOr("description")} view={descriptionView} onView={setDescriptionView} onOpenTask={onOpenTask} />

        <SubtasksSection task={task} project={project} readOnly={readOnly} onOpenTask={onOpenTask} />
        <LinksSection task={task} project={project} readOnly={readOnly} onOpenTask={onOpenTask} />
        <BacklinksSection taskKey={task.key} onOpenTask={onOpenTask} />
        <FilesSection task={task.key} files={task.files ?? []} readOnly={readOnly} />
        <ActivityTabs
          task={task}
          project={project}
          readOnly={readOnly}
          onOpenTask={onOpenTask}
          onShowDescriptionDiff={(older, newer) => {
            setDescriptionView({ kind: "history", compare: [older, newer] });
          }}
        />
      </div>
      <Composer task={task} readOnly={readOnly} />
    </>
  );
}

/** Says which fields the task's status keeps from changing, so a disabled field never goes unexplained. */
function LockedNote({ locked, status, project }: { locked: ReadonlySet<string>; status: string; project: string }) {
  const t = useT();
  const { locale } = useLocale();
  const custom = useCustomFields(project);
  const names = [...locked].flatMap((field) => {
    const id = customFieldId(field);
    if (id === undefined) {
      return [t(`rule-field-${field}`)];
    }
    const name = custom.find((each) => each.id === id)?.name;
    return name === undefined ? [] : [name];
  });
  if (names.length === 0) {
    return null;
  }
  const list = new Intl.ListFormat(locale, { style: "long", type: "conjunction" }).format(names);
  return (
    <p className={classes("meta", styles.ruleNote)} role="note">
      {t("panel-rule-read-only", { status, fields: list, count: names.length })}
    </p>
  );
}

/**
 * A phrase of a value that never breaks inside: on one line, ending in an
 * ellipsis when even its own line is too narrow, with the whole text in its
 * tooltip. The value's accessible name holds it too.
 */
function Whole({ text, className }: { text: string; className?: string | undefined }) {
  return (
    <span className={classes("truncate", className)} title={text}>
      {text}
    </span>
  );
}

function Prop({ label, children, wide = false, full = false }: { label: string; children: ReactNode; wide?: boolean; full?: boolean }) {
  return (
    <dl className={classes("prop", styles.prop, wide && styles.wide, full && styles.full)}>
      <dt>{label}</dt>
      <dd>{children}</dd>
    </dl>
  );
}

/** The title: click to edit in place; Enter saves, Escape cancels. */
function Title({ task, readOnly, onSave }: { task: Task; readOnly: boolean; onSave: (title: string) => void }) {
  const t = useT();
  const [editing, setEditing] = useState<string | null>(null);
  if (editing !== null) {
    const save = () => {
      if (editing.trim() !== "" && editing !== task.title) {
        onSave(editing);
      }
      setEditing(null);
    };
    return (
      <textarea
        className={styles.titleField}
        aria-label={t("panel-title")}
        autoFocus
        rows={2}
        value={editing}
        maxLength={500}
        onChange={(event) => {
          setEditing(event.target.value);
        }}
        onBlur={save}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            save();
          } else if (event.key === "Escape") {
            event.preventDefault();
            setEditing(null);
          }
        }}
      />
    );
  }
  return (
    <h1
      className={classes("selectable", styles.title)}
      tabIndex={readOnly ? undefined : 0}
      onClick={() => {
        if (!readOnly && !window.getSelection()?.toString()) {
          setEditing(task.title);
        }
      }}
      onKeyDown={(event) => {
        if (!readOnly && (event.key === "Enter" || event.key === "F2")) {
          event.preventDefault();
          setEditing(task.title);
        }
      }}
    >
      {task.title}
    </h1>
  );
}

/**
 * A day: shown as "Nov 24", with how far off it is under it when the cell is
 * too narrow for both on one line; edited in a day picker anchored to it.
 */
function DateValue({
  label,
  value,
  readOnly,
  closed = false,
  relative = false,
  keyTarget,
  onChange,
}: {
  label: string;
  value: string | null;
  readOnly: boolean;
  closed?: boolean;
  relative?: boolean;
  /** The task key (D) that opens it, if one does. */
  keyTarget?: TaskKeyId;
  onChange: (value: string | null) => void;
}) {
  const t = useT();
  const { locale } = useLocale();
  const day = useDayFormat();
  const [editing, setEditing] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const today = useToday();
  const state = relative ? dueState(value, closed, today) : null;
  const days = value === null ? 0 : Math.round((Date.parse(`${value}T00:00:00Z`) - Date.parse(`${today}T00:00:00Z`)) / 86_400_000);
  const away = value !== null && relative && !closed ? new Intl.RelativeTimeFormat(locale, { numeric: "auto" }).format(days, "day") : null;
  return (
    <>
      <Button
        ref={trigger}
        className={classes("val num", styles.dateValue)}
        isDisabled={readOnly}
        aria-label={away === null ? t("prop-date-value", { name: label, value: value === null ? t("prop-none") : day(value) }) : t("prop-date-value-away", { name: label, value: day(value ?? today), away })}
        data-task-key={keyTarget}
        data-date-value={label}
        onPress={() => {
          setEditing(true);
        }}
      >
        {value === null ? (
          <span className="faint">{t("prop-none")}</span>
        ) : (
          <>
            <span className={classes("meta", styles.dateText, state === "overdue" && "overdue", (state === "soon" || state === "today") && "soon")}>{day(value)}</span>
            {away !== null && <span className={classes("faint", styles.relative)} title={away}>{away}</span>}
          </>
        )}
      </Button>
      <DayPopover
        triggerRef={trigger}
        isOpen={editing}
        label={label}
        value={value}
        onDone={(next) => {
          setEditing(false);
          if (next !== undefined && next !== value) {
            onChange(next);
          }
        }}
      />
    </>
  );
}

/** A duration: shown as "1h 30m", typed as "90m", "1.5h", "1:30", "2d". */
function DurationValue({
  label,
  value,
  readOnly,
  onChange,
}: {
  label: string;
  value: number | null;
  readOnly: boolean;
  onChange: (value: number | null) => void;
}) {
  const t = useT();
  const durations = useDurations();
  const [editing, setEditing] = useState<string | null>(null);
  const [invalid, setInvalid] = useState(false);
  if (editing !== null) {
    const save = () => {
      if (editing.trim() === "") {
        if (value !== null) {
          onChange(null);
        }
        setEditing(null);
        return;
      }
      const minutes = durations.parse(editing);
      if (minutes === null) {
        setInvalid(true);
        return;
      }
      if (minutes !== value) {
        onChange(minutes);
      }
      setEditing(null);
    };
    return (
      <span className={classes("field", styles.inlineField, invalid && styles.invalid)}>
        <input
          aria-label={label}
          aria-invalid={invalid}
          autoFocus
          placeholder={t("prop-duration-placeholder")}
          value={editing}
          onChange={(event) => {
            setEditing(event.target.value);
            setInvalid(false);
          }}
          onBlur={save}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              save();
            } else if (event.key === "Escape") {
              event.preventDefault();
              setEditing(null);
            }
          }}
        />
      </span>
    );
  }
  return (
    <Button className="val num" isDisabled={readOnly} aria-label={t("prop-date-value", { name: label, value: value === null ? t("prop-none") : durations.format(value) })} onPress={() => { setEditing(value === null ? "" : durations.format(value)); }}>
      {value === null ? <span className="faint">{t("prop-none")}</span> : <Whole text={durations.format(value)} />}
    </Button>
  );
}

const CREATE_LABEL = "create-label";

/**
 * The task's labels and the "+" that changes them. With labels in the
 * project it opens a picker with a search field, which also offers to
 * create a label by the typed name; without any, a small form that creates
 * the first one. A new label gets the colour the engine suggests and goes
 * on the task at once.
 */
function Labels({ task, project, readOnly, onChange }: { task: Task; project: Project; readOnly: boolean; onChange: (labels: string[]) => void }) {
  const t = useT();
  const createLabel = useCreateLabel(task, project);
  const [query, setQuery] = useState("");
  const [picking, setPicking] = useState(false);
  const { contains } = useFilter({ sensitivity: "base" });
  const typed = query.trim();
  const exists = project.labels.some((label) => label.name.localeCompare(typed, undefined, { sensitivity: "base" }) === 0);
  const offerCreate = typed !== "" && !exists;
  return (
    <>
      {task.labels.map((label) => (
        <span key={label.id} className={classes(`chip hue-${label.hue}`, styles.labelChip)}>
          <Whole text={label.name} />
        </span>
      ))}
      {project.labels.length === 0 ? (
        <DialogTrigger>
          <Button className="btn flat icon sm" isDisabled={readOnly} aria-label={t("prop-labels-edit")} data-task-key="labels">
            <Plus className="ic s" />
          </Button>
          <FitPopover placement="bottom start" offset={4}>
            <AriaDialog className={classes(styles.labelDialog)} aria-label={t("label-create-title")}>
              {({ close }) => (
                <NewLabelForm
                  suggested={project.next_label_hue}
                  onCreate={(name, hue) => {
                    createLabel(name, hue);
                    close();
                  }}
                />
              )}
            </AriaDialog>
          </FitPopover>
        </DialogTrigger>
      ) : (
        <MenuTrigger
          isOpen={picking}
          onOpenChange={(isOpen) => {
            setPicking(isOpen);
            if (!isOpen) {
              setQuery("");
            }
          }}
        >
          <Button className="btn flat icon sm" isDisabled={readOnly} aria-label={t("prop-labels-edit")} data-task-key="labels">
            <Plus className="ic s" />
          </Button>
          <FitPopover className={classes("menuPopover", styles.labelPicker)} placement="bottom start" offset={4}>
            <Autocomplete
              inputValue={query}
              onInputChange={setQuery}
              filter={(text, _input, node) => node.key === CREATE_LABEL || contains(text, typed)}
            >
              <SearchField
                className={classes("field", styles.labelSearch)}
                aria-label={t("label-search")}
                autoFocus
                onKeyDown={(event) => {
                  // Escape clears the search first; with nothing typed it closes the picker.
                  if (event.key === "Escape" && query === "") {
                    setPicking(false);
                  }
                }}
              >
                <Search className="ic s" aria-hidden="true" />
                <Input placeholder={t("label-search-placeholder")} />
              </SearchField>
              <Menu
                className="menu"
                aria-label={t("prop-labels")}
                selectionMode="multiple"
                selectedKeys={new Set(task.labels.map((label) => label.id))}
                onSelectionChange={(keys: Selection) => {
                  if (keys !== "all" && keys.has(CREATE_LABEL)) {
                    createLabel(typed, null);
                    setQuery("");
                    return;
                  }
                  onChange(project.labels.filter((label) => keys === "all" || keys.has(label.id)).map((label) => label.id));
                }}
              >
                {project.labels.map((label) => (
                  <MenuItem key={label.id} id={label.id} className="item" textValue={label.name}>
                    {({ isSelected }) => (
                      <>
                        <input type="checkbox" className="check" checked={isSelected} readOnly tabIndex={-1} aria-hidden="true" />
                        <span className={`dot hue-${label.hue}`} />
                        <span className="grow truncate">{label.name}</span>
                      </>
                    )}
                  </MenuItem>
                ))}
                {offerCreate && (
                  <MenuItem key={CREATE_LABEL} id={CREATE_LABEL} className="item" textValue={typed}>
                    <Plus className="ic s" aria-hidden="true" />
                    <span className={`dot hue-${project.next_label_hue}`} />
                    <span className="grow truncate">{t("label-create-named", { name: typed })}</span>
                  </MenuItem>
                )}
              </Menu>
            </Autocomplete>
          </FitPopover>
        </MenuTrigger>
      )}
    </>
  );
}

/** The first label of a project: a name and a colour, the suggested one until another is picked. */
function NewLabelForm({ suggested, onCreate }: { suggested: Hue; onCreate: (name: string, hue: Hue | null) => void }) {
  const t = useT();
  const [name, setName] = useState("");
  const [hue, setHue] = useState<Hue | null>(null);
  return (
    <form
      className={styles.labelForm}
      onSubmit={(event) => {
        event.preventDefault();
        if (name.trim() !== "") {
          onCreate(name.trim(), hue);
        }
      }}
    >
      <p className={classes("muted", styles.small)}>{t("label-create-first")}</p>
      <span className="field">
        <input aria-label={t("label-name")} placeholder={t("label-name")} value={name} autoFocus onChange={(event) => { setName(event.target.value); }} />
      </span>
      <div className={styles.labelFormRow}>
        <Select
          label={t("label-colour")}
          value={hue ?? suggested}
          onChange={setHue}
          options={HUES.map((each) => ({ id: each, label: t(`hue-${each}`), icon: <span className={`dot hue-${each}`} /> }))}
        />
        <span className="grow" />
        <button type="submit" className="btn primary" disabled={name.trim() === ""}>
          {t("label-create")}
        </button>
      </div>
    </form>
  );
}

/**
 * Creates a label in the task's project and puts it on the task. They are
 * two changes: Undo takes back the label on the task first, then the label.
 */
function useCreateLabel(task: Task, project: Project): (name: string, hue: Hue | null) => void {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  return (name, hue) => {
    const wanted = name.trim();
    call("label.create", hue === null ? { project: project.key, name: wanted } : { project: project.key, name: wanted, hue })
      .then((change) => {
        const created = change.project.labels.find((label) => label.name.localeCompare(wanted, undefined, { sensitivity: "base" }) === 0);
        if (!created) {
          return undefined;
        }
        return call("task.update", { key: task.key, add_labels: [created.id] });
      })
      .catch((error: unknown) => {
        toasts.show(errorToast(t, error));
      });
  };
}
