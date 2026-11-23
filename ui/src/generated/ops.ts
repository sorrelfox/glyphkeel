// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Generated from the operation catalog by `cargo xtask gen`. Do not edit:
// change the catalog in crates/gk-api and run `cargo xtask gen` again.

/* eslint-disable */

/** One changed item in the activity log. */
export type ActivityEntry = {
  /** Who. */
  actor: Actor;
  /** When (RFC 3339, UTC). */
  at: string;
  /** What kind of item changed. */
  entity: string;
  /** The changed fields, each with its value before and after. */
  fields: Record<string, FieldChange>;
  /** Its history group: everything one command changed. */
  group: string;
  /** The item's ID. */
  id: string;
  /** The operation (`settings.update`). */
  op: string;
  /** The project of the changed item, if any. */
  project?: string;
  /** The group that reverted this change, if one did. */
  reverted_by?: string;
  /** The change's sequence number. */
  seq: number;
  /** The agent session, for changes through MCP. */
  session?: string;
  /** Through which program. */
  via: Via;
};

/** A page of the activity log, newest first. */
export type ActivityPage = {
  /** The changes. */
  items: ActivityEntry[];
  /** Pass as `cursor` for the next page; left out on the last page. */
  next_cursor?: string;
  /** The agent sessions, newest first, when asked for. */
  sessions?: AgentSession[];
  /** How many changes match in all. */
  total: number;
};

/** Which changes to list. */
export type ActivityQuery = {
  /** Only changes by this actor, by name (case-insensitive). */
  actor?: string | null;
  /** `next_cursor` of the previous page. */
  cursor?: string | null;
  /** Only the changes of this history group. */
  group?: string | null;
  /** At most this many changes (default 50, at most 200). */
  limit?: number | null;
  /** Only changes of this agent session; `"current"` is the caller's own. */
  session?: string | null;
  /** Also list the agent sessions. */
  sessions?: boolean;
};

/** Who did something. */
export type Actor = {
  /** Person or agent. */
  kind: ActorKind;
  /** The display name ("You", "Claude Code"). */
  name: string;
};

/** A person or an agent. */
export type ActorKind = "user" | "agent";

/** The agent policy to set. */
export type AgentPolicyPatch = {
  /** The most an agent may do, whatever profile it asks for. */
  max_profile: Profile;
};

/** One run of `glyphkeel mcp`. */
export type AgentSession = {
  /** The client's own ID (`claude-code`), when it sent one. */
  client?: string;
  /** The client's version, when it sent one. */
  client_version?: string;
  /** When it ended; left out while it runs. */
  ended_at?: string;
  /** The session's ID. */
  id: string;
  /** When it was last seen working. */
  last_seen_at: string;
  /** The agent's display name. */
  name: string;
  /** The profile the session ran with. */
  profile: Profile;
  /** Operations that read. */
  reads: number;
  /** When it started (RFC 3339, UTC). */
  started_at: string;
  /** Operations that changed something. */
  writes: number;
};

/** Agent sessions, newest first. */
export type AgentSessionList = {
  /** The sessions. */
  items: AgentSession[];
  /** How many sessions match in all. */
  total: number;
};

/** Which sessions to list. */
export type AgentSessionsInput = {
  /** At most this many, newest first (default 50, at most 200). */
  limit?: number | null;
  /** Only sessions that haven't ended. */
  running_only?: boolean;
};

/** The caller of an operation as the engine sees it. */
export type Caller = {
  /** The person or the agent. */
  actor: Actor;
  /** The agent's profile, after the workspace's cap; left out for the person. */
  profile?: Profile;
  /** The agent session; left out for the person. */
  session?: string;
  /** The program. */
  via: Via;
};

/** One committed command. Events carry no content: a view re-reads what it shows, so there is one source of truth. */
export type Changed = {
  /** One entry per changed item. */
  changes: EntityChange[];
  /** The history group: what a revert or Undo names. */
  group: string;
  /** Who made the change, and through which program. */
  origin: Origin;
  /** The highest history sequence number of the command. */
  seq: number;
};

/** One changed item. */
export type EntityChange = {
  /** What kind of item (`task`, `project`, `settings`, …). */
  entity: string;
  /** The fields that changed. */
  fields: string[];
  /** Its ID. */
  id: string;
  /** The project it belongs to, if any. */
  project?: string;
};

/** What kind of failure an operation error is. Adapters decide on the code, never on the message. */
export type ErrorCode = "invalid" | "not_found" | "conflict" | "forbidden" | "limit" | "busy" | "workspace_upgraded" | "workspace_too_new" | "storage";

/** One field's change. A side is left out when the field had no value. */
export type FieldChange = {
  /** The value after. */
  after?: unknown;
  /** The value before. */
  before?: unknown;
};

/** An operation failed. The message says what happened, the hint what to do next; both are written for the reader, person or agent. */
export type OpError = {
  /** What kind of failure. */
  code: ErrorCode;
  /** Facts a program can act on: `path`, `choices`, `current_version`, `max`, `retry_after`, … */
  details?: Record<string, unknown>;
  /** What to do next. */
  hint?: string;
  /** What happened. */
  message: string;
};

/** Who made a change. */
export type Origin = {
  /** The person or the agent. */
  actor: Actor;
  /** The agent session, for changes made through MCP. */
  session?: string;
  /** The program: the app, the command line or MCP. */
  via: Via;
};

/** What an agent may do. The workspace setting "most an agent may do" caps the profile an agent asks for. */
export type Profile = "read-only" | "standard" | "full";

/** What to revert: history groups, or everything an agent session changed. */
export type RevertInput = {
  /** Only report what would be reverted. */
  dry_run?: boolean;
  /** History groups to revert. */
  groups?: string[];
  /** An agent session whose changes to revert. */
  session?: string | null;
};

/** What a revert did. */
export type RevertResult = {
  /** The history group of the revert itself; left out in a dry run or when nothing was reverted. */
  group?: string;
  /** The groups reverted (or that would be, in a dry run), newest first. */
  reverted: string[];
  /** Groups left alone, with the reason. */
  skipped: SkippedGroup[];
};

/** The workspace's settings, with defaults filled in. */
export type Settings = {
  /** The most an agent may do, whatever profile it asks for. */
  agent_max_profile: Profile;
  /** Working days in a week, for durations written in weeks (`1w`). */
  days_per_week: number;
  /** Working hours in a day, for durations written in days (`2d`). */
  hours_per_day: number;
  /** When reminders for a due date fire, as `HH:MM`. */
  reminder_time: string;
  /** Days an item stays in the trash before it is purged. */
  trash_days: number;
  /** The first day of the week in calendars and in `this-week`. */
  week_start: WeekStart;
};

/** The settings after a change. */
export type SettingsChange = {
  /** The settings whose value changed. */
  changed: string[];
  /** The history group of the change, for revert and Undo; left out when nothing changed. */
  group?: string;
  /** All settings, after the change. */
  settings: Settings;
};

/** No input. */
export type SettingsGetInput = Record<string, never>;

/** The settings to change; fields left out keep their value. Settings that control agents are changed in the app only. */
export type SettingsPatch = {
  /** Working days in a week, 1 to 7. */
  days_per_week?: number | null;
  /** Working hours in a day, 1 to 24. */
  hours_per_day?: number | null;
  /** Reminder time, `HH:MM` (24-hour). */
  reminder_time?: string | null;
  /** Days in the trash before purging, 1 to 3650. */
  trash_days?: number | null;
  /** First day of the week. */
  week_start?: WeekStart | null;
};

/** A group a revert left alone. */
export type SkippedGroup = {
  /** The group. */
  group: string;
  /** Why: something changed it since, it was already reverted, and so on. */
  reason: string;
};

/** No input. */
export type UndoInput = Record<string, never>;

/** What Undo or Redo did. */
export type UndoResult = {
  /** The history group of the Undo or Redo itself. */
  group: string;
  /** The operation that was undone or redone. */
  op: string;
  /** The group that was undone or redone. */
  target: string;
};

/** The program a change came through. */
export type Via = "app" | "cli" | "mcp";

/** The first day of the week. */
export type WeekStart = "monday" | "saturday" | "sunday";

/** No input. */
export type WorkspaceGetInput = Record<string, never>;

/** The open workspace and who is asking. */
export type WorkspaceInfo = {
  /** Who is asking, through which program, with which rights. */
  caller: Caller;
  /** A short guide to keys, references and the query language. */
  guide: string;
  /** The workspace's ID. */
  id: string;
  /** The workspace's name (its folder name). */
  name: string;
  /** The version of the program answering. */
  program_version: string;
  /** The schema version of its database. */
  schema_version: number;
  /** The workspace settings. */
  settings: Settings;
};

/** An error from the IPC layer itself, before the engine saw the call. */
export interface IpcError {
  code: "ipc";
  message: string;
}

/** Every error a call can end with. */
export type CallError = OpError | IpcError;

/** Every operation: its input, its output, its safety class and kind. */
export interface OpMap {
  "workspace.get": { input: WorkspaceGetInput; output: WorkspaceInfo; safety: "read"; kind: "query" };
  "settings.get": { input: SettingsGetInput; output: Settings; safety: "read"; kind: "query" };
  "settings.update": { input: SettingsPatch; output: SettingsChange; safety: "write"; kind: "command" };
  "agent.sessions": { input: AgentSessionsInput; output: AgentSessionList; safety: "read"; kind: "query" };
  "agent.policy.update": { input: AgentPolicyPatch; output: SettingsChange; safety: "app"; kind: "command" };
  "history.feed": { input: ActivityQuery; output: ActivityPage; safety: "read"; kind: "query" };
  "history.revert": { input: RevertInput; output: RevertResult; safety: "write"; kind: "command" };
  "history.undo": { input: UndoInput; output: UndoResult; safety: "app"; kind: "command" };
  "history.redo": { input: UndoInput; output: UndoResult; safety: "app"; kind: "command" };
}

/** The name of an operation. */
export type OpId = keyof OpMap;

/** The safety class of every operation. */
export const OP_SAFETY = {
  "workspace.get": "read",
  "settings.get": "read",
  "settings.update": "write",
  "agent.sessions": "read",
  "agent.policy.update": "app",
  "history.feed": "read",
  "history.revert": "write",
  "history.undo": "app",
  "history.redo": "app",
} as const satisfies { [K in OpId]: OpMap[K]["safety"] };

/** Sends one call; the transport (Tauri IPC or the development bridge) supplies it. */
export type Call = <K extends OpId>(op: K, input: OpMap[K]["input"]) => Promise<OpMap[K]["output"]>;

/** One typed function per operation, over any transport. */
export function makeOps(call: Call) {
  return {
    agent: {
      /** List agent sessions, newest first, with their client, profile, times and counts of reads and writes. (read) */
      sessions: (input: OpMap["agent.sessions"]["input"] = {}) => call("agent.sessions", input),
      /** Set the most an agent may do, whatever profile it asks for. (app) */
      policyUpdate: (input: OpMap["agent.policy.update"]["input"]) => call("agent.policy.update", input),
    },
    history: {
      /** The activity log, newest first: who changed what, when and through which program; optionally the agent sessions. (read) */
      feed: (input: OpMap["history.feed"]["input"] = {}) => call("history.feed", input),
      /** Revert history groups or everything an agent session changed. Groups that something changed since are skipped and listed. (write) */
      revert: (input: OpMap["history.revert"]["input"] = {}) => call("history.revert", input),
      /** Undo the last change made in this run of the app. (app) */
      undo: (input: OpMap["history.undo"]["input"] = {}) => call("history.undo", input),
      /** Redo the change the last Undo took back. (app) */
      redo: (input: OpMap["history.redo"]["input"] = {}) => call("history.redo", input),
    },
    settings: {
      /** Read the workspace settings. (read) */
      get: (input: OpMap["settings.get"]["input"] = {}) => call("settings.get", input),
      /** Change workspace settings that are not about agents: trash retention, reminder time, working hours and days, first day of the week. (write) */
      update: (input: OpMap["settings.update"]["input"] = {}) => call("settings.update", input),
    },
    workspace: {
      /** The open workspace: its name and ID, the schema and program versions, who is asking with which rights, the settings and a short guide. (read) */
      get: (input: OpMap["workspace.get"]["input"] = {}) => call("workspace.get", input),
    },
  };
}

/** The typed operations. */
export type Ops = ReturnType<typeof makeOps>;
