// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { GitCompare, History as HistoryIcon, X } from "lucide-react";
import { useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { useToasts } from "../../components/Toasts";
import { useUpdateTask } from "../../data/board";
import { useDraft, useTaskDrafts, type Draft } from "../../data/drafts";
import { useCall, useCommand, useOp } from "../../data/hooks";
import { toggleChecklist, useRenderedDescription } from "../../data/markdown";
import { type ConflictAction, type RevisionInfo, type Task } from "../../generated/ops";
import { useT, type Translate } from "../../i18n/I18n";
import { MarkdownReader } from "../../markdown/Reader";
import { useMomentFormat } from "./Activity";
import { DiffView } from "./DiffView";
import { MarkdownField } from "./MarkdownField";
import styles from "./TaskPanel.module.css";

/** What the description section shows: the text, its editor, its history, or a conflict copy to resolve. */
export type DescriptionView =
  | { kind: "read" }
  | { kind: "edit" }
  | { kind: "history"; compare?: readonly [string, string] | undefined }
  | { kind: "resolve" };

interface DescriptionProps {
  task: Task;
  readOnly: boolean;
  view: DescriptionView;
  onView: (view: DescriptionView) => void;
  onOpenTask: (key: string) => void;
}

/**
 * The description: rendered by the engine, with checkboxes that tick
 * through it. Edit (or `E`) swaps in the Markdown editor; a save names the
 * revision it was edited from, so an edit made meanwhile elsewhere is
 * merged, or kept as a conflict copy to resolve here. History lists the
 * revisions and shows the differences between any two.
 */
export function DescriptionSection({ task, readOnly, view, onView, onOpenTask }: DescriptionProps) {
  const t = useT();
  // Unsaved text kept from earlier, in this window or, after a restart, by the engine.
  const [draft, setDraft] = useDraft({ kind: "task", id: task.id }, useTaskDrafts(task.key));
  const editing = view.kind === "edit" && !readOnly;

  const startEditing = () => {
    onView({ kind: "edit" });
  };

  return (
    <section className={styles.section} aria-label={t("panel-description")} data-description={task.key}>
      <div className={styles.sectionHead}>
        <span className="caption grow">{t("panel-description")}</span>
        {!editing && (
          <>
            <Button
              className={classes("btn flat sm", view.kind === "history" && "on")}
              aria-pressed={view.kind === "history"}
              onPress={() => {
                onView(view.kind === "history" ? { kind: "read" } : { kind: "history" });
              }}
            >
              <HistoryIcon className="ic s" />
              {t("description-history")}
            </Button>
            <Button className="btn flat sm" data-edit="" isDisabled={readOnly} onPress={startEditing}>
              {t("panel-edit")}
            </Button>
          </>
        )}
      </div>
      {!editing && draft !== undefined && draft.text !== task.description && (
        <div className={classes("banner", styles.banner)} role="status">
          <span className="grow">{t("description-draft-kept")}</span>
          <Button className="btn sm" isDisabled={readOnly} onPress={startEditing}>
            {t("description-draft-continue")}
          </Button>
          <Button
            className="btn flat sm"
            onPress={() => {
              setDraft(null);
            }}
          >
            {t("description-draft-discard")}
          </Button>
        </div>
      )}
      {task.conflict_copies > 0 && view.kind !== "resolve" && !editing && (
        <div className={classes("banner", styles.banner, styles.conflictBanner)} role="alert">
          <span className="grow">{t("description-conflict", { count: task.conflict_copies })}</span>
          <Button className="btn sm" isDisabled={readOnly} onPress={() => { onView({ kind: "resolve" }); }}>
            {t("description-resolve")}
          </Button>
        </div>
      )}
      {editing ? (
        <DescriptionEditor
          task={task}
          kept={draft}
          onDraft={setDraft}
          onDone={() => {
            onView({ kind: "read" });
          }}
        />
      ) : view.kind === "history" ? (
        <DescriptionHistory key={view.compare?.join(" ") ?? "all"} task={task} readOnly={readOnly} compare={view.compare} onClose={() => { onView({ kind: "read" }); }} />
      ) : view.kind === "resolve" && task.conflict_copies > 0 ? (
        <ConflictResolver
          task={task}
          onEdit={(copy, text) => {
            setDraft({ text, base: task.description_revision, resolving: copy });
            onView({ kind: "edit" });
          }}
          onClose={() => { onView({ kind: "read" }); }}
        />
      ) : (
        <DescriptionReader task={task} readOnly={readOnly} onOpenTask={onOpenTask} />
      )}
    </section>
  );
}

function DescriptionReader({ task, readOnly, onOpenTask }: { task: Task; readOnly: boolean; onOpenTask: (key: string) => void }) {
  const t = useT();
  const call = useCall();
  const rendered = useRenderedDescription(task.key);
  const item = rendered.data?.items[0];
  if (task.description === "") {
    return <p className="faint">{t("panel-no-description")}</p>;
  }
  if (!item) {
    return rendered.isError ? <p className="faint">{rendered.error.error.message}</p> : null;
  }
  return (
    <MarkdownReader
      html={item.html}
      label={t("panel-description")}
      onOpenTask={onOpenTask}
      onToggle={readOnly ? undefined : (click) => toggleChecklist(call, { task: task.key }, item.checklist, click)}
    />
  );
}

/** Saves a description and tells what became of it. */
function useSaveDescription(task: Task) {
  const t = useT();
  const toasts = useToasts();
  const update = useUpdateTask();
  const resolve = useCommand("task.resolve_conflict");
  return {
    pending: update.isPending || resolve.isPending,
    save: (draft: Draft, done: () => void) => {
      if (draft.resolving !== undefined) {
        resolve.mutate(
          { key: task.key, copy: draft.resolving, action: "use_text", text: draft.text, base_revision: draft.base ?? null },
          {
            onSuccess: () => {
              toasts.show({ tone: "info", text: t("description-resolved") });
              done();
            },
          },
        );
        return;
      }
      update.mutate(
        { key: task.key, description: draft.text, base_revision: draft.base ?? null },
        {
          onSuccess: (change) => {
            const outcome = change.description_save?.outcome;
            if (outcome === "merged") {
              toasts.show({ tone: "info", text: t("description-merged") });
            } else if (outcome === "conflict") {
              toasts.show({ tone: "error", text: t("description-conflict-saved"), detail: t("description-conflict-detail") });
            }
            done();
          },
        },
      );
    },
  };
}

/**
 * The description's editor. Without a kept draft it starts from the text
 * and revision as they are when it opens, and keeps both together: a save
 * made meanwhile elsewhere doesn't move them, so this save names the
 * revision its text came from and the engine merges the two, or keeps a
 * conflict copy. Following the live text instead would let a save pair an
 * old text with the newer revision and overwrite the other change.
 */
function DescriptionEditor({ task, kept, onDraft, onDone }: { task: Task; kept: Draft | undefined; onDraft: (draft: Draft | null) => void; onDone: () => void }) {
  const t = useT();
  const [source, setSource] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [opened] = useState<Draft>(() => ({ text: task.description, base: task.description_revision }));
  const draft = kept ?? opened;
  const { save, pending } = useSaveDescription(task);
  // Nothing to save: the text as it opened, or as it is now.
  const unchanged = draft.text === task.description || (draft.base === opened.base && draft.text === opened.text);
  const changed = draft.resolving !== undefined || !unchanged;
  const submit = () => {
    if (pending) {
      return;
    }
    if (!changed) {
      onDraft(null);
      onDone();
      return;
    }
    save(draft, () => {
      onDraft(null);
      onDone();
    });
  };
  const cancel = () => {
    if (changed && !confirming) {
      setConfirming(true);
      return;
    }
    onDraft(null);
    onDone();
  };
  return (
    <div className={styles.descriptionEditor}>
      {draft.resolving !== undefined && <p className={classes("faint", styles.small)}>{t("description-resolving")}</p>}
      <MarkdownField
        label={t("panel-description")}
        value={draft.text}
        mode={source ? "source" : "preview"}
        autoFocus
        attachTo={task.key}
        onChange={(text) => {
          setConfirming(false);
          onDraft({ ...draft, text });
        }}
        onSubmit={submit}
        onCancel={cancel}
      />
      {confirming ? (
        <div className={classes("banner", styles.banner)} role="alertdialog" aria-label={t("description-discard-question")}>
          <span className="grow">{t("description-discard-question")}</span>
          <Button className="btn sm" autoFocus onPress={() => { setConfirming(false); }}>
            {t("description-keep-editing")}
          </Button>
          <Button className="btn sm danger" onPress={cancel}>
            {t("description-discard")}
          </Button>
        </div>
      ) : (
        <div className={styles.sectionHead}>
          <Button
            className={classes("btn flat sm", source && "on")}
            aria-pressed={source}
            onPress={() => {
              setSource((on) => !on);
            }}
          >
            {t("description-source")}
          </Button>
          <span className={classes("faint grow", styles.small)}>{t("description-keys")}</span>
          <Button className="btn flat sm" onPress={cancel}>
            {t("dialog-cancel")}
          </Button>
          <Button className="btn primary sm" isDisabled={pending} onPress={submit}>
            {t("panel-save")}
          </Button>
        </div>
      )}
    </div>
  );
}

/** Who saved a revision and how it came about. */
function revisionLine(t: Translate, moment: (at: string) => string, revision: RevisionInfo): string {
  const who = revision.author.kind === "user" ? t("comment-me") : revision.author.name;
  return t("description-revision-line", { kind: t(`description-kind-${revision.kind}`), actor: who, time: moment(revision.created_at) });
}

function ConflictResolver({ task, onEdit, onClose }: { task: Task; onEdit: (copy: string, text: string) => void; onClose: () => void }) {
  const t = useT();
  const toasts = useToasts();
  const moment = useMomentFormat();
  const history = useOp("description.history", { task: task.key });
  const copies = (history.data?.items ?? []).filter((revision) => revision.kind === "conflict" && revision.resolved_at === undefined);
  const current = history.data?.items.find((revision) => revision.current);
  const copy = copies.at(-1);
  const detail = useOp(
    "description.revision",
    { revision: copy?.id ?? "", ...(current ? { diff_from: current.id } : {}) },
    { enabled: copy !== undefined },
  );
  const resolve = useCommand("task.resolve_conflict");
  const run = (action: ConflictAction) => {
    if (!copy) {
      return;
    }
    resolve.mutate(
      { key: task.key, copy: copy.id, action, base_revision: current?.id ?? null },
      {
        onSuccess: () => {
          toasts.show({ tone: "info", text: t("description-resolved") });
          if (copies.length <= 1) {
            onClose();
          }
        },
      },
    );
  };
  if (!copy) {
    return history.isError ? <p className="faint">{history.error.error.message}</p> : null;
  }
  return (
    <div className={styles.resolver} role="group" aria-label={t("description-resolve-title")}>
      <div className={styles.sectionHead}>
        <span className="grow">
          <strong>{t("description-resolve-title")}</strong>
          <span className={classes("faint", styles.small)}> · {revisionLine(t, moment, copy)}</span>
        </span>
        <Button className="btn flat icon sm" aria-label={t("description-close")} onPress={onClose}>
          <X className="ic s" />
        </Button>
      </div>
      <p className={classes("faint", styles.small)}>{t("description-resolve-help", { count: copies.length })}</p>
      {detail.data?.diff !== undefined && <DiffView diff={detail.data.diff} label={t("description-copy-diff")} />}
      <div className={styles.sectionHead}>
        <div className="grow" />
        <Button className="btn sm" isDisabled={resolve.isPending} onPress={() => { run("keep_current"); }}>
          {t("description-keep-current")}
        </Button>
        <Button
          className="btn sm"
          isDisabled={detail.data === undefined}
          onPress={() => {
            if (detail.data) {
              onEdit(copy.id, detail.data.body);
            }
          }}
        >
          {t("description-merge-by-hand")}
        </Button>
        <Button className="btn primary sm" isDisabled={resolve.isPending} onPress={() => { run("use_copy"); }}>
          {t("description-use-copy")}
        </Button>
      </div>
    </div>
  );
}

function DescriptionHistory({ task, readOnly, compare, onClose }: { task: Task; readOnly: boolean; compare: readonly [string, string] | undefined; onClose: () => void }) {
  const t = useT();
  const moment = useMomentFormat();
  const update = useUpdateTask();
  const history = useOp("description.history", { task: task.key });
  const items = history.data?.items ?? [];
  const [chosen, setChosen] = useState<readonly string[]>(compare ? [...compare] : []);
  // Newest first: the later of two chosen revisions is the one higher up.
  const ordered = items.filter((revision) => chosen.includes(revision.id));
  const newer = ordered[0];
  const older = ordered[1] ?? (newer ? items.slice(items.indexOf(newer) + 1).find((revision) => revision.kind !== "conflict") : undefined);
  const detail = useOp("description.revision", { revision: newer?.id ?? "", ...(older ? { diff_from: older.id } : {}) }, { enabled: newer !== undefined });
  const current = items.find((revision) => revision.current);
  const choose = (id: string) => {
    setChosen((was) => (was.includes(id) ? was.filter((other) => other !== id) : [...was, id].slice(-2)));
  };
  return (
    <div className={styles.revisionHistory}>
      <div className={styles.sectionHead}>
        <span className={classes("faint grow", styles.small)}>{t("description-history-help")}</span>
        <Button className="btn flat icon sm" aria-label={t("description-close")} onPress={onClose}>
          <X className="ic s" />
        </Button>
      </div>
      {history.isError && <p className="faint">{history.error.error.message}</p>}
      {history.data && items.length === 0 && <p className={classes("faint", styles.small)}>{t("description-history-none")}</p>}
      <ol className={styles.revisionList} aria-label={t("description-history")}>
        {items.map((revision) => (
          <li key={revision.id} className={classes(styles.revisionRow, chosen.includes(revision.id) && styles.chosen)}>
            <label className={styles.revisionPick}>
              <input
                type="checkbox"
                className="check"
                checked={chosen.includes(revision.id)}
                onChange={() => {
                  choose(revision.id);
                }}
              />
              <span className="grow">{revisionLine(t, moment, revision)}</span>
              {revision.current && <span className="chip hue-green">{t("description-current")}</span>}
              {revision.kind === "conflict" && revision.resolved_at === undefined && <span className="chip hue-red">{t("description-open-copy")}</span>}
            </label>
          </li>
        ))}
      </ol>
      {newer && (
        <div className={styles.revisionDetail}>
          <div className={styles.sectionHead}>
            <GitCompare className="ic s muted" />
            <span className={classes("grow", styles.small)}>
              {older
                ? t("description-comparing", { older: moment(older.created_at), newer: moment(newer.created_at) })
                : t("description-first", { time: moment(newer.created_at) })}
            </span>
            {current && newer.id !== current.id && newer.kind !== "conflict" && !readOnly && detail.data && (
              <Button
                className="btn sm"
                isDisabled={update.isPending}
                onPress={() => {
                  update.mutate({ key: task.key, description: detail.data.body, base_revision: current.id }, { onSuccess: onClose });
                }}
              >
                {t("description-restore")}
              </Button>
            )}
          </div>
          {detail.data &&
            (older ? (
              <DiffView diff={detail.data.diff ?? ""} label={t("description-diff")} />
            ) : (
              <pre className={styles.revisionBody}>{detail.data.body}</pre>
            ))}
        </div>
      )}
    </div>
  );
}
