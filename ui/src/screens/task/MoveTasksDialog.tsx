// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useState } from "react";
import { Button } from "react-aria-components";

import { Dialog } from "../../components/Dialog";
import { ProjectMark } from "../../components/ProjectMark";
import { Select, type SelectOption } from "../../components/Select";
import { useToasts } from "../../components/Toasts";
import { useProject } from "../../data/board";
import { errorToast, useCall, useOp, useUndo } from "../../data/hooks";
import { applyBulk, previewBulk } from "../../data/list";
import { type MoveCheck, type MoveMapping, type MoveMatch, type Project } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import styles from "../project/Project.module.css";

/** The choice of a label that the target project has no match for: leave it off. */
const LEAVE_OFF = "";

type Kind = "statuses" | "types" | "labels";
type Choices = Record<Kind, Record<string, string>>;

interface MoveTasksDialogProps {
  /** The tasks to move; their subtasks come along. */
  keys: readonly string[];
  /** The project they are in, when it is one: it isn't offered as the target. */
  from?: string | undefined;
  /** The target chosen already, as the bulk bar does. */
  to?: string | undefined;
  /** Called when the dialog closes; with the first task's new key after a move. */
  onClose: (moved?: string) => void;
}

/**
 * Moves tasks, with their subtasks, comments, files, time and links, to
 * another project. It shows the keys they would get (the old ones keep
 * finding them) and, for each status, type or label the target project
 * has no match for, what to take instead. One Undo step takes it back.
 */
export function MoveTasksDialog({ keys, from, to, onClose }: MoveTasksDialogProps) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const { undo } = useUndo();
  const projects = useOp("project.list", {});
  const targets = (projects.data?.items ?? []).filter((project) => project.state === "active" && project.key !== from);
  const [target, setTarget] = useState<string>(to ?? "");
  const check = useOp("task.move_check", { keys: [...keys], project: target }, { enabled: target !== "" });
  const project = useProject(target === "" ? undefined : target);
  const [choices, setChoices] = useState<Choices>({ statuses: {}, types: {}, labels: {} });
  const [busy, setBusy] = useState(false);

  const shown = check.data && project.data && check.data.project === project.data.key ? { check: check.data, project: project.data } : null;

  const move = () => {
    if (!shown) {
      return;
    }
    const mapping = mappingOf(shown.check, choices);
    const done = (key: string | undefined, text: string) => {
      setBusy(false);
      toasts.show({ tone: "info", text, action: { label: t("toast-undo"), run: undo } });
      onClose(key);
    };
    const failed = (error: unknown) => {
      setBusy(false);
      toasts.show(errorToast(t, error));
    };
    setBusy(true);
    const [only] = keys;
    if (keys.length === 1 && only !== undefined) {
      call("task.move", { key: only, project: shown.project.key, ...(isEmpty(mapping) ? {} : { mapping }) }).then((change) => {
        done(change.task.key, t("move-to-done", { key: change.task.key }));
      }, failed);
      return;
    }
    const changes = { project: shown.project.key, ...(isEmpty(mapping) ? {} : { mapping }) };
    previewBulk(call, keys, changes)
      .then((preview) => applyBulk(call, keys, changes, preview.count))
      .then((result) => {
        done(shown.check.keys[0]?.to, t("move-to-done-many", { count: result.changed, project: shown.project.key }));
      }, failed);
  };

  const projectOptions: SelectOption<string>[] = [
    { id: "", label: t("move-to-choose-project") },
    ...targets.map((candidate) => ({ id: candidate.key, label: t("move-to-project-option", { key: candidate.key, name: candidate.name }), icon: <ProjectMark hue={candidate.hue} icon={candidate.icon} /> })),
  ];

  return (
    <Dialog
      title={t("move-to-title", { count: keys.length })}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) {
          onClose();
        }
      }}
      footer={
        <>
          <Button className="btn" onPress={() => { onClose(); }}>
            {t("dialog-cancel")}
          </Button>
          <Button className="btn primary" isDisabled={!shown || busy || shown.check.keys.length === 0} onPress={move}>
            {t("move-to-confirm")}
          </Button>
        </>
      }
    >
      <div className={styles.form} data-testid="move-dialog">
        <div className={styles.formRow}>
          <span className={styles.formLabel}>{t("move-to-project")}</span>
          {targets.length === 0 && projects.data ? (
            <span className="meta">{t("move-to-no-project")}</span>
          ) : (
            <Select
              label={t("move-to-project")}
              options={projectOptions}
              value={target}
              onChange={(key) => {
                setTarget(key);
                setChoices({ statuses: {}, types: {}, labels: {} });
              }}
            />
          )}
        </div>
        {target !== "" && !shown && !check.isError && <p className="meta">{t("move-to-checking")}</p>}
        {check.isError && (
          <p className={styles.error} role="alert">
            {errorToast(t, check.error).text}
          </p>
        )}
        {shown && <Plan roots={keys.length} check={shown.check} project={shown.project} choices={choices} onChoose={setChoices} />}
      </div>
    </Dialog>
  );
}

/** The new keys, and a choice for each status, type and label without a match. */
function Plan({ roots, check, project, choices, onChoose }: { roots: number; check: MoveCheck; project: Project; choices: Choices; onChoose: (choices: Choices) => void }) {
  const t = useT();
  const [first] = check.keys;
  if (first === undefined) {
    return <p className="meta">{t("move-to-nothing")}</p>;
  }
  const missing = {
    statuses: check.statuses.filter((match) => match.target === undefined),
    types: check.types.filter((match) => match.target === undefined),
    labels: check.labels.filter((match) => match.target === undefined),
  };
  const options: Record<Kind, SelectOption<string>[]> = {
    statuses: project.statuses.map((status) => ({ id: status.name, label: status.name })),
    types: project.types.map((type) => ({ id: type.name, label: type.name })),
    labels: [{ id: LEAVE_OFF, label: t("move-to-leave-off") }, ...project.labels.map((label) => ({ id: label.name, label: label.name }))],
  };
  const fallback: Record<Kind, string> = { statuses: check.fallback_status, types: check.fallback_type, labels: LEAVE_OFF };
  const headings: Record<Kind, string> = { statuses: t("move-to-map-statuses"), types: t("move-to-map-types"), labels: t("move-to-map-labels") };
  const kinds = (["statuses", "types", "labels"] as const).filter((kind) => missing[kind].length > 0);
  return (
    <>
      <p data-testid="move-keys">
        {roots === 1 ? t("move-to-keys-one", { from: first.from, to: first.to, subtasks: check.keys.length - 1 }) : t("move-to-keys-many", { count: check.keys.length, project: project.key })}
      </p>
      {kinds.length === 0 ? (
        <p className="meta">{t("move-to-all-match", { project: project.key })}</p>
      ) : (
        <>
          <p className="meta">{t("move-to-map-intro", { project: project.key })}</p>
          {kinds.map((kind) => (
            <fieldset key={kind} className={styles.form} data-testid={`move-map-${kind}`}>
              <legend className="caption">{headings[kind]}</legend>
              {missing[kind].map((match: MoveMatch) => (
                <div key={match.name} className={styles.formRow}>
                  <span className={styles.formLabel}>{t("move-to-map-row", { name: match.name, count: match.tasks })}</span>
                  <Select
                    label={t("move-to-map-choice", { name: match.name })}
                    options={options[kind]}
                    value={choices[kind][match.name] ?? fallback[kind]}
                    onChange={(value) => {
                      onChoose({ ...choices, [kind]: { ...choices[kind], [match.name]: value } });
                    }}
                  />
                </div>
              ))}
            </fieldset>
          ))}
        </>
      )}
    </>
  );
}

/** The mapping the choices make: only for what has no match, and no label left off. */
export function mappingOf(check: MoveCheck, choices: Choices): MoveMapping {
  const pick = (kind: Kind, matches: readonly MoveMatch[], fallback: string) => {
    const out: Record<string, string> = {};
    for (const match of matches.filter((candidate) => candidate.target === undefined)) {
      const chosen = choices[kind][match.name] ?? fallback;
      if (chosen !== LEAVE_OFF) {
        out[match.name] = chosen;
      }
    }
    return out;
  };
  const statuses = pick("statuses", check.statuses, check.fallback_status);
  const types = pick("types", check.types, check.fallback_type);
  const labels = pick("labels", check.labels, LEAVE_OFF);
  return {
    ...(Object.keys(statuses).length > 0 ? { statuses } : {}),
    ...(Object.keys(types).length > 0 ? { types } : {}),
    ...(Object.keys(labels).length > 0 ? { labels } : {}),
  };
}

function isEmpty(mapping: MoveMapping): boolean {
  return mapping.statuses === undefined && mapping.types === undefined && mapping.labels === undefined;
}
