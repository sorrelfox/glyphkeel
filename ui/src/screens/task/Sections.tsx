// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { closestCenter, DndContext, PointerSensor, useSensor, useSensors, type DragEndEvent, type Modifier } from "@dnd-kit/core";
import { arrayMove, SortableContext, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS as DndCss } from "@dnd-kit/utilities";
import { ArrowDown, ArrowUp, GripVertical, MoreHorizontal, Plus, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Button, Dialog, DialogTrigger, Group, Input, Label, Menu, MenuTrigger, NumberField, Popover } from "react-aria-components";

import { classes } from "../../classes";
import { MenuEntries } from "../../components/ContextMenu";
import { useAskCloseDuplicates } from "../../components/DuplicatePrompt";
import { Select } from "../../components/Select";
import { type Project } from "../../data/catalog";
import { useCommand } from "../../data/hooks";
import { type Link, type LinkKind, type RelationKind, type Task, type TaskRef } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import styles from "./TaskPanel.module.css";

/** A task's status as a coloured pill. */
function StatusPill({ task, project }: { task: TaskRef; project: Project | undefined }) {
  const hue = project?.statuses.find((status) => status.id === task.status.id)?.hue ?? "grey";
  return (
    <span className={`pill hue-${hue}`}>
      <span className="dot" />
      {task.status.name}
    </span>
  );
}

/** A small field that takes one line of text: Enter adds, Escape closes. */
function AddField({
  label,
  placeholder,
  onAdd,
  onClose,
  closeOnBlur = true,
}: {
  label: string;
  placeholder: string;
  onAdd: (text: string) => void;
  onClose: () => void;
  closeOnBlur?: boolean;
}) {
  const [text, setText] = useState("");
  return (
    <label className={classes("field", styles.addField)}>
      <input
        aria-label={label}
        placeholder={placeholder}
        autoFocus
        value={text}
        onChange={(event) => {
          setText(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter" && text.trim() !== "") {
            event.preventDefault();
            onAdd(text.trim());
            setText("");
          } else if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            onClose();
          }
        }}
        onBlur={() => {
          if (closeOnBlur && text.trim() === "") {
            onClose();
          }
        }}
      />
    </label>
  );
}

/** Where a subtask goes when it moves from `from` to `to` in `keys`: before or after the one it lands on. */
export function subtaskPlace(keys: readonly string[], from: number, to: number): { before: string } | { after: string } | null {
  const target = keys[to];
  if (from === to || target === undefined || keys[from] === undefined) {
    return null;
  }
  return to < from ? { before: target } : { after: target };
}

/** One subtask: a grip to drag it by, its checkbox, key, title and status; a menu to move it. */
function SubtaskRow({
  sub,
  project,
  readOnly,
  first,
  last,
  closing,
  reopening,
  onShift,
  onToggle,
  onOpenTask,
}: {
  sub: TaskRef;
  project: Project;
  readOnly: boolean;
  first: boolean;
  last: boolean;
  closing: boolean;
  reopening: boolean;
  onShift: (step: -1 | 1) => void;
  onToggle: () => void;
  onOpenTask: (key: string) => void;
}) {
  const t = useT();
  const { attributes, listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } = useSortable({ id: sub.key, disabled: readOnly });
  return (
    <div
      ref={setNodeRef}
      className={classes("item", styles.row, styles.subtaskRow, isDragging && styles.dragging)}
      style={{ transform: DndCss.Translate.toString(transform), transition }}
      data-subtask={sub.key}
      onKeyDown={(event) => {
        if (event.altKey && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
          event.preventDefault();
          onShift(event.key === "ArrowUp" ? -1 : 1);
        }
      }}
    >
      {/* Dragging is for the pointer; the keyboard moves a subtask with Alt+↑ and Alt+↓, or from its menu. */}
      <span ref={setActivatorNodeRef} className={classes(styles.grip, readOnly && styles.gripOff)} aria-hidden="true" data-subtask-grip={sub.key} {...attributes} {...listeners} tabIndex={-1} role="presentation">
        <GripVertical className="ic s" />
      </span>
      <input
        type="checkbox"
        className="check"
        checked={sub.closed}
        disabled={readOnly || (sub.closed ? !reopening : !closing)}
        aria-label={t(sub.closed ? "subtasks-reopen" : "subtasks-close", { key: sub.key })}
        onChange={onToggle}
      />
      <Button
        className={classes(styles.rowMain)}
        aria-label={t("subtasks-open", { key: sub.key, title: sub.title })}
        onPress={() => {
          onOpenTask(sub.key);
        }}
      >
        <span className="key">{sub.key}</span>
        <span className={classes("truncate grow", sub.closed && styles.closedTitle)}>{sub.title}</span>
        <StatusPill task={sub} project={project} />
      </Button>
      <MenuTrigger>
        <Button className="btn flat icon sm" isDisabled={readOnly} aria-label={t("subtasks-menu", { key: sub.key })}>
          <MoreHorizontal className="ic s" />
        </Button>
        <Popover className="pop menuPopover" placement="bottom end" offset={4}>
          <Menu className="menu" aria-label={t("subtasks-menu", { key: sub.key })}>
            <MenuEntries
              entries={[
                { id: "up", label: t("subtasks-move-up"), icon: <ArrowUp className="ic s" />, disabled: first, run: () => { onShift(-1); } },
                { id: "down", label: t("subtasks-move-down"), icon: <ArrowDown className="ic s" />, disabled: last, run: () => { onShift(1); } },
              ]}
            />
          </Menu>
        </Popover>
      </MenuTrigger>
    </div>
  );
}

/**
 * Subtasks in their own order, whatever their statuses: a checkbox that
 * closes or reopens each, its key, title and status; Add subtask. A subtask
 * moves by dragging its grip, from its menu, or with Alt+↑ and Alt+↓.
 */
export function SubtasksSection({
  task,
  project,
  readOnly,
  onOpenTask,
}: {
  task: Task;
  project: Project;
  readOnly: boolean;
  onOpenTask: (key: string) => void;
}) {
  const t = useT();
  const [adding, setAdding] = useState(false);
  const create = useCommand("task.create");
  const move = useCommand("task.move");
  const askDuplicates = useAskCloseDuplicates();
  const reorder = useCommand("task.reorder_subtask");
  const given = task.subtasks ?? [];
  // The order just asked for, shown until the engine's answer differs from the order it was made from.
  const [asked, setAsked] = useState<{ from: string; keys: string[] } | null>(null);
  const [focusKey, setFocusKey] = useState<string | null>(null);
  const givenKeys = given.map((sub) => sub.key);
  const showAsked = asked !== null && asked.from === givenKeys.join(" ");
  const subtasks = showAsked ? asked.keys.map((key) => given.find((sub) => sub.key === key)).filter((sub): sub is TaskRef => sub !== undefined) : given;
  const keys = subtasks.map((sub) => sub.key);
  const done = subtasks.filter((sub) => sub.closed).length;
  const closing = project.statuses.find((status) => status.category === "done");
  const reopening = project.statuses.find((status) => status.category === "todo") ?? project.statuses[0];
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  // A subtask moved from the keyboard keeps the focus where it lands.
  useEffect(() => {
    if (focusKey === null) {
      return;
    }
    const row = document.querySelector<HTMLElement>(`[data-subtask="${focusKey}"]`);
    if (row && !row.contains(document.activeElement)) {
      row.querySelector<HTMLElement>("button")?.focus();
    }
  });

  const place = (from: number, to: number) => {
    const sub = subtasks[from];
    const where = subtaskPlace(keys, from, to);
    if (!sub || !where || readOnly) {
      return;
    }
    setAsked({ from: givenKeys.join(" "), keys: arrayMove(keys, from, to) });
    reorder.mutate(
      { key: sub.key, ...where },
      {
        onError: () => {
          setAsked(null);
        },
      },
    );
  };
  const shift = (index: number, step: -1 | 1) => {
    const sub = subtasks[index];
    if (sub) {
      setFocusKey(sub.key);
      place(index, index + step);
    }
  };
  const onDragEnd = ({ active, over }: DragEndEvent) => {
    if (over) {
      place(keys.indexOf(String(active.id)), keys.indexOf(String(over.id)));
    }
  };

  return (
    <section className={styles.section} aria-label={t("subtasks-title")}>
      <div className={styles.sectionHead}>
        <span className="caption">{t("subtasks-title")}</span>
        {subtasks.length > 0 && (
          <>
            <div className={classes("meter grow", styles.sectionMeter)} role="img" aria-label={t("card-subtasks", { done, total: subtasks.length })}>
              <i style={{ width: `${String(Math.round((done / subtasks.length) * 100))}%` }} />
            </div>
            <span className={classes("num muted", styles.small)}>
              {done}/{subtasks.length}
            </span>
          </>
        )}
        {subtasks.length === 0 && <div className="grow" />}
        <Button
          className="btn flat icon sm"
          isDisabled={readOnly}
          aria-label={t("subtasks-add")}
          onPress={() => {
            setAdding(true);
          }}
        >
          <Plus className="ic s" />
        </Button>
      </div>
      <DndContext sensors={readOnly ? [] : sensors} collisionDetection={closestCenter} modifiers={[restrictToVertical]} onDragEnd={onDragEnd}>
        <SortableContext items={keys} strategy={verticalListSortingStrategy}>
          {subtasks.map((sub, index) => (
            <SubtaskRow
              key={sub.id}
              sub={sub}
              project={project}
              readOnly={readOnly}
              first={index === 0}
              last={index === subtasks.length - 1}
              closing={closing !== undefined}
              reopening={reopening !== undefined}
              onShift={(step) => {
                shift(index, step);
              }}
              onToggle={() => {
                const target = sub.closed ? reopening : closing;
                if (target) {
                  void askDuplicates(sub.key, !sub.closed).then((close) => {
                    move.mutate({ key: sub.key, status: target.id, ...(close === undefined ? {} : { close_duplicates: close }) });
                  });
                }
              }}
              onOpenTask={onOpenTask}
            />
          ))}
        </SortableContext>
      </DndContext>
      {adding && (
        <AddField
          label={t("subtasks-new")}
          placeholder={t("subtasks-new-placeholder")}
          onAdd={(title) => {
            create.mutate({ project: task.project, title, parent: task.key });
          }}
          onClose={() => {
            setAdding(false);
          }}
        />
      )}
    </section>
  );
}

/** Drags in a list move up and down only. */
const restrictToVertical: Modifier = ({ transform }) => ({ ...transform, x: 0 });

/** The kinds of link the person adds, as they read from this task. */
const ADDABLE: readonly RelationKind[] = ["blocks", "blocked_by", "relates", "duplicates", "duplicated_by", "subtask_of", "parent_of"];

/**
 * The lag of a `blocks` link: the days the blocked task waits after its
 * blocker ends. A button showing it opens a small form to change it.
 */
function LagButton({ task, link, readOnly }: { task: Task; link: Link; readOnly: boolean }) {
  const t = useT();
  const relate = useCommand("relation.add", { undoable: t("links-lag-changed") });
  const lag = link.lag_days ?? 0;
  const [days, setDays] = useState(lag);
  return (
    <DialogTrigger
      onOpenChange={(open) => {
        if (open) {
          setDays(lag);
        }
      }}
    >
      <Button className={classes("btn flat sm", lag === 0 && "faint")} isDisabled={readOnly} aria-label={t("links-lag-edit", { key: link.task.key, days: lag })} data-lag-button={link.task.key}>
        {lag === 0 ? t("links-lag-none") : t("links-lag", { days: lag })}
      </Button>
      <Popover className="pop" placement="bottom end" offset={4}>
        <Dialog className={classes(styles.lagDialog)} aria-label={t("links-lag-edit", { key: link.task.key, days: lag })}>
          {({ close }) => (
            <form
              className={styles.lagForm}
              onSubmit={(event) => {
                event.preventDefault();
                close();
                if (days !== lag) {
                  relate.mutate({ key: task.key, kind: link.kind === "blocked_by" ? "blocked_by" : "blocks", other: link.task.key, lag_days: days });
                }
              }}
            >
              <NumberField
                minValue={0}
                maxValue={365}
                step={1}
                value={days}
                onChange={(value) => {
                  setDays(Number.isFinite(value) ? Math.round(value) : 0);
                }}
                autoFocus
              >
                <Label className="caption">{t("links-lag-label")}</Label>
                <Group className="field">
                  <Input className="num" />
                </Group>
              </NumberField>
              <Button type="submit" className="btn primary sm">
                {t("panel-save")}
              </Button>
            </form>
          )}
        </Dialog>
      </Popover>
    </DialogTrigger>
  );
}

/** Links to other tasks: how each reads from this task, its key, title and status; add and remove. */
export function LinksSection({
  task,
  project,
  readOnly,
  onOpenTask,
}: {
  task: Task;
  project: Project;
  readOnly: boolean;
  onOpenTask: (key: string) => void;
}) {
  const t = useT();
  const [adding, setAdding] = useState(false);
  const [kind, setKind] = useState<RelationKind>("blocks");
  const add = useCommand("relation.add");
  const remove = useCommand("relation.remove", { undoable: t("links-removed") });
  const links: readonly Link[] = task.links ?? [];
  const label = (linkKind: LinkKind | RelationKind) => t(`relation-${linkKind}`);
  return (
    <section className={styles.section} aria-label={t("links-title")}>
      <div className={styles.sectionHead}>
        <span className="caption grow">{t("links-title")}</span>
        <Button
          className="btn flat icon sm"
          isDisabled={readOnly}
          aria-label={t("links-add")}
          onPress={() => {
            setAdding(true);
          }}
        >
          <Plus className="ic s" />
        </Button>
      </div>
      {links.length === 0 && !adding && <p className={classes("faint", styles.small, styles.emptyNote)}>{t("links-none")}</p>}
      {links.map((link) => (
        <div key={`${link.id}-${link.kind}`} className={classes("item", styles.row)} data-link={`${link.kind} ${link.task.key}`}>
          <Button
            className={classes(styles.rowMain)}
            aria-label={t("links-open", { kind: label(link.kind), key: link.task.key, title: link.task.title })}
            onPress={() => {
              onOpenTask(link.task.key);
            }}
          >
            <span className={classes("muted", styles.linkKind)}>{label(link.kind)}</span>
            <span className="key">{link.task.key}</span>
            <span className={classes("truncate grow", link.task.closed && styles.closedTitle)}>{link.task.title}</span>
            <StatusPill task={link.task} project={link.task.key.startsWith(`${project.key}-`) ? project : undefined} />
          </Button>
          {(link.kind === "blocks" || link.kind === "blocked_by") && <LagButton task={task} link={link} readOnly={readOnly} />}
          <Button
            className="btn flat icon sm"
            isDisabled={readOnly}
            aria-label={t("links-remove", { kind: label(link.kind), key: link.task.key })}
            onPress={() => {
              remove.mutate({ key: task.key, kind: link.kind, other: link.task.key });
            }}
          >
            <X className="ic s" />
          </Button>
        </div>
      ))}
      {adding && (
        <div className={styles.addLink}>
          <Select
            className={styles.select}
            label={t("links-kind")}
            value={kind}
            onChange={setKind}
            options={ADDABLE.map((option) => ({ id: option, label: label(option) }))}
          />
          <AddField
            label={t("links-other")}
            placeholder={t("links-other-placeholder")}
            onAdd={(other) => {
              add.mutate({ key: task.key, kind, other: other.toUpperCase() }, { onSuccess: () => { setAdding(false); } });
            }}
            onClose={() => {
              setAdding(false);
            }}
            closeOnBlur={false}
          />
        </div>
      )}
    </section>
  );
}
