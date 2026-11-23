// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ChevronDown, ChevronRight, Link2, Plus, X } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Button, Menu, MenuItem, MenuTrigger, Popover, useLocale, type Selection } from "react-aria-components";

import { classes } from "../../classes";
import { MenuEntries } from "../../components/ContextMenu";
import { PriorityGlyph, TypeGlyph } from "../../components/Glyphs";
import { useToasts } from "../../components/Toasts";
import { useProject, useTask, useUpdateTask } from "../../data/board";
import { PRIORITIES, type Project, type Task, type TaskField, type TaskUpdateInput } from "../../data/catalog";
import { errorToast, useCall, useOp } from "../../data/hooks";
import { useT } from "../../i18n/I18n";
import { useDayFormat } from "../board/CardView";
import { dueState, formatMinutes, isoDay, parseDuration } from "../board/model";
import styles from "./TaskPanel.module.css";

interface TaskPanelProps {
  taskKey: string;
  onClose: () => void;
  onOpenTask: (key: string) => void;
}

/**
 * The task panel, docked on the right of the board, which stays live beside
 * it. Every field edits in place through the engine. The description is
 * plain text until the Markdown editor arrives.
 */
export function TaskPanel({ taskKey, onClose, onOpenTask }: TaskPanelProps) {
  const t = useT();
  const task = useTask(taskKey);
  const loaded = task.data?.items[0];
  const project = useProject(loaded?.project);
  return (
    <aside
      className={styles.panel}
      aria-label={t("panel-label", { key: taskKey })}
      data-region="panel"
      tabIndex={-1}
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
          <span className="grow">{task.error.error.message}</span>
          <Button className="btn flat icon" aria-label={t("panel-close")} onPress={onClose}>
            <X className="ic" />
          </Button>
        </div>
      )}
      {loaded && project.data && <Loaded task={loaded} project={project.data} onClose={onClose} onOpenTask={onOpenTask} />}
    </aside>
  );
}

function Loaded({ task, project, onClose, onOpenTask }: { task: Task; project: Project; onClose: () => void; onOpenTask: (key: string) => void }) {
  const t = useT();
  const toasts = useToasts();
  const update = useUpdateTask();
  const call = useCall();
  const settings = useOp("settings.get", {});
  const readOnly = project.state === "archived";
  const patch = (change: Omit<TaskUpdateInput, "key">) => {
    update.mutate({ key: task.key, ...change });
  };
  /** Sets a field, or clears it when the value is null. */
  const setOrClear = <F extends TaskField & keyof TaskUpdateInput>(field: F, value: TaskUpdateInput[F] | null) => {
    patch(value === null ? { clear: [field] } : { [field]: value });
  };
  const setStatus = (status: string) => {
    call("task.move", { key: task.key, status }).catch((error: unknown) => {
      toasts.show(errorToast(t, error));
    });
  };
  const estimate = task.estimate_minutes ?? null;
  const spent = task.spent_minutes;
  const left = estimate === null ? null : estimate - spent;
  const statusHue = project.statuses.find((status) => status.id === task.status.id)?.hue ?? "grey";

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
        <Button className="btn flat icon" aria-label={t("panel-close")} onPress={onClose}>
          <X className="ic" />
        </Button>
      </div>

      <div className={styles.body}>
        <Title task={task} readOnly={readOnly} onSave={(title) => { patch({ title }); }} />

        <div className={styles.actions}>
          <MenuTrigger>
            <Button className="btn" isDisabled={readOnly} aria-label={t("panel-status", { name: task.status.name })}>
              <span className={`dot hue-${statusHue}`} />
              <span>{task.status.name}</span>
              <ChevronDown className="ic s muted" />
            </Button>
            <Popover className="pop menuPopover" placement="bottom start" offset={4}>
              <Menu className="menu" aria-label={t("panel-status-menu")}>
                <MenuEntries
                  entries={project.statuses.map((status) => ({
                    id: status.id,
                    label: status.name,
                    checked: status.id === task.status.id,
                    icon: <span className={`dot hue-${status.hue}`} />,
                    run: () => { setStatus(status.id); },
                  }))}
                />
              </Menu>
            </Popover>
          </MenuTrigger>
        </div>

        <div className={styles.props}>
          <Prop label={t("prop-priority")}>
            <MenuTrigger>
              <Button className="val" isDisabled={readOnly} aria-label={t("prop-priority-value", { value: t(`priority-${task.priority}`) })}>
                <PriorityGlyph priority={task.priority} />
                {t(`priority-${task.priority}`)}
              </Button>
              <Popover className="pop menuPopover" placement="bottom start" offset={4}>
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
              </Popover>
            </MenuTrigger>
          </Prop>
          <Prop label={t("prop-type")}>
            <MenuTrigger>
              <Button className="val" isDisabled={readOnly} aria-label={t("prop-type-value", { value: task.type.name })}>
                <TypeGlyph glyph={task.type.glyph} />
                {task.type.name}
              </Button>
              <Popover className="pop menuPopover" placement="bottom start" offset={4}>
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
              </Popover>
            </MenuTrigger>
          </Prop>
          <Prop label={t("prop-start")}>
            <DateValue label={t("prop-start")} value={task.start_date ?? null} readOnly={readOnly} onChange={(day) => { setOrClear("start_date", day); }} />
          </Prop>
          <Prop label={t("prop-due")}>
            <DateValue label={t("prop-due")} value={task.due_date ?? null} readOnly={readOnly} closed={task.closed_at !== undefined} relative onChange={(day) => { setOrClear("due_date", day); }} />
          </Prop>
          <Prop label={t("prop-estimate")}>
            <DurationValue
              label={t("prop-estimate")}
              value={estimate}
              readOnly={readOnly}
              hoursPerDay={settings.data?.hours_per_day}
              daysPerWeek={settings.data?.days_per_week}
              onChange={(minutes) => { setOrClear("estimate", minutes === null ? null : `${String(minutes)}m`); }}
            />
          </Prop>
          <Prop label={t("prop-spent")}>
            <span className={classes("num", styles.static)}>
              {spent === 0 && left === null ? <span className="faint">{t("prop-none")}</span> : formatMinutes(spent)}
              {left !== null && (
                <span className={left < 0 ? styles.over : "faint"}>
                  {left < 0 ? t("prop-over-by", { time: formatMinutes(-left) }) : t("prop-left", { time: formatMinutes(left) })}
                </span>
              )}
            </span>
          </Prop>
          <Prop label={t("prop-labels")} wide>
            <Labels task={task} project={project} readOnly={readOnly} onChange={(labels) => { setOrClear("labels", labels.length === 0 ? null : labels); }} />
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
        </div>

        <Description task={task} readOnly={readOnly} onSave={(description) => { patch({ description }); }} />

      </div>
    </>
  );
}

function Prop({ label, children, wide = false }: { label: string; children: ReactNode; wide?: boolean }) {
  return (
    <dl className={classes("prop", styles.prop, wide && styles.wide)}>
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

/** A day: shown as "Nov 24", edited with the platform's date field; empty clears it. */
function DateValue({
  label,
  value,
  readOnly,
  closed = false,
  relative = false,
  onChange,
}: {
  label: string;
  value: string | null;
  readOnly: boolean;
  closed?: boolean;
  relative?: boolean;
  onChange: (value: string | null) => void;
}) {
  const t = useT();
  const { locale } = useLocale();
  const day = useDayFormat();
  const [editing, setEditing] = useState(false);
  const today = isoDay(new Date());
  if (editing) {
    return (
      <span className={classes("field", styles.inlineField)}>
        <input
          type="date"
          aria-label={label}
          autoFocus
          defaultValue={value ?? ""}
          onBlur={(event) => {
            const next = event.target.value === "" ? null : event.target.value;
            if (next !== value) {
              onChange(next);
            }
            setEditing(false);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.currentTarget.blur();
            } else if (event.key === "Escape") {
              event.preventDefault();
              setEditing(false);
            }
          }}
        />
      </span>
    );
  }
  const state = relative ? dueState(value, closed, today) : null;
  const days = value === null ? 0 : Math.round((Date.parse(`${value}T00:00:00Z`) - Date.parse(`${today}T00:00:00Z`)) / 86_400_000);
  return (
    <Button className="val num" isDisabled={readOnly} aria-label={t("prop-date-value", { name: label, value: value === null ? t("prop-none") : day(value) })} onPress={() => { setEditing(true); }}>
      {value === null ? (
        <span className="faint">{t("prop-none")}</span>
      ) : (
        <>
          <span className={classes("meta", styles.dateText, state === "overdue" && "overdue", (state === "soon" || state === "today") && "soon")}>{day(value)}</span>
          {relative && !closed && <span className="faint">{new Intl.RelativeTimeFormat(locale, { numeric: "auto" }).format(days, "day")}</span>}
        </>
      )}
    </Button>
  );
}

/** A duration: shown as "1h 30m", typed as "90m", "1.5h", "1:30", "2d". */
function DurationValue({
  label,
  value,
  readOnly,
  hoursPerDay,
  daysPerWeek,
  onChange,
}: {
  label: string;
  value: number | null;
  readOnly: boolean;
  hoursPerDay: number | undefined;
  daysPerWeek: number | undefined;
  onChange: (value: number | null) => void;
}) {
  const t = useT();
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
      const minutes = parseDuration(editing, hoursPerDay, daysPerWeek);
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
    <Button className="val num" isDisabled={readOnly} aria-label={t("prop-date-value", { name: label, value: value === null ? t("prop-none") : formatMinutes(value) })} onPress={() => { setEditing(value === null ? "" : formatMinutes(value)); }}>
      {value === null ? <span className="faint">{t("prop-none")}</span> : formatMinutes(value)}
    </Button>
  );
}

function Labels({ task, project, readOnly, onChange }: { task: Task; project: Project; readOnly: boolean; onChange: (labels: string[]) => void }) {
  const t = useT();
  return (
    <>
      {task.labels.map((label) => (
        <span key={label.id} className={`chip hue-${label.hue}`}>
          {label.name}
        </span>
      ))}
      <MenuTrigger>
        <Button className="btn flat icon sm" isDisabled={readOnly || project.labels.length === 0} aria-label={t("prop-labels-edit")}>
          <Plus className="ic s" />
        </Button>
        <Popover className="pop menuPopover" placement="bottom start" offset={4}>
          <Menu
            className="menu"
            aria-label={t("prop-labels")}
            selectionMode="multiple"
            selectedKeys={new Set(task.labels.map((label) => label.id))}
            onSelectionChange={(keys: Selection) => {
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
          </Menu>
        </Popover>
      </MenuTrigger>
    </>
  );
}

/**
 * The description, as text for now: Edit (or `E`) swaps in a text area;
 * Ctrl+Enter saves, Escape cancels.
 */
function Description({ task, readOnly, onSave }: { task: Task; readOnly: boolean; onSave: (text: string) => void }) {
  const t = useT();
  const [editing, setEditing] = useState<string | null>(null);
  const save = () => {
    if (editing !== null && editing !== task.description) {
      onSave(editing);
    }
    setEditing(null);
  };
  return (
    <section className={styles.section} aria-label={t("panel-description")}>
      <div className={styles.sectionHead}>
        <span className="caption grow">{t("panel-description")}</span>
        {editing === null ? (
          <Button className="btn flat sm" isDisabled={readOnly} onPress={() => { setEditing(task.description); }}>
            {t("panel-edit")}
          </Button>
        ) : (
          <>
            <Button className="btn flat sm" onPress={() => { setEditing(null); }}>
              {t("dialog-cancel")}
            </Button>
            <Button className="btn primary sm" onPress={save}>
              {t("panel-save")}
            </Button>
          </>
        )}
      </div>
      {editing === null ? (
        task.description === "" ? (
          <p className="faint">{t("panel-no-description")}</p>
        ) : (
          <div className={classes("md", styles.description)}>{task.description}</div>
        )
      ) : (
        <textarea
          className={styles.descriptionField}
          aria-label={t("panel-description")}
          autoFocus
          rows={10}
          value={editing}
          onChange={(event) => {
            setEditing(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter" && event.ctrlKey) {
              event.preventDefault();
              save();
            } else if (event.key === "Escape") {
              event.preventDefault();
              setEditing(null);
            }
          }}
        />
      )}
    </section>
  );
}
