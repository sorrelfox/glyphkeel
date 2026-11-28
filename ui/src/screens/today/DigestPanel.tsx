// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate } from "@tanstack/react-router";
import { ChevronDown, History, Undo2 } from "lucide-react";
import { type ReactNode, useId, useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { useToasts } from "../../components/Toasts";
import { useDigest, useDismissDigest } from "../../data/digest";
import { failureText, useCall } from "../../data/hooks";
import { useFailureToast } from "../../data/safety";
import { type ActivityTask, type Digest, type DigestRule, type DigestSession, type RevertInput } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { useMomentFormat } from "../task/Activity";
import { useDayFormat } from "../board/CardView";
import styles from "./Digest.module.css";

/** The ID of the digest's heading, which the sidebar's Inbox moves to. */
export const DIGEST_HEADING = "digest-title";

/**
 * The digest at the top of Today: what agents, automation rules, reminders
 * and due dates, and the command line did since the person last marked it
 * as seen, each group with a sentence that sums it up. Sessions and rules
 * can be reverted from here and opened in the activity log or their run
 * log; tasks open in the panel beside Today. Nothing shows while the
 * digest is empty.
 */
export function DigestPanel({ onOpen }: { onOpen: (key: string) => void }) {
  const digest = useDigest();
  if (!digest.data || digest.data.counts.items === 0) {
    return null;
  }
  return <DigestBody digest={digest.data} onOpen={onOpen} />;
}

function DigestBody({ digest, onOpen }: { digest: Digest; onOpen: (key: string) => void }) {
  const t = useT();
  const moment = useMomentFormat();
  const dismiss = useDismissDigest();
  const fail = useFailureToast();
  const aboutId = useId();
  const { counts } = digest;
  return (
    <section className={styles.digest} aria-labelledby={DIGEST_HEADING} data-digest="">
      <div className={styles.head}>
        <h2 id={DIGEST_HEADING} className={styles.title} tabIndex={-1}>
          {t("digest-title")}
        </h2>
        <span className="faint">{t("digest-since", { time: moment(digest.since) })}</span>
        <span className="grow" />
        <Button
          className="btn sm"
          aria-describedby={aboutId}
          isDisabled={dismiss.isPending}
          onPress={() => {
            dismiss.mutate(digest.until, { onError: fail });
          }}
        >
          {t("digest-dismiss")}
        </Button>
        <span id={aboutId} hidden>
          {t("digest-dismiss-about")}
        </span>
      </div>

      {digest.agents.length > 0 && (
        <Group id="agents" title={t("digest-agents")} summary={[t("digest-agents-summary", { sessions: counts.sessions, changes: counts.agent_changes })]}>
          {digest.agents.map((session) => (
            <SessionRow key={session.id} session={session} onOpen={onOpen} />
          ))}
        </Group>
      )}

      {digest.automation.length > 0 && (
        <Group
          id="automation"
          title={t("digest-automation")}
          summary={[t("digest-automation-summary", { rules: counts.rules, runs: counts.runs }), ...(counts.failures > 0 ? [t("digest-automation-failures", { count: counts.failures })] : [])]}
        >
          {digest.automation.map((rule) => (
            <RuleRow key={rule.id} rule={rule} onOpen={onOpen} />
          ))}
        </Group>
      )}

      {(digest.reminders.length > 0 || digest.became_due.length > 0) && (
        <Group
          id="due"
          title={t("digest-due")}
          summary={[
            ...(counts.reminders > 0 ? [t("digest-reminders-summary", { count: counts.reminders })] : []),
            ...(counts.became_due > 0 ? [t("digest-became-due-summary", { count: counts.became_due })] : []),
          ]}
        >
          <DueRows digest={digest} onOpen={onOpen} />
        </Group>
      )}

      {digest.cli.changes > 0 && (
        <Group id="cli" title={t("digest-cli")} summary={[t("digest-cli-summary", { commands: digest.cli.group_count, changes: digest.cli.changes })]}>
          <li className={styles.row}>
            <div className={styles.main}>
              <span className="faint">{digest.cli.last_at === undefined ? "" : t("digest-session-last", { time: moment(digest.cli.last_at) })}</span>
            </div>
            <Tasks tasks={digest.cli.tasks} total={digest.cli.task_count} onOpen={onOpen} />
            <div className={styles.actions}>
              {digest.cli.groups.length > 0 && <RevertButton label={t("digest-cli-revert")} input={{ groups: digest.cli.groups }} />}
            </div>
          </li>
        </Group>
      )}
    </section>
  );
}

/** One group: its heading, its summary sentences and its rows. */
function Group({ id, title, summary, children }: { id: string; title: string; summary: string[]; children: ReactNode }) {
  const headingId = `digest-${id}`;
  return (
    <section className={styles.group} aria-labelledby={headingId} data-digest-group={id}>
      <h3 id={headingId} className={styles.groupHead}>
        {title}
      </h3>
      {summary.length > 0 && (
        <p className={classes("muted", styles.summary)} data-digest-summary="">
          {summary.map((sentence) => (
            <span key={sentence}>{sentence}</span>
          ))}
        </p>
      )}
      <ul className={styles.boxed} aria-label={title}>
        {children}
      </ul>
    </section>
  );
}

/** The tasks a session, rule or the command line touched, as links that open them beside Today. */
function Tasks({ tasks, total, onOpen }: { tasks: ActivityTask[]; total: number; onOpen: (key: string) => void }) {
  const t = useT();
  if (tasks.length === 0) {
    return null;
  }
  return (
    <div className={styles.tasks}>
      {tasks.slice(0, 6).map((task) => (
        <Button
          key={task.id}
          className={classes("linkbtn", styles.task)}
          onPress={() => {
            onOpen(task.key);
          }}
        >
          <span className="key" title={task.title}>
            {task.key}
          </span>
        </Button>
      ))}
      {total > Math.min(tasks.length, 6) && <span className="faint">{t("digest-more-tasks", { count: total - Math.min(tasks.length, 6) })}</span>}
    </div>
  );
}

/** Reverts what `input` names and says how it went; Undo in the activity log takes it back. */
function RevertButton({ label, ariaLabel, input }: { label: string; ariaLabel?: string; input: RevertInput }) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const fail = useFailureToast();
  const [busy, setBusy] = useState(false);
  return (
    <Button
      className="btn flat sm"
      {...(ariaLabel === undefined ? {} : { "aria-label": ariaLabel })}
      isDisabled={busy}
      onPress={() => {
        setBusy(true);
        call("history.revert", input).then(
          (result) => {
            setBusy(false);
            toasts.show(
              result.skipped.length > 0
                ? { tone: "error", text: t("digest-revert-skipped", { count: result.skipped.length }) }
                : { tone: "info", text: t("digest-reverted-toast", { count: result.reverted.length }) },
            );
          },
          (error: unknown) => {
            setBusy(false);
            fail(error);
          },
        );
      }}
    >
      <Undo2 className="ic s" />
      {label}
    </Button>
  );
}

function SessionRow({ session, onOpen }: { session: DigestSession; onOpen: (key: string) => void }) {
  const t = useT();
  const moment = useMomentFormat();
  const navigate = useNavigate();
  const reverted = session.changes > 0 && session.reverted >= session.changes;
  return (
    <li className={classes(styles.row, reverted && styles.reverted)} data-digest-session={session.id}>
      <div className={styles.main}>
        <span className={styles.name}>{session.name}</span>
        {session.client !== undefined && <span className="chip hue-purple" data-client="">{session.client}</span>}
        {session.running && <span className="chip hue-green">{t("digest-session-running")}</span>}
        {reverted && <span className="chip outline">{t("digest-session-reverted")}</span>}
        <span className="faint">{t("digest-session-changes", { changes: session.changes, tasks: session.task_count })}</span>
        <span className="faint">{t("digest-session-last", { time: moment(session.last_at) })}</span>
      </div>
      <Tasks tasks={session.tasks} total={session.task_count} onOpen={onOpen} />
      <div className={styles.actions}>
        <Button
          className="btn flat sm"
          aria-label={t("digest-open-activity-label", { name: session.name })}
          onPress={() => void navigate({ to: "/activity", search: { session: session.id } })}
        >
          <History className="ic s" />
          {t("digest-open-activity")}
        </Button>
        {!reverted && <RevertButton label={t("digest-revert-session")} ariaLabel={t("digest-revert-session-label", { name: session.name })} input={{ session: session.id }} />}
      </div>
    </li>
  );
}

function RuleRow({ rule, onOpen }: { rule: DigestRule; onOpen: (key: string) => void }) {
  const t = useT();
  const moment = useMomentFormat();
  const [open, setOpen] = useState(false);
  const logId = useId();
  return (
    <li className={styles.row} data-digest-rule={rule.id}>
      <div className={styles.main}>
        <span className={styles.name}>{rule.name}</span>
        {rule.project !== undefined && <span className="chip outline" data-project="">{rule.project}</span>}
        <span className="faint">{t("digest-rule-runs", { runs: rule.runs })}</span>
        {rule.failures > 0 && <span className="chip hue-red">{t("digest-rule-failures", { count: rule.failures })}</span>}
        {rule.last_error && <span className={styles.error}>{failureText(t, rule.last_error)}</span>}
      </div>
      <Tasks tasks={rule.tasks} total={rule.task_count} onOpen={onOpen} />
      <div className={styles.actions}>
        <Button
          className="btn flat sm"
          aria-expanded={open}
          aria-controls={logId}
          aria-label={t("digest-rule-log-label", { name: rule.name })}
          onPress={() => {
            setOpen(!open);
          }}
        >
          {t("digest-rule-log")}
          <ChevronDown className={classes("ic s muted", !open && styles.turned)} />
        </Button>
        {rule.groups.length > 0 && <RevertButton label={t("digest-rule-revert")} ariaLabel={t("digest-rule-revert-label", { name: rule.name })} input={{ groups: rule.groups }} />}
      </div>
      {open && (
        <ol id={logId} className={styles.runs} aria-label={t("digest-rule-log-label", { name: rule.name })}>
          {rule.recent.map((run) => (
            <li key={run.id} className={styles.run}>
              <span className={classes("chip", run.result === "done" ? "hue-green" : run.result === "failed" ? "hue-red" : "hue-orange")}>{t(`automation-run-${run.result}`)}</span>
              <time dateTime={run.at}>{moment(run.at)}</time>
              {run.task !== undefined && <span className="key">{run.task}</span>}
              <span className="muted">{t(`automation-trigger-${run.trigger}`)}</span>
              {run.result === "done" && <span className="muted">{t("automation-run-changes", { count: run.changes })}</span>}
              {run.error && <span className={styles.error}>{failureText(t, run.error)}</span>}
            </li>
          ))}
        </ol>
      )}
    </li>
  );
}

function DueRows({ digest, onOpen }: { digest: Digest; onOpen: (key: string) => void }) {
  const t = useT();
  const moment = useMomentFormat();
  const day = useDayFormat();
  return (
    <>
      {digest.reminders.map((reminder) => (
        <li key={reminder.id} className={styles.line}>
          <Button
            className={classes(styles.open)}
            onPress={() => {
              onOpen(reminder.task.key);
            }}
          >
            <span className="chip outline">{t("digest-reminder")}</span>{" "}
            <span className="key">{reminder.task.key}</span> <span className={styles.taskTitle}>{reminder.task.title}</span>{" "}
            {reminder.note === undefined ? (
              <time className="faint" dateTime={reminder.at}>
                {moment(reminder.at)}
              </time>
            ) : (
              <span className="faint">{t("today-reminder-note", { time: moment(reminder.at), note: reminder.note })}</span>
            )}
          </Button>
        </li>
      ))}
      {digest.became_due.map((due) => (
        <li key={due.task.id} className={styles.line}>
          <Button
            className={classes(styles.open)}
            onPress={() => {
              onOpen(due.task.key);
            }}
          >
            <span className={classes("chip", due.overdue ? "hue-red" : "hue-orange")}>{t(due.overdue ? "digest-due-overdue" : "digest-due-today")}</span>{" "}
            <span className="key">{due.task.key}</span> <span className={styles.taskTitle}>{due.task.title}</span>{" "}
            <time className={classes("meta", due.overdue && "overdue")} dateTime={due.due_date}>
              {day(due.due_date)}
            </time>
          </Button>
        </li>
      ))}
    </>
  );
}
