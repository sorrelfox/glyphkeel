// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useQueryClient } from "@tanstack/react-query";
import { Copy, Terminal } from "lucide-react";
import { useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { Segmented } from "../../components/Segmented";
import { Select } from "../../components/Select";
import { useToasts } from "../../components/Toasts";
import { useCall, useCommand, useOp } from "../../data/hooks";
import { opPrefix } from "../../data/queries";
import { skippedReason, useFailureToast } from "../../data/safety";
import { Switch } from "../../components/Switch";
import { type AgentLimit, type AgentLimitsPatch, type AgentSession, type Profile } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { useCliStatus, useInstallCli, type CliStatus } from "../../ipc/cli";
import { useMomentFormat } from "../../screens/task/Activity";
import { Group, NumberSetting, Row } from "../PreferenceRows";
import styles from "../Preferences.module.css";
import { MCP_CLIENTS, projectSnippet } from "./clients";

const PROFILES: readonly Profile[] = ["read-only", "standard", "full"];

/** The program agents' clients run, until the app says where it is: the bare name. */
const BARE_COMMAND: readonly string[] = ["glyphkeel"];

/**
 * Agents & CLI: where the command-line tool is (and a link to install it),
 * the set-up to copy into common MCP clients, the most an agent may do, the
 * limits on each agent session (only the person changes them, here), and
 * the agent sessions, each of which can be reverted.
 */
export function AgentsPage() {
  const cli = useCliStatus();
  const command = cli.data?.command ?? BARE_COMMAND;
  return (
    <>
      <CliGroup status={cli.data} isPending={cli.isPending} />
      <ClientsGroup command={command} />
      <PolicyGroup />
      <LimitsGroup />
      <SessionsGroup />
    </>
  );
}

// ---------- the command-line tool ----------

function CliGroup({ status, isPending }: { status: CliStatus | null | undefined; isPending: boolean }) {
  const t = useT();
  const install = useInstallCli();
  if (isPending) {
    return null;
  }
  return (
    <Group title={t("agents-cli")}>
      {status === null || status === undefined ? (
        <Row label={t("agents-cli-tool")} about={t("agents-cli-outside-app")}>
          <code className={styles.command}>{BARE_COMMAND.join(" ")}</code>
        </Row>
      ) : status.flatpak ? (
        <Row label={t("agents-cli-tool")} about={t("agents-cli-flatpak")}>
          <code className={classes("selectable", styles.command)}>{status.command.join(" ")}</code>
        </Row>
      ) : status.on_path !== undefined ? (
        <Row label={t("agents-cli-tool")} about={t("agents-cli-on-path", { path: status.on_path })}>
          <span className="chip hue-green" data-testid="cli-state">
            {t("agents-cli-on-path-chip")}
          </span>
        </Row>
      ) : status.link !== undefined ? (
        <Row
          label={t("agents-cli-tool")}
          about={t(status.link_dir_on_path ? "agents-cli-not-on-path" : "agents-cli-not-on-path-dir", { link: status.link })}
        >
          <Button
            className="btn"
            isDisabled={install.isPending}
            onPress={() => {
              install.mutate();
            }}
          >
            <Terminal className="ic s" />
            {t("agents-cli-install")}
          </Button>
        </Row>
      ) : status.bundled !== undefined ? (
        <Row label={t("agents-cli-tool")} about={t("agents-cli-bundled-not-on-path", { path: status.bundled })}>
          <span className="chip" data-testid="cli-state">
            {t("agents-cli-not-on-path-chip")}
          </span>
        </Row>
      ) : (
        <Row label={t("agents-cli-tool")} about={t("agents-cli-missing")}>
          <span className="chip" data-testid="cli-state">
            {t("agents-cli-missing-chip")}
          </span>
        </Row>
      )}
    </Group>
  );
}

// ---------- set-up for MCP clients ----------

/** A snippet to copy, with its Copy button. */
function Snippet({ text, label }: { text: string; label: string }) {
  const t = useT();
  const toasts = useToasts();
  return (
    <div className={styles.snippet}>
      <pre className={classes("selectable", styles.snippetText)} aria-label={label}>
        {text}
      </pre>
      <Button
        className="btn sm"
        onPress={() => {
          navigator.clipboard.writeText(text).then(
            () => {
              toasts.show({ tone: "info", text: t("agents-copied") });
            },
            () => {
              toasts.show({ tone: "error", text: t("agents-copy-failed") });
            },
          );
        }}
      >
        <Copy className="ic s" />
        {t("agents-copy")}
      </Button>
    </div>
  );
}

function ClientsGroup({ command }: { command: readonly string[] }) {
  const t = useT();
  const [clientId, setClientId] = useState(MCP_CLIENTS[0]?.id ?? "");
  const client = MCP_CLIENTS.find((each) => each.id === clientId) ?? MCP_CLIENTS[0];
  const projects = useOp("project.list", {});
  const keys = (projects.data?.items ?? []).filter((each) => each.state === "active").map((each) => each.key);
  const [chosenProject, setProject] = useState<string | null>(null);
  const project = chosenProject !== null && keys.includes(chosenProject) ? chosenProject : keys[0];
  if (!client) {
    return null;
  }
  return (
    <Group title={t("agents-clients")}>
      <Row label={t("agents-client")} about={t("agents-clients-about")}>
        <Select
          label={t("agents-client")}
          value={client.id}
          options={MCP_CLIENTS.map((each) => ({ id: each.id, label: each.name }))}
          onChange={setClientId}
        />
      </Row>
      <div className={styles.snippetBlock}>
        <p className={styles.snippetWhere}>
          {client.file === undefined ? t("agents-client-run", { client: client.name }) : t("agents-client-file", { client: client.name, file: client.file })}
        </p>
        <Snippet text={client.snippet(command)} label={t("agents-client-snippet", { client: client.name })} />
      </div>
      {project !== undefined && (
        <>
          <Row label={t("agents-project-scope")} about={t("agents-project-scope-about")}>
            <Select
              label={t("agents-project-scope")}
              value={project}
              options={keys.map((key) => ({ id: key, label: key }))}
              onChange={setProject}
            />
          </Row>
          <div className={styles.snippetBlock}>
            <p className={styles.snippetWhere}>{t("agents-project-file", { file: ".mcp.json" })}</p>
            <Snippet text={projectSnippet(command, project)} label={t("agents-project-snippet", { project })} />
          </div>
        </>
      )}
    </Group>
  );
}

// ---------- what agents may do ----------

/** One limit on agent sessions, as the page shows it. */
interface LimitRow {
  limit: AgentLimit;
  /** The start of its message IDs. */
  message: string;
  /** The engine's default; undefined: no limit. */
  byDefault: number | undefined;
  /** The number offered when a lifted limit is set again. */
  offered: number;
}

export const LIMIT_ROWS: readonly LimitRow[] = [
  { limit: "writes_per_minute", message: "agents-limit-writes", byDefault: 120, offered: 120 },
  { limit: "new_tasks_per_day", message: "agents-limit-tasks", byDefault: undefined, offered: 500 },
  { limit: "trash_items_per_call", message: "agents-limit-trash-call", byDefault: 20, offered: 20 },
  { limit: "trash_items_per_hour", message: "agents-limit-trash-hour", byDefault: 50, offered: 50 },
];

/** The largest number a limit can be set to, as the engine accepts it. */
export const LIMIT_MAX = 1_000_000;

/** The change that sets `limit` to `value`, or lifts it when `value` is null. */
export function limitPatch(limit: AgentLimit, value: number | null): AgentLimitsPatch {
  return value === null ? { no_limit: [limit] } : { [limit]: value };
}

function PolicyGroup() {
  const t = useT();
  const queryClient = useQueryClient();
  const settings = useOp("settings.get", {});
  const update = useCommand("agent.policy.update");
  if (settings.isError) {
    return <p className={styles.error}>{settings.error.error.message}</p>;
  }
  if (!settings.data) {
    return null;
  }
  const current = settings.data.agent_max_profile;
  return (
    <Group title={t("agents-policy")}>
      <Row label={t("agents-max-profile")} about={t(`agents-profile-${current}-about`)}>
        <Segmented<Profile>
          label={t("agents-max-profile")}
          value={current}
          segments={PROFILES.map((profile) => ({ id: profile, label: t(`agents-profile-${profile}`) }))}
          onChange={(max_profile) => {
            update.mutate(
              { max_profile },
              {
                onSettled: () => {
                  void queryClient.invalidateQueries({ queryKey: opPrefix("settings.get") });
                },
              },
            );
          }}
        />
      </Row>
      <Row label={t("agents-raise")} about={t("agents-raise-about")} />
    </Group>
  );
}

// ---------- limits on each agent session ----------

function LimitsGroup() {
  const t = useT();
  const queryClient = useQueryClient();
  const settings = useOp("settings.get", {});
  const update = useCommand("agent.limits.update");
  if (!settings.data) {
    // The policy above shows why the settings can't be read.
    return null;
  }
  const limits = settings.data.agent_limits;
  const change = (limit: AgentLimit, value: number | null) => {
    update.mutate(limitPatch(limit, value), {
      onSettled: () => {
        void queryClient.invalidateQueries({ queryKey: opPrefix("settings.get") });
        void queryClient.invalidateQueries({ queryKey: opPrefix("workspace.get") });
      },
    });
  };
  return (
    <Group title={t("agents-limits")}>
      {LIMIT_ROWS.map((row) => {
        const value = limits[row.limit];
        const lifted = value === undefined;
        const label = t(row.message);
        return (
          <Row key={row.limit} label={label} about={t(`${row.message}-about`, row.byDefault === undefined ? {} : { default: row.byDefault })}>
            <NumberSetting
              label={label}
              value={value ?? row.offered}
              min={1}
              max={LIMIT_MAX}
              unit={t(`${row.message}-unit`, { count: value ?? row.offered })}
              isDisabled={lifted}
              wide
              onSave={(number) => {
                change(row.limit, number);
              }}
            />
            <Switch
              aria-label={t(`${row.message}-none`)}
              isSelected={lifted}
              onChange={(none) => {
                change(row.limit, none ? null : row.offered);
              }}
            >
              {t("agents-no-limit")}
            </Switch>
          </Row>
        );
      })}
      <Row label={t("agents-limits-only-here")} about={t("agents-limits-only-here-about")} />
    </Group>
  );
}

// ---------- sessions ----------

function SessionsGroup() {
  const t = useT();
  const sessions = useOp("agent.sessions", { limit: 50 });
  return (
    <Group title={t("agents-sessions")}>
      {sessions.isError && <p className={styles.error}>{sessions.error.error.message}</p>}
      {sessions.data?.items.length === 0 && (
        <Row label={t("agents-sessions-none")} about={t("agents-sessions-none-about")} />
      )}
      {sessions.data?.items.map((session) => (
        <SessionRow key={session.id} session={session} />
      ))}
      {sessions.data !== undefined && sessions.data.total > sessions.data.items.length && (
        <p className={classes("faint", styles.snippetWhere)}>{t("agents-sessions-more", { count: sessions.data.total - sessions.data.items.length })}</p>
      )}
    </Group>
  );
}

/** One session: who, with which profile, when, how much it read and changed; Revert asks first. */
function SessionRow({ session }: { session: AgentSession }) {
  const t = useT();
  const moment = useMomentFormat();
  const call = useCall();
  const toasts = useToasts();
  const fail = useFailureToast();
  // The changes a revert would undo, once counted; null while not asking.
  const [asking, setAsking] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const running = session.ended_at === undefined;
  const name = session.client_version === undefined ? session.name : t("agents-session-name-version", { name: session.name, version: session.client_version });
  const count = () => {
    setBusy(true);
    call("history.revert", { session: session.id, dry_run: true }).then(
      (result) => {
        setBusy(false);
        if (result.reverted.length === 0) {
          toasts.show({ tone: "info", text: t("agents-revert-nothing") });
        } else {
          setAsking(result.reverted.length);
        }
      },
      (error: unknown) => {
        setBusy(false);
        fail(error);
      },
    );
  };
  const revert = () => {
    setBusy(true);
    call("history.revert", { session: session.id }).then(
      (result) => {
        setBusy(false);
        setAsking(null);
        toasts.show({
          tone: result.skipped.length > 0 ? "error" : "info",
          text: t("agents-reverted", { reverted: result.reverted.length, skipped: result.skipped.length }),
          detail: result.skipped[0] === undefined ? undefined : skippedReason(t, result.skipped[0]),
        });
      },
      (error: unknown) => {
        setBusy(false);
        fail(error);
      },
    );
  };
  return (
    <div className={styles.session} data-testid="agent-session">
      <div className={styles.row}>
        <span className={styles.text}>
          <span>
            {name} <span className="chip hue-purple">{t(`agents-profile-${session.profile}`)}</span>{" "}
            {running && <span className="chip hue-green">{t("agents-session-running")}</span>}
          </span>
          <small>
            {running
              ? t("agents-session-since", { started: moment(session.started_at), seen: moment(session.last_seen_at) })
              : t("agents-session-ran", { started: moment(session.started_at), ended: moment(session.ended_at ?? session.last_seen_at) })}
          </small>
          <small>{t("agents-session-counts", { reads: session.reads, writes: session.writes })}</small>
        </span>
        <span className={styles.control}>
          {session.writes > 0 && asking === null && (
            <Button className="btn sm" isDisabled={busy} onPress={count}>
              {t("agents-revert")}
            </Button>
          )}
        </span>
      </div>
      {asking !== null && (
        <div className={styles.sessionConfirm} role="group" aria-label={t("agents-revert")}>
          <p>{t("agents-revert-confirm", { count: asking, name: session.name })}</p>
          <span className={styles.sessionButtons}>
            <Button
              className="btn"
              isDisabled={busy}
              onPress={() => {
                setAsking(null);
              }}
            >
              {t("dialog-cancel")}
            </Button>
            <Button className="btn primary" isDisabled={busy} onPress={revert}>
              {t("agents-revert-confirm-button", { count: asking })}
            </Button>
          </span>
        </div>
      )}
    </div>
  );
}
