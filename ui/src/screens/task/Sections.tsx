// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ArrowDown, ArrowUp, MoreHorizontal, Plus, X } from "lucide-react";
import { useState } from "react";
import { Button, Menu, MenuTrigger, Popover } from "react-aria-components";

import { classes } from "../../classes";
import { MenuEntries } from "../../components/ContextMenu";
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

/**
 * Subtasks, in board order: a checkbox that closes or reopens each, its key,
 * title and status; Add subtask. A subtask moves up or down among those of
 * the same status, from its menu or with Alt+↑ and Alt+↓.
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
  const subtasks = task.subtasks ?? [];
  const done = subtasks.filter((sub) => sub.closed).length;
  const closing = project.statuses.find((status) => status.category === "done");
  const reopening = project.statuses.find((status) => status.category === "todo") ?? project.statuses[0];

  /** The neighbour a subtask can move past: the one before or after it, in the same status. */
  const neighbour = (index: number, step: -1 | 1) => {
    const other = subtasks[index + step];
    const sub = subtasks[index];
    return other && sub && other.status.id === sub.status.id ? other : undefined;
  };
  const shift = (index: number, step: -1 | 1) => {
    const sub = subtasks[index];
    const other = neighbour(index, step);
    if (sub && other) {
      move.mutate({ key: sub.key, ...(step < 0 ? { before: other.key } : { after: other.key }) });
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
      {subtasks.map((sub, index) => (
        <div
          key={sub.id}
          className={classes("item", styles.row)}
          data-subtask={sub.key}
          onKeyDown={(event) => {
            if (event.altKey && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
              event.preventDefault();
              shift(index, event.key === "ArrowUp" ? -1 : 1);
            }
          }}
        >
          <input
            type="checkbox"
            className="check"
            checked={sub.closed}
            disabled={readOnly || (sub.closed ? !reopening : !closing)}
            aria-label={t(sub.closed ? "subtasks-reopen" : "subtasks-close", { key: sub.key })}
            onChange={() => {
              const target = sub.closed ? reopening : closing;
              if (target) {
                move.mutate({ key: sub.key, status: target.id });
              }
            }}
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
                    { id: "up", label: t("subtasks-move-up"), icon: <ArrowUp className="ic s" />, disabled: !neighbour(index, -1), run: () => { shift(index, -1); } },
                    { id: "down", label: t("subtasks-move-down"), icon: <ArrowDown className="ic s" />, disabled: !neighbour(index, 1), run: () => { shift(index, 1); } },
                  ]}
                />
              </Menu>
            </Popover>
          </MenuTrigger>
        </div>
      ))}
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

/** The kinds of link the person adds, as they read from this task. */
const ADDABLE: readonly RelationKind[] = ["blocks", "blocked_by", "relates", "duplicates", "duplicated_by", "subtask_of", "parent_of"];

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
          <select
            className={styles.select}
            aria-label={t("links-kind")}
            value={kind}
            onChange={(event) => {
              setKind(event.target.value as RelationKind);
            }}
          >
            {ADDABLE.map((option) => (
              <option key={option} value={option}>
                {label(option)}
              </option>
            ))}
          </select>
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
