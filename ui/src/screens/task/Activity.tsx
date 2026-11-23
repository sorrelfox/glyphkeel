// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { History as HistoryIcon, MoreHorizontal, Paperclip, Pencil, Trash2, Undo2 } from "lucide-react";
import { useState } from "react";
import { Button, Menu, MenuTrigger, Popover, Tab, TabList, TabPanel, Tabs, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { MenuEntries } from "../../components/ContextMenu";
import { useToasts } from "../../components/Toasts";
import { type Project } from "../../data/catalog";
import { errorToast, useCall, useCommand } from "../../data/hooks";
import { revertible, useComments, useTaskHistory } from "../../data/task";
import { type ActivityEntry, type Actor, type Comment, type Task, type Via } from "../../generated/ops";
import { useT, type Translate } from "../../i18n/I18n";
import { formatMinutes } from "../board/model";
import { FileRow, useAttach } from "./Files";
import styles from "./TaskPanel.module.css";

/** A moment as the panel shows it: "Nov 19, 2026, 16:20". */
export function useMomentFormat(): (at: string) => string {
  const { locale } = useLocale();
  const format = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" });
  return (at) => format.format(new Date(at));
}

/** Who did something: "Me", or the agent's name with where it came through. */
function Author({ actor, via }: { actor: Actor; via: Via }) {
  const t = useT();
  if (actor.kind === "user") {
    return <span className={styles.author}>{t("comment-me")}</span>;
  }
  return (
    <>
      <span className={styles.author}>{actor.name}</span>
      <span className="chip hue-purple">{t("comment-agent", { via: t(`via-${via}`) })}</span>
    </>
  );
}

/** A text field for a comment: Ctrl+Enter sends, Escape gives up. */
function CommentField({
  label,
  value,
  onChange,
  onSubmit,
  onCancel,
  rows,
}: {
  label: string;
  value: string;
  onChange: (text: string) => void;
  onSubmit: () => void;
  onCancel?: () => void;
  rows: number;
}) {
  const t = useT();
  return (
    <textarea
      className={styles.commentField}
      aria-label={label}
      placeholder={t("composer-placeholder")}
      rows={rows}
      value={value}
      onChange={(event) => {
        onChange(event.target.value);
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
          event.preventDefault();
          onSubmit();
        } else if (event.key === "Escape" && onCancel) {
          event.preventDefault();
          event.stopPropagation();
          onCancel();
        }
      }}
    />
  );
}

function CommentView({ comment, readOnly }: { comment: Comment; readOnly: boolean }) {
  const t = useT();
  const moment = useMomentFormat();
  const [editing, setEditing] = useState<string | null>(null);
  const [earlier, setEarlier] = useState(false);
  const update = useCommand("comment.update");
  const remove = useCommand("comment.delete", { undoable: t("comment-trashed") });
  const save = () => {
    if (editing === null || editing.trim() === "" || editing === comment.body) {
      setEditing(null);
      return;
    }
    update.mutate({ comment: comment.id, body: editing, expected_version: comment.version }, { onSuccess: () => { setEditing(null); } });
  };
  return (
    <article className={styles.comment} data-comment={comment.id} aria-label={t("comment-label", { author: comment.author.kind === "user" ? t("comment-me") : comment.author.name, time: moment(comment.created_at) })}>
      <div className={styles.commentHead}>
        <Author actor={comment.author} via={comment.via} />
        <span className="faint">{moment(comment.created_at)}</span>
        {comment.edited_at !== undefined && (
          <Button
            className={classes("linkbtn", styles.edited)}
            aria-expanded={earlier}
            isDisabled={comment.revision_count === 0}
            onPress={() => {
              setEarlier((shown) => !shown);
            }}
          >
            {t("comment-edited", { count: comment.revision_count })}
          </Button>
        )}
        <div className="grow" />
        <MenuTrigger>
          <Button className="btn flat icon sm" isDisabled={readOnly} aria-label={t("comment-menu")}>
            <MoreHorizontal className="ic s" />
          </Button>
          <Popover className="pop menuPopover" placement="bottom end" offset={4}>
            <Menu className="menu" aria-label={t("comment-menu")}>
              <MenuEntries
                entries={[
                  { id: "edit", label: t("comment-edit"), icon: <Pencil className="ic s" />, run: () => { setEditing(comment.body); } },
                  { id: "delete", label: t("comment-delete"), icon: <Trash2 className="ic s" />, run: () => { remove.mutate({ comment: comment.id }); } },
                ]}
              />
            </Menu>
          </Popover>
        </MenuTrigger>
      </div>
      {editing === null ? (
        <div className={classes("md selectable", styles.commentBody)}>{comment.body}</div>
      ) : (
        <div className={styles.commentEdit}>
          <CommentField label={t("comment-edit-field")} value={editing} rows={4} onChange={setEditing} onSubmit={save} onCancel={() => { setEditing(null); }} />
          <div className={styles.sectionHead}>
            <div className="grow" />
            <Button className="btn flat sm" onPress={() => { setEditing(null); }}>
              {t("dialog-cancel")}
            </Button>
            <Button className="btn primary sm" onPress={save}>
              {t("panel-save")}
            </Button>
          </div>
        </div>
      )}
      {comment.files?.map((file) => <FileRow key={file.id} file={file} readOnly={readOnly} />)}
      {earlier && (
        <ol className={styles.revisions} aria-label={t("comment-earlier")}>
          {(comment.revisions ?? []).map((revision) => (
            <li key={revision.replaced_at} className={styles.revision}>
              <span className={classes("faint", styles.small)}>{t("comment-replaced", { time: moment(revision.replaced_at), actor: revision.replaced_by.kind === "user" ? t("comment-me") : revision.replaced_by.name })}</span>
              <div className={classes("md selectable", styles.commentBody)}>{revision.body}</div>
            </li>
          ))}
        </ol>
      )}
    </article>
  );
}

function CommentsList({ taskKey, readOnly }: { taskKey: string; readOnly: boolean }) {
  const t = useT();
  const comments = useComments(taskKey);
  if (!comments.data) {
    return null;
  }
  if (comments.data.items.length === 0) {
    return <p className={classes("faint", styles.small)}>{t("comments-none")}</p>;
  }
  return (
    <>
      {comments.data.items.map((comment) => (
        <CommentView key={comment.id} comment={comment} readOnly={readOnly} />
      ))}
    </>
  );
}

// ---------- history ----------

const NAMED_FIELDS = ["title", "description", "status", "type", "priority", "labels", "parent", "start_date", "due_date", "estimate", "progress", "rank", "project"];

interface Names {
  task: Task;
  project: Project;
  /** Task keys by task ID, as far as the panel knows them. */
  keys: ReadonlyMap<string, string>;
}

const BACKWARDS: Record<string, string> = { blocks: "blocked_by", duplicates: "duplicated_by", clones: "cloned_by", relates: "relates" };

/** Any recorded value as text. */
function plainText(value: unknown): string {
  return typeof value === "string" || typeof value === "number" || typeof value === "boolean" ? String(value) : JSON.stringify(value);
}

/** A field's value as people read it. */
function shown(t: Translate, names: Names, field: string, value: unknown): string {
  if (value === undefined || value === null || value === "") {
    return t("prop-none");
  }
  const byId = (list: readonly { id: string; name: string }[], id: unknown) => list.find((item) => item.id === id)?.name ?? plainText(id);
  switch (field) {
    case "status":
      return byId(names.project.statuses, value);
    case "type":
      return byId(names.project.types, value);
    case "labels":
      return Array.isArray(value) ? value.map((id) => byId(names.project.labels, id)).join(", ") : plainText(value);
    case "priority":
      return typeof value === "string" && ["none", "low", "normal", "high", "urgent"].includes(value) ? t(`priority-${value}`) : plainText(value);
    case "estimate":
      return typeof value === "number" ? formatMinutes(value) : plainText(value);
    case "parent":
      return names.keys.get(plainText(value)) ?? plainText(value);
    default:
      return plainText(value);
  }
}

/** The lines one history entry reads as: "Status: To do → In progress". */
export function describeEntry(t: Translate, names: Names, entry: ActivityEntry): string[] {
  const fields = entry.fields;
  const has = (name: string) => name in fields;
  switch (entry.entity) {
    case "comment":
      if (has("created")) {
        return [t("history-comment-added")];
      }
      if (has("trashed")) {
        return [t(fields.trashed?.after === true ? "history-comment-trashed" : "history-comment-restored")];
      }
      return [t("history-comment-edited")];
    case "attachment": {
      const name = typeof fields.name?.after === "string" ? fields.name.after : "";
      if (has("created")) {
        return [t("history-file-attached", { name })];
      }
      return [t(fields.trashed?.after === true ? "history-file-trashed" : "history-file-restored")];
    }
    case "relation": {
      const added = fields.link?.after !== undefined;
      const link = (fields.link?.after ?? fields.link?.before ?? {}) as { from?: string; to?: string; kind?: string };
      const forward = link.from === names.task.id;
      const kind = forward ? (link.kind ?? "relates") : (BACKWARDS[link.kind ?? "relates"] ?? "relates");
      const other = forward ? link.to : link.from;
      const key = (other !== undefined && names.keys.get(other)) || t("history-another-task");
      return [t(added ? "history-link-added" : "history-link-removed", { kind: t(`relation-${kind}`), key })];
    }
    default:
      break;
  }
  if (has("created")) {
    return [t("history-created", { key: shown(t, names, "key", fields.key?.after ?? names.task.key) })];
  }
  const lines: string[] = [];
  for (const [field, change] of Object.entries(fields)) {
    if (field === "claim") {
      const claim = change.after as { agent?: string } | undefined;
      lines.push(claim?.agent === undefined ? t("history-released") : t("history-claimed", { agent: claim.agent }));
    } else if (field === "description") {
      lines.push(t("history-description"));
    } else if (field === "rank") {
      lines.push(t("history-moved"));
    } else {
      const name = NAMED_FIELDS.includes(field) ? t(`history-field-${field}`) : field;
      lines.push(
        change.after === undefined
          ? t("history-cleared", { field: name })
          : change.before === undefined
            ? t("history-set", { field: name, value: shown(t, names, field, change.after) })
            : t("history-changed", { field: name, before: shown(t, names, field, change.before), after: shown(t, names, field, change.after) }),
      );
    }
  }
  return lines;
}

function HistoryList({ task, project, readOnly }: { task: Task; project: Project; readOnly: boolean }) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const moment = useMomentFormat();
  const files = (task.files ?? []).map((file) => file.id);
  const history = useTaskHistory(task, files, true);
  const keys = new Map<string, string>([[task.id, task.key]]);
  for (const link of task.links ?? []) {
    keys.set(link.task.id, link.task.key);
  }
  for (const sub of task.subtasks ?? []) {
    keys.set(sub.id, sub.key);
  }
  const revert = (group: string) => {
    call("history.revert", { groups: [group] }).then(
      (result) => {
        const skipped = result.skipped[0];
        toasts.show(skipped ? { tone: "error", text: t("history-not-reverted"), detail: skipped.reason } : { tone: "info", text: t("history-reverted-toast") });
      },
      (error: unknown) => {
        toasts.show(errorToast(t, error));
      },
    );
  };
  if (history.isError) {
    return <p className="faint">{history.error.error.message}</p>;
  }
  if (!history.data) {
    return null;
  }
  if (history.data.length === 0) {
    return <p className={classes("faint", styles.small)}>{t("history-none")}</p>;
  }
  const seen = new Set<string>();
  return (
    <ol className={styles.history} aria-label={t("history-title")}>
      {history.data.map((entry) => {
        const first = !seen.has(entry.group);
        seen.add(entry.group);
        return (
          <li key={`${String(entry.seq)}-${entry.id}`} className={classes(styles.historyEntry, entry.reverted_by !== undefined && styles.reverted)} data-history={entry.op}>
            <div className={styles.historyText}>
              {describeEntry(t, { task, project, keys }, entry).map((line) => (
                <span key={line}>{line}</span>
              ))}
              <span className={classes("faint", styles.small)}>
                {t("history-by", { actor: entry.actor.kind === "user" ? t("comment-me") : entry.actor.name, via: t(`via-${entry.via}`), time: moment(entry.at) })}
                {entry.reverted_by !== undefined && ` · ${t("history-was-reverted")}`}
              </span>
            </div>
            {first && revertible(entry) && !readOnly && (
              <Button className="btn flat icon sm" aria-label={t("history-revert", { time: moment(entry.at) })} onPress={() => { revert(entry.group); }}>
                <Undo2 className="ic s" />
              </Button>
            )}
          </li>
        );
      })}
    </ol>
  );
}

/** The tabs under the task's sections: its comments and its history. */
export function ActivityTabs({ task, project, readOnly }: { task: Task; project: Project; readOnly: boolean }) {
  const t = useT();
  return (
    <Tabs className={classes(styles.tabs)} defaultSelectedKey="comments">
      <TabList className="tabs" aria-label={t("activity-tabs")}>
        <Tab id="comments" className={({ isSelected }) => classes("tab", isSelected && "on")}>
          {t("comments-title")}
          {task.comment_count > 0 && <span className="count">{task.comment_count}</span>}
        </Tab>
        <Tab id="history" className={({ isSelected }) => classes("tab", isSelected && "on")}>
          <HistoryIcon className="ic s" />
          {t("history-title")}
        </Tab>
      </TabList>
      <TabPanel id="comments" className={classes(styles.tabPanel)}>
        <CommentsList taskKey={task.key} readOnly={readOnly} />
      </TabPanel>
      <TabPanel id="history" className={classes(styles.tabPanel)}>
        <HistoryList task={task} project={project} readOnly={readOnly} />
      </TabPanel>
    </Tabs>
  );
}

/** The comment composer, pinned under the panel: Ctrl+Enter sends. */
export function Composer({ taskKey, readOnly }: { taskKey: string; readOnly: boolean }) {
  const t = useT();
  const [text, setText] = useState("");
  const [requestId, setRequestId] = useState(() => crypto.randomUUID());
  const add = useCommand("comment.add");
  const { attach, field, busy } = useAttach(taskKey);
  const send = () => {
    if (text.trim() === "" || add.isPending) {
      return;
    }
    add.mutate(
      { key: taskKey, body: text, client_request_id: requestId },
      {
        onSuccess: () => {
          setText("");
          setRequestId(crypto.randomUUID());
        },
      },
    );
  };
  return (
    <div className={styles.composer}>
      <CommentField label={t("composer-label")} value={text} rows={text.includes("\n") ? 4 : 1} onChange={setText} onSubmit={send} />
      <Button className="btn flat icon" isDisabled={readOnly || busy} aria-label={t("files-attach")} onPress={attach}>
        <Paperclip className="ic" />
      </Button>
      {field}
      <Button className="btn" isDisabled={readOnly || text.trim() === "" || add.isPending} onPress={send}>
        {t("composer-send")}
      </Button>
    </div>
  );
}
