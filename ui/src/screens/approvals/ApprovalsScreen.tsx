// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate } from "@tanstack/react-router";
import { Check, ExternalLink, TriangleAlert, X } from "lucide-react";
import { useState } from "react";
import { Button, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { Dialog } from "../../components/Dialog";
import { Segmented } from "../../components/Segmented";
import { useToasts } from "../../components/Toasts";
import { engineText, failureText, useCall, useOp } from "../../data/hooks";
import { bySession, proposalTitle, shortId, useProposals, type SessionGroup } from "../../data/proposals";
import { useFailureToast } from "../../data/safety";
import { type FieldChange, type Project, type Proposal, type ProposalApproveResult, type ProposalStatus } from "../../generated/ops";
import { useT, type Translate } from "../../i18n/I18n";
import { fieldLabel, useMomentFormat } from "../task/Activity";
import styles from "./Approvals.module.css";

const FILTERS: readonly ProposalStatus[] = ["pending", "approved", "rejected", "expired"];

/** Fields a preview records that tell nothing to people. */
const QUIET = new Set(["created", "rank", "subtask_rank", "number", "version", "updated_at"]);

/**
 * Approvals: the changes agents proposed that wait for the person, by agent
 * session, each with what it does, the task it changes and its fields
 * before and after, as the preview found them when the agent proposed it.
 * A proposal whose fields changed since says so, and is approved only
 * "anyway". Approve runs the change as the agent; Reject asks for an
 * optional reason the agent reads. One being approved right now shows so;
 * one whose approval was interrupted can only be set aside (rejected), after
 * the person checked the activity log for whether it ran. The decided ones
 * can be shown too.
 */
export function ApprovalsScreen() {
  const t = useT();
  const [status, setStatus] = useState<ProposalStatus>("pending");
  const list = useProposals(status);
  // Waiting includes the ones being approved now, or whose approval was interrupted.
  const approving = useProposals("approving");
  const items = status === "pending" ? [...(list.data?.items ?? []), ...(approving.data?.items ?? [])].sort((a, b) => b.created_at.localeCompare(a.created_at, "en")) : (list.data?.items ?? []);
  const groups = bySession(items);
  return (
    <section className={styles.screen} aria-label={t("view-approvals")} data-screen="approvals">
      <div className={classes("toolbar", styles.toolbar)} role="toolbar" aria-label={t("approvals-toolbar")}>
        <Segmented<ProposalStatus> label={t("approvals-filter")} value={status} segments={FILTERS.map((id) => ({ id, label: t(`approvals-filter-${id}`) }))} onChange={setStatus} />
      </div>
      <div className={styles.scroll}>
        <p className={classes("faint", styles.note, styles.small)}>{t("approvals-about")}</p>
        {list.isError && <p className={classes("faint", styles.note)}>{failureText(t, list.error)}</p>}
        {list.data && items.length === 0 && <p className={classes("muted", styles.note)} data-testid="approvals-none">{t(status === "pending" ? "approvals-none" : "approvals-none-decided")}</p>}
        {groups.map((group) => (
          <SessionBlock key={group.session ?? group.agent} group={group} waiting={status === "pending"} />
        ))}
      </div>
    </section>
  );
}

/** The toast after approving: how many ran, and why the others didn't. */
function useApproved(): (result: ProposalApproveResult) => void {
  const t = useT();
  const toasts = useToasts();
  return (result) => {
    const failed = result.items.filter((item) => !item.approved);
    if (result.approved > 0) {
      toasts.show({ tone: "info", text: t("approvals-approved-toast", { count: result.approved }) });
    }
    const first = failed[0]?.error;
    if (first) {
      toasts.show({ tone: "error", text: t("approvals-not-approved", { count: failed.length }), detail: engineText(t, first.details?.message_id, first.details?.args, first.message) });
    }
  };
}

function SessionBlock({ group, waiting }: { group: SessionGroup; waiting: boolean }) {
  const t = useT();
  const call = useCall();
  const fail = useFailureToast();
  const approved = useApproved();
  const [busy, setBusy] = useState(false);
  const session = group.session;
  const title = session === undefined ? group.agent : t("approvals-session", { agent: group.agent, session: shortId(session) });
  return (
    <section className={styles.session} aria-label={title} data-session={session ?? ""}>
      <header className={styles.sessionHeader}>
        <h2 className={styles.sessionTitle}>{title}</h2>
        <span className="faint">{t("approvals-session-count", { count: group.items.length })}</span>
        <div className="grow" />
        {waiting && session !== undefined && group.items.filter((item) => item.status === "pending").length > 1 && (
          <Button
            className="btn"
            isDisabled={busy}
            onPress={() => {
              setBusy(true);
              call("proposal.approve", { session }).then(
                (result) => {
                  setBusy(false);
                  approved(result);
                },
                (error: unknown) => {
                  setBusy(false);
                  fail(error);
                },
              );
            }}
          >
            <Check className="ic s" />
            {t("approvals-approve-all")}
          </Button>
        )}
      </header>
      <ul className={styles.list}>
        {group.items.map((proposal) => (
          <ProposalCard key={proposal.id} proposal={proposal} />
        ))}
      </ul>
    </section>
  );
}

function ProposalCard({ proposal }: { proposal: Proposal }) {
  const t = useT();
  const { locale } = useLocale();
  const call = useCall();
  const fail = useFailureToast();
  const toasts = useToasts();
  const approved = useApproved();
  const navigate = useNavigate();
  const moment = useMomentFormat();
  const [rejecting, setRejecting] = useState(false);
  const [busy, setBusy] = useState(false);
  const project = useOp("project.get", { project: proposal.project ?? "" }, { enabled: proposal.project !== undefined });
  const conflicts = proposal.conflicts ?? [];
  const waiting = proposal.status === "pending";
  const approving = proposal.status === "approving";
  const interrupted = approving && proposal.interrupted === true;
  const title = proposalTitle(t, proposal);
  const value = (field: string, shown: unknown) => valueText(t, locale, project.data, field, shown);

  const approve = () => {
    setBusy(true);
    call("proposal.approve", { ids: [proposal.id], ...(conflicts.length > 0 ? { force: true } : {}) }).then(
      (result) => {
        setBusy(false);
        approved(result);
      },
      (error: unknown) => {
        setBusy(false);
        fail(error);
      },
    );
  };
  const reject = (reason: string) => {
    setBusy(true);
    call("proposal.reject", { ids: [proposal.id], ...(reason.trim() === "" ? {} : { reason: reason.trim() }) }).then(
      (result) => {
        setBusy(false);
        setRejecting(false);
        toasts.show({ tone: "info", text: t("approvals-rejected-toast", { count: result.items.length }) });
      },
      (error: unknown) => {
        setBusy(false);
        fail(error);
      },
    );
  };

  return (
    <li className={classes("card", styles.proposal)} aria-label={title} data-proposal={proposal.id} data-status={proposal.status}>
      <div className={styles.head}>
        <span className={styles.title}>{title}</span>
        {proposal.task && (
          <Button className={classes("linkbtn", styles.task)} onPress={() => void navigate({ to: "/open/$key", params: { key: proposal.task?.id ?? "" } })}>
            <ExternalLink className="ic s" />
            {t("activity-task", { key: proposal.task.key, title: proposal.task.title })}
          </Button>
        )}
      </div>
      <p className={classes("faint", styles.small, styles.line)}>
        <span>{t("approvals-proposed", { time: moment(proposal.created_at) })}</span>
        {waiting && <span>{t("approvals-expires", { time: moment(proposal.expires_at) })}</span>}
        {approving && !interrupted && (
          <span className="chip hue-blue" data-testid="proposal-approving">
            {t("approvals-being-approved")}
          </span>
        )}
        {!waiting && !approving && proposal.decided_at !== undefined && <span>{t("approvals-decided", { status: proposal.status, time: moment(proposal.decided_at) })}</span>}
      </p>
      {proposal.reason !== undefined && <p className={classes("muted", styles.small)}>{t("approvals-reason", { reason: proposal.reason })}</p>}
      {!proposal.previewed ? (
        <p className={classes("faint", styles.small)}>{t("approvals-no-preview")}</p>
      ) : (
        <Changes proposal={proposal} value={value} />
      )}
      {waiting && proposal.last_error !== undefined && (
        <div className={classes("callout hue-red", styles.conflicts)} role="note" data-testid="proposal-failed">
          <TriangleAlert className="ic s" />
          <p className={styles.small}>{t("approvals-last-error", { error: proposal.last_error })}</p>
        </div>
      )}
      {interrupted && (
        <div className={classes("callout hue-orange", styles.conflicts)} role="note" data-testid="proposal-interrupted">
          <TriangleAlert className="ic s" />
          <p className={styles.small}>{t("approvals-interrupted", { time: moment(proposal.claimed_at ?? proposal.created_at) })}</p>
        </div>
      )}
      {waiting && conflicts.length > 0 && (
        <div className={classes("callout hue-orange", styles.conflicts)} role="note" data-testid="proposal-conflicts">
          <TriangleAlert className="ic s" />
          <div>
            <p className={styles.small}>{t("approvals-conflicts", { count: conflicts.length })}</p>
            <ul className={styles.fields}>
              {conflicts.map((conflict) => (
                <li key={`${conflict.id}-${conflict.field}`} className={styles.small}>
                  {t("approvals-conflict", { field: fieldLabel(t, conflict.field) ?? conflict.field, then: value(conflict.field, conflict.then), now: value(conflict.field, conflict.now) })}
                </li>
              ))}
            </ul>
          </div>
        </div>
      )}
      {waiting && (
        <div className={styles.actions}>
          <Button className="btn" isDisabled={busy} onPress={() => { setRejecting(true); }}>
            <X className="ic s" />
            {t("approvals-reject")}
          </Button>
          <Button className={classes("btn", conflicts.length === 0 && "primary")} isDisabled={busy} onPress={approve}>
            <Check className="ic s" />
            {t(conflicts.length > 0 ? "approvals-approve-anyway" : "approvals-approve")}
          </Button>
        </div>
      )}
      {interrupted && (
        <div className={styles.actions}>
          <Button className="btn" isDisabled={busy} onPress={() => { setRejecting(true); }}>
            <X className="ic s" />
            {t("approvals-set-aside")}
          </Button>
        </div>
      )}
      {rejecting && (
        <RejectDialog
          setAside={interrupted}
          title={title}
          busy={busy}
          onClose={() => {
            setRejecting(false);
          }}
          onReject={reject}
        />
      )}
    </li>
  );
}

/** The fields a proposal changes, item by item, before → after. */
function Changes({ proposal, value }: { proposal: Proposal; value: (field: string, shown: unknown) => string }) {
  const t = useT();
  const lines: { key: string; text: string }[] = [];
  for (const change of proposal.changes) {
    const entity = t(`activity-entity-${entityName(change.entity)}`);
    const trashed = change.fields.trashed;
    if (trashed !== undefined) {
      // Moving to the trash or back tells all: the item's other fields only name it.
      lines.push({ key: `${change.id}-trashed`, text: t(trashed.after === true ? "activity-trashed" : "activity-restored", { entity }) });
      continue;
    }
    const created = "created" in change.fields;
    if (created) {
      lines.push({ key: `${change.id}-created`, text: t("approvals-new-item", { entity }) });
    }
    for (const [field, recorded] of Object.entries(change.fields)) {
      const label = QUIET.has(field) ? undefined : fieldLabel(t, field);
      if (label === undefined) {
        continue;
      }
      lines.push({
        key: `${change.id}-${field}`,
        text: created ? t("approvals-field-new", { field: label, after: value(field, recorded.after) }) : t("approvals-field", { field: label, ...sides(recorded, (shown) => value(field, shown)) }),
      });
    }
  }
  if (lines.length === 0) {
    return <p className={classes("faint", styles.small)}>{t("approvals-no-changes")}</p>;
  }
  return (
    <ul className={styles.fields} aria-label={t("approvals-changes")} data-testid="proposal-diff">
      {lines.map((line) => (
        <li key={line.key} className={styles.small}>
          {line.text}
        </li>
      ))}
    </ul>
  );
}

function sides(change: FieldChange, show: (value: unknown) => string): { before: string; after: string } {
  return { before: show(change.before), after: show(change.after) };
}

const ENTITIES = new Set(["task", "project", "comment", "attachment", "time_entry", "relation", "status", "task_type", "label", "view", "version", "sprint", "wiki_page", "template", "work_kind", "automation_rule", "dashboard", "project_folder", "baseline"]);

function entityName(entity: string): string {
  return ENTITIES.has(entity) ? entity : "other";
}

/** A recorded value as people read it: names for statuses, types and labels of the project. */
export function valueText(t: Translate, locale: string, project: Project | undefined, field: string, value: unknown): string {
  if (value === undefined || value === null || value === "") {
    return t("prop-none");
  }
  const byId = (list: readonly { id: string; name: string }[] | undefined, id: unknown) => list?.find((item) => item.id === id)?.name ?? plain(id);
  switch (field) {
    case "status":
      return byId(project?.statuses, value);
    case "type":
      return byId(project?.types, value);
    case "labels":
      return Array.isArray(value) ? new Intl.ListFormat(locale, { style: "long", type: "conjunction" }).format(value.map((id) => byId(project?.labels, id))) : plain(value);
    case "priority":
      return typeof value === "string" && ["none", "low", "normal", "high", "urgent"].includes(value) ? t(`priority-${value}`) : plain(value);
    default:
      return Array.isArray(value) ? new Intl.ListFormat(locale, { style: "long", type: "conjunction" }).format(value.map(plain)) : plain(value);
  }
}

function plain(value: unknown): string {
  return typeof value === "string" || typeof value === "number" || typeof value === "boolean" ? String(value) : JSON.stringify(value);
}

function RejectDialog({ title, setAside, busy, onClose, onReject }: { title: string; setAside: boolean; busy: boolean; onClose: () => void; onReject: (reason: string) => void }) {
  const t = useT();
  const [reason, setReason] = useState("");
  return (
    <Dialog
      title={t(setAside ? "approvals-set-aside-title" : "approvals-reject-title")}
      isOpen
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
      footer={
        <>
          <Button className="btn" onPress={onClose}>
            {t("dialog-cancel")}
          </Button>
          <Button className="btn danger" isDisabled={busy} onPress={() => { onReject(reason); }}>
            {t(setAside ? "approvals-set-aside-confirm" : "approvals-reject-confirm")}
          </Button>
        </>
      }
    >
      <p className={styles.small}>{title}</p>
      <label className={styles.reason}>
        <span className={styles.small}>{t("approvals-reject-reason")}</span>
        <textarea
          className={styles.textarea}
          autoFocus
          rows={4}
          maxLength={2000}
          value={reason}
          onChange={(event) => {
            setReason(event.target.value);
          }}
        />
      </label>
    </Dialog>
  );
}
