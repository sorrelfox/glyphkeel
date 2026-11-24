// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { History as HistoryIcon, MoreHorizontal, Paperclip, Pencil, Trash2, Undo2 } from "lucide-react";
import { useState } from "react";
import { Button, Menu, MenuTrigger, Popover, Tab, TabList, TabPanel, Tabs, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { MenuEntries } from "../../components/ContextMenu";
import { skippedReason } from "../../data/safety";
import { useToasts } from "../../components/Toasts";
import { type Project } from "../../data/catalog";
import { errorToast, useCall, useCommand } from "../../data/hooks";
import { readDraft, writeDraft } from "../../data/drafts";
import { toggleChecklist, useRendered } from "../../data/markdown";
import { revertible, useComments, useTaskHistory } from "../../data/task";
import { type ActivityEntry, type Actor, type Comment, type Rendered, type Task, type Via } from "../../generated/ops";
import { useT, type Translate } from "../../i18n/I18n";
import { MarkdownReader } from "../../markdown/Reader";
import { formatDuration, type DurationUnits } from "../../data/duration";
import { useDurationUnits } from "../../data/time";
import { useTimeZone } from "../../data/zone";
import { FileRow, useAttach } from "./Files";
import { MarkdownField } from "./MarkdownField";
import { TimeEntries } from "./Time";
import styles from "./TaskPanel.module.css";

/** A moment as the panel shows it: "Nov 19, 2026, 16:20". */
export function useMomentFormat(): (at: string) => string {
  const { locale } = useLocale();
  const zone = useTimeZone();
  const format = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short", ...(zone === undefined ? {} : { timeZone: zone }) });
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

function CommentView({ comment, rendered, readOnly, onOpenTask }: { comment: Comment; rendered: Rendered | undefined; readOnly: boolean; onOpenTask: (key: string) => void }) {
  const t = useT();
  const call = useCall();
  const moment = useMomentFormat();
  // The text being edited, with the version it was opened at: a save names
  // that version, so a change made meanwhile elsewhere refuses it instead of
  // being overwritten.
  const [editing, setEditingState] = useState<{ text: string; version: number } | null>(null);
  const setEditing = (text: string | null) => {
    setEditingState((was) => (text === null ? null : { text, version: was?.version ?? comment.version }));
  };
  const [earlier, setEarlier] = useState(false);
  const update = useCommand("comment.update");
  const remove = useCommand("comment.delete", { undoable: t("comment-trashed") });
  const save = () => {
    if (editing === null || editing.text.trim() === "" || editing.text === comment.body) {
      setEditing(null);
      return;
    }
    update.mutate({ comment: comment.id, body: editing.text, expected_version: editing.version }, { onSuccess: () => { setEditing(null); } });
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
        rendered === undefined ? (
          <div className={classes("selectable", styles.commentBody, styles.plainBody)}>{comment.body}</div>
        ) : (
          <MarkdownReader
            compact
            html={rendered.html}
            label={t("comment-body")}
            onOpenTask={onOpenTask}
            onToggle={readOnly ? undefined : (click) => toggleChecklist(call, { comment: comment.id }, rendered.checklist, click)}
          />
        )
      ) : (
        <div className={styles.commentEdit}>
          <MarkdownField label={t("comment-edit-field")} value={editing.text} autoFocus attachTo={comment.task} onChange={setEditing} onSubmit={save} onCancel={() => { setEditing(null); }} />
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
      {earlier && <EarlierVersions comment={comment} onOpenTask={onOpenTask} />}
    </article>
  );
}

/** A comment's earlier texts, newest first, rendered like the comment. */
function EarlierVersions({ comment, onOpenTask }: { comment: Comment; onOpenTask: (key: string) => void }) {
  const t = useT();
  const moment = useMomentFormat();
  const revisions = comment.revisions ?? [];
  const rendered = useRendered(revisions.map((revision, index) => ({ source: revision.body, document_id: `${comment.id}-v${String(revisions.length - index)}` })));
  return (
    <ol className={styles.revisions} aria-label={t("comment-earlier")}>
      {revisions.map((revision, index) => {
        const html = rendered.items[index]?.html;
        return (
          <li key={revision.replaced_at} className={styles.revision}>
            <span className={classes("faint", styles.small)}>{t("comment-replaced", { time: moment(revision.replaced_at), actor: revision.replaced_by.kind === "user" ? t("comment-me") : revision.replaced_by.name })}</span>
            {html === undefined ? (
              <div className={classes("selectable", styles.commentBody, styles.plainBody)}>{revision.body}</div>
            ) : (
              <MarkdownReader compact html={html} onOpenTask={onOpenTask} />
            )}
          </li>
        );
      })}
    </ol>
  );
}

function CommentsList({ taskKey, readOnly, onOpenTask }: { taskKey: string; readOnly: boolean; onOpenTask: (key: string) => void }) {
  const t = useT();
  const comments = useComments(taskKey);
  const rendered = useRendered((comments.data?.items ?? []).map((comment) => ({ comment: comment.id })));
  if (!comments.data) {
    return null;
  }
  if (comments.data.items.length === 0) {
    return <p className={classes("faint", styles.small)}>{t("comments-none")}</p>;
  }
  return (
    <>
      {comments.data.items.map((comment, index) => {
        const html = rendered.items[index];
        // A rendering of an older text of this comment isn't shown.
        const current = html !== undefined && (html.version === undefined || html.version === comment.version) ? html : undefined;
        return <CommentView key={comment.id} comment={comment} rendered={current} readOnly={readOnly} onOpenTask={onOpenTask} />;
      })}
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
  /** The workspace's working day and week, for durations. */
  units?: DurationUnits;
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
      return typeof value === "number" ? formatDuration(value, names.units) : plainText(value);
    case "parent":
      return names.keys.get(plainText(value)) ?? plainText(value);
    default:
      return plainText(value);
  }
}

/** The description revisions an entry moved between, when it changed the description. */
export function descriptionRevisions(entry: ActivityEntry): [string, string] | null {
  const change = entry.fields.description;
  const revision = (value: unknown) => (typeof value === "object" && value !== null && "revision" in value && typeof value.revision === "string" ? value.revision : null);
  const before = revision(change?.before);
  const after = revision(change?.after);
  return before !== null && after !== null ? [before, after] : null;
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
    case "time_entry": {
      const minutes = typeof fields.minutes?.after === "number" ? fields.minutes.after : typeof fields.minutes?.before === "number" ? fields.minutes.before : null;
      const time = minutes === null ? "" : formatDuration(minutes, names.units);
      if (has("created")) {
        return [t("history-time-logged", { time })];
      }
      if (has("trashed")) {
        return [t(fields.trashed?.after === true ? "history-time-deleted" : "history-time-restored")];
      }
      return [t("history-time-edited")];
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
  if (has("trashed")) {
    return [t(fields.trashed?.after === true ? "history-task-trashed" : "history-task-restored")];
  }
  const lines: string[] = [];
  for (const [field, change] of Object.entries(fields)) {
    if (field === "claim") {
      const claim = change.after as { agent?: string } | undefined;
      lines.push(claim?.agent === undefined ? t("history-released") : t("history-claimed", { agent: claim.agent }));
    } else if (field === "description") {
      lines.push(t("history-description"));
    } else if (field === "reminder") {
      lines.push(t(change.after === undefined || change.after === null ? "history-reminder-cleared" : "history-reminder-set"));
    } else if (field === "rank") {
      lines.push(t("history-moved"));
    } else if (field === "subtask_rank") {
      // Set and cleared with the parent, which has its own line; only a move among the parent's subtasks is told.
      if (change.before !== undefined && change.after !== undefined && !has("parent")) {
        lines.push(t("history-subtask-moved"));
      }
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

function HistoryList({
  task,
  project,
  readOnly,
  onShowDescriptionDiff,
}: {
  task: Task;
  project: Project;
  readOnly: boolean;
  onShowDescriptionDiff: (older: string, newer: string) => void;
}) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const moment = useMomentFormat();
  const history = useTaskHistory(task.id, true);
  const units = useDurationUnits();
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
        toasts.show(skipped ? { tone: "error", text: t("history-not-reverted"), detail: skippedReason(t, skipped) } : { tone: "info", text: t("history-reverted-toast") });
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
  const entries = history.data.pages.flatMap((page) => page.items);
  if (entries.length === 0) {
    return <p className={classes("faint", styles.small)}>{t("history-none")}</p>;
  }
  const seen = new Set<string>();
  return (
    <>
    <ol className={styles.history} aria-label={t("history-title")}>
      {entries.map((entry) => {
        const lines = describeEntry(t, { task, project, keys, units }, entry);
        if (lines.length === 0) {
          return null;
        }
        const first = !seen.has(entry.group);
        seen.add(entry.group);
        const revisions = entry.entity === "task" ? descriptionRevisions(entry) : null;
        return (
          <li key={`${String(entry.seq)}-${entry.id}`} className={classes(styles.historyEntry, entry.reverted_by !== undefined && styles.reverted)} data-history={entry.op}>
            <div className={styles.historyText}>
              {lines.map((line) =>
                revisions !== null && line === t("history-description") ? (
                  <Button
                    key={line}
                    className={classes("linkbtn", styles.diffLink)}
                    onPress={() => {
                      onShowDescriptionDiff(revisions[0], revisions[1]);
                    }}
                  >
                    {t("history-description-diff")}
                  </Button>
                ) : (
                  <span key={line}>{line}</span>
                ),
              )}
              <span className={classes("faint", styles.small)}>
                {t(entry.reverted_by === undefined ? "history-by" : "history-by-reverted", { actor: entry.actor.kind === "user" ? t("comment-me") : entry.actor.name, via: t(`via-${entry.via}`), time: moment(entry.at) })}
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
    {history.hasNextPage && (
      <Button className={classes("btn flat sm", styles.olderHistory)} isDisabled={history.isFetchingNextPage} onPress={() => void history.fetchNextPage()}>
        {t("history-older")}
      </Button>
    )}
    </>
  );
}

/** The tabs under the task's sections: its comments, its time and its history. */
export function ActivityTabs({
  task,
  project,
  readOnly,
  onOpenTask,
  onShowDescriptionDiff,
}: {
  task: Task;
  project: Project;
  readOnly: boolean;
  onOpenTask: (key: string) => void;
  onShowDescriptionDiff: (older: string, newer: string) => void;
}) {
  const t = useT();
  return (
    <Tabs className={classes(styles.tabs)} defaultSelectedKey="comments">
      <TabList className="tabs" aria-label={t("activity-tabs")}>
        <Tab id="comments" className={({ isSelected }) => classes("tab", isSelected && "on")}>
          {t("comments-title")}
          {task.comment_count > 0 && <span className="count">{task.comment_count}</span>}
        </Tab>
        <Tab id="time" className={({ isSelected }) => classes("tab", isSelected && "on")}>
          {t("time-title")}
          {(task.time_entries?.length ?? 0) > 0 && <span className="count">{task.time_entries?.length}</span>}
        </Tab>
        <Tab id="history" className={({ isSelected }) => classes("tab", isSelected && "on")}>
          <HistoryIcon className="ic s" />
          {t("history-title")}
        </Tab>
      </TabList>
      <TabPanel id="comments" className={classes(styles.tabPanel)}>
        <CommentsList taskKey={task.key} readOnly={readOnly} onOpenTask={onOpenTask} />
      </TabPanel>
      <TabPanel id="time" className={classes(styles.tabPanel)}>
        <TimeEntries task={task} readOnly={readOnly} />
      </TabPanel>
      <TabPanel id="history" className={classes(styles.tabPanel)}>
        <HistoryList task={task} project={project} readOnly={readOnly} onShowDescriptionDiff={onShowDescriptionDiff} />
      </TabPanel>
    </Tabs>
  );
}

/** The comment composer, pinned under the panel, with the Markdown editor: Ctrl+Enter sends. */
export function Composer({ taskKey, readOnly }: { taskKey: string; readOnly: boolean }) {
  const t = useT();
  const draft = `composer:${taskKey}`;
  const [text, setTextState] = useState(() => readDraft(draft)?.text ?? "");
  const setText = (next: string) => {
    setTextState(next);
    writeDraft(draft, next === "" ? null : { text: next });
  };
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
      <MarkdownField compact label={t("composer-label")} value={text} readOnly={readOnly} attachTo={taskKey} onChange={setText} onSubmit={send} />
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
