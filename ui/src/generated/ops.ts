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

/** A project's board as the person sees it. */
export type Board = {
  /** The columns, in the layout's order, hidden ones included and flagged. */
  columns: Column[];
  /** Hidden columns and the tasks in them. */
  hidden_columns: number;
  /** Tasks in hidden columns. */
  hidden_tasks: number;
  /** The layout the board was built with. */
  layout: BoardLayout;
  /** The project's key. */
  project: string;
};

/** Which board to read. */
export type BoardGetInput = {
  /** Show the sub-projects' tasks too; default: as the layout says. */
  include_subprojects?: boolean | null;
  /** The project: its key (current or old) or ID. */
  project: string;
  /** Show every closed card, however old. */
  show_older?: boolean;
};

/** The person's layout of one project's board. */
export type BoardLayout = {
  /** The fields shown on cards. */
  card_fields: string[];
  /** Collapsed columns, as status IDs. */
  collapsed: string[];
  /** Sort modes of single columns, by status ID. */
  column_sort: Record<string, SortMode>;
  /** Hidden columns, as status IDs. */
  hidden: string[];
  /** Cards closed more than this many days ago are hidden behind "Show older"; 0 shows every card. */
  hide_done_after_days: number;
  /** Whether the sub-projects' tasks are on the board too. */
  include_subprojects: boolean;
  /** The column order, as status IDs. Statuses not listed keep their project position between their neighbours. */
  order: string[];
  /** The sort mode of every column without its own. */
  sort_mode: SortMode;
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

/** One card. */
export type Card = {
  /** The description's checklist. */
  checklist: Checklist;
  /** When it was closed; left out while open. */
  closed_at?: string;
  /** The due date. */
  due_date?: string;
  /** The estimate in minutes. */
  estimate_minutes?: number;
  /** The task's ID. */
  id: string;
  /** The task's key. */
  key: string;
  /** Its labels. */
  labels: LabelRef[];
  /** The parent task's key, for a subtask. */
  parent?: string;
  /** Its priority. */
  priority: Priority;
  /** Its project's key (differs from the board's for sub-project tasks). */
  project: string;
  /** Its place in the column. */
  rank: string;
  /** The start date. */
  start_date?: string;
  /** When it entered its status. */
  status_entered_at: string;
  /** Its subtasks: closed and all. */
  subtasks: Checklist;
  /** Its title. */
  title: string;
  /** Its type. */
  type: TypeRef;
  /** When anything about it last changed. */
  updated_at: string;
  /** Its version, for `expected_version`. */
  version: number;
};

/** What a status means for the work in it. */
export type Category = "todo" | "in_progress" | "done" | "cancelled";

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

/** Ticked and all checklist items. */
export type Checklist = {
  /** Ticked items. */
  done: number;
  /** All items. */
  total: number;
};

/** One column: a status, or with sub-projects, every status of that name. */
export type Column = {
  /** The cards, in order. */
  cards: Card[];
  /** Its category. */
  category: Category;
  /** Collapsed by the layout. */
  collapsed: boolean;
  /** Tasks in the column, including hidden older ones. */
  count: number;
  /** Hidden by the layout; its cards are left out. */
  hidden: boolean;
  /** Its colour. */
  hue: Hue;
  /** The status's ID in the board's own project; a status that exists only in sub-projects has the first sub-project's. */
  id: string;
  /** The status name. */
  name: string;
  /** Closed cards hidden behind "Show older". */
  older: number;
  /** Whether the column holds more tasks than its WIP limit. */
  over_limit: boolean;
  /** How its cards are ordered. */
  sort_mode: SortMode;
  /** Every status shown in this column, by project key: one, or more when sub-projects are included. */
  statuses: Record<string, string>;
  /** The WIP limit, if the status has one. */
  wip_limit?: number;
};

/** The two ends of an ordered list. */
export type Edge = "top" | "bottom";

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

/** The shape that shows a task's type. */
export type Glyph = "task" | "bug" | "feature" | "milestone";

/** The design's nine colours, for projects, statuses, types and labels. */
export type Hue = "red" | "orange" | "yellow" | "green" | "teal" | "blue" | "purple" | "pink" | "grey";

/** A label. */
export type Label = {
  /** Its colour. */
  hue: Hue;
  /** The label's ID. */
  id: string;
  /** Its name. */
  name: string;
  /** Tasks with this label. */
  tasks: number;
};

/** Adds, changes or removes a label inside [`ProjectUpdateInput`]. */
export type LabelChange = {
  /** Its colour. */
  hue?: Hue | null;
  /** The label to change or remove; left out to add one. */
  label?: string | null;
  /** Its (new) name. */
  name?: string | null;
  /** Delete the label and take it off every task. Can't be undone. */
  remove?: boolean;
};

/** A new label. */
export type LabelCreateInput = {
  /** Its colour; default: grey. */
  hue?: Hue | null;
  /** The name, 1 to 50 characters, unique in the project ignoring case. */
  name: string;
  /** The project: its key (current or old) or ID. */
  project: string;
};

/** A label to delete. */
export type LabelDeleteInput = {
  /** The label: its name or ID. It is taken off every task. */
  label: string;
  /** The project: its key (current or old) or ID. */
  project: string;
};

/** A label on a task. */
export type LabelRef = {
  /** Its colour. */
  hue: Hue;
  /** The label's ID. */
  id: string;
  /** Its name. */
  name: string;
};

/** Changes to a label; fields left out keep their value. */
export type LabelUpdateInput = {
  /** A new colour. */
  hue?: Hue | null;
  /** The label: its name or ID. */
  label: string;
  /** A new name. */
  name?: string | null;
  /** The project: its key (current or old) or ID. */
  project: string;
};

/** A layout after a change. */
export type LayoutChange = {
  /** The history group; left out when nothing changed. */
  group?: string;
  /** The layout after the change. */
  layout: BoardLayout;
  /** The project's key. */
  project: string;
};

/** Which layout to read. */
export type LayoutGetInput = {
  /** The project: its key (current or old) or ID. */
  project: string;
};

/** Changes to a board layout; fields left out keep their value. */
export type LayoutUpdateInput = {
  /** The fields shown on cards. */
  card_fields?: string[] | null;
  /** Columns to collapse. */
  collapse?: string[];
  /** Sort modes of single columns, by status name or ID; `manual` columns can be reordered by hand. */
  column_sort?: Record<string, SortMode> | null;
  /** Columns to expand again. */
  expand?: string[];
  /** Columns to hide, by status name or ID. */
  hide?: string[];
  /** Hide cards closed more than this many days ago; 0 shows all. */
  hide_done_after_days?: number | null;
  /** Show the sub-projects' tasks too. */
  include_subprojects?: boolean | null;
  /** The column order, by status name or ID; columns left out follow in project order. */
  order?: string[] | null;
  /** The project: its key (current or old) or ID. */
  project: string;
  /** Go back to the project's layout first: every column shown and expanded, in project order, sorted by hand. */
  reset?: boolean;
  /** Columns to show again. */
  show?: string[];
  /** The sort mode of every column without its own. */
  sort_mode?: SortMode | null;
};

/** Which parts of a project are switched on. */
export type Modules = {
  /** The board. */
  board: boolean;
  /** The Gantt chart. */
  gantt: boolean;
  /** The task list. */
  list: boolean;
  /** Time tracking. */
  time: boolean;
  /** The wiki. */
  wiki: boolean;
};

/** Modules to switch on (`true`) or off (`false`); those left out stay as they are. */
export type ModulesPatch = {
  /** The board. */
  board?: boolean | null;
  /** The Gantt chart. */
  gantt?: boolean | null;
  /** The task list. */
  list?: boolean | null;
  /** Time tracking. */
  time?: boolean | null;
  /** The wiki. */
  wiki?: boolean | null;
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

/** A task's priority. */
export type Priority = "none" | "low" | "normal" | "high" | "urgent";

/** What an agent may do. The workspace setting "most an agent may do" caps the profile an agent asks for. */
export type Profile = "read-only" | "standard" | "full";

/** One project with its configuration. */
export type Project = {
  /** The sub-projects' keys, in order. */
  children: string[];
  /** When it was created (RFC 3339, UTC). */
  created_at: string;
  /** Its description (Markdown). */
  description: string;
  /** Its colour. */
  hue: Hue;
  /** Its icon, if one was chosen. */
  icon?: string;
  /** The project's ID. */
  id: string;
  /** Its key (`TM`). */
  key: string;
  /** The labels, in order. */
  labels: Label[];
  /** Which parts are switched on. */
  modules: Modules;
  /** Its name. */
  name: string;
  /** Keys it had before; they still find it. */
  old_keys?: string[];
  /** Open tasks in this project alone. */
  open_tasks: number;
  /** The parent's key; left out for a top-level project. */
  parent?: string;
  /** The names from the top-level project down to this one, joined by " / ". */
  path: string;
  /** Active or archived. */
  state: ProjectState;
  /** The statuses, in board order. */
  statuses: Status[];
  /** All tasks in this project alone, open or closed (not in the trash). */
  total_tasks: number;
  /** The task types, in order. */
  types: TaskType[];
  /** When it last changed (RFC 3339, UTC). */
  updated_at: string;
};

/** A project changed, with the history group of the change. */
export type ProjectChange = {
  /** The history group, for revert and Undo; left out when nothing changed. */
  group?: string;
  /** The project after the change. */
  project: Project;
};

/** A new project or sub-project. */
export type ProjectCreateInput = {
  /** Your own ID for this request; repeating it within 24 hours returns the first answer instead of creating a second project. */
  client_request_id?: string | null;
  /** The description (Markdown). */
  description?: string | null;
  /** The colour; default: the parent's, or blue. */
  hue?: Hue | null;
  /** An icon name. */
  icon?: string | null;
  /** The key: 2 to 10 capital letters and digits, starting with a letter (`TM`). Task keys start with it. */
  key: string;
  /** The name, 1 to 100 characters. */
  name: string;
  /** The parent, for a sub-project (at most 3 levels). It copies the parent's statuses, types and labels. */
  parent?: string | null;
};

/** A project field that can be cleared. */
export type ProjectField = "icon" | "description" | "parent";

/** The project tree, in sidebar order: each project followed by its sub-projects. */
export type ProjectList = {
  /** The projects. */
  items: ProjectSummary[];
  /** How many there are. */
  total: number;
};

/** Which projects to list. */
export type ProjectListInput = {
  /** Also list archived projects. */
  include_archived?: boolean;
};

/** Where to move a project in the tree. */
export type ProjectMoveInput = {
  /** Place it after this sibling project. */
  after?: string | null;
  /** Place it first or last among its siblings. */
  at?: Edge | null;
  /** Place it before this sibling project. */
  before?: string | null;
  /** The new parent; with `top_level`, none. */
  parent?: string | null;
  /** The project: its key (current or old) or ID. */
  project: string;
  /** Make it a top-level project. */
  top_level?: boolean;
};

/** Names one project. */
export type ProjectRef = {
  /** The project: its key (current or old) or ID. */
  project: string;
};

/** A project's new key. */
export type ProjectRekeyInput = {
  /** The new key: 2 to 10 capital letters and digits, starting with a letter. The old key keeps finding the project and its tasks. */
  key: string;
  /** The project: its key (current or old) or ID. */
  project: string;
};

/** Whether a project can be changed. */
export type ProjectState = "active" | "archived";

/** One project in the tree. */
export type ProjectSummary = {
  /** 0 for a top-level project, 1 for its sub-projects, 2 below them. */
  depth: number;
  /** Its colour. */
  hue: Hue;
  /** Its icon, if one was chosen. */
  icon?: string;
  /** The project's ID. */
  id: string;
  /** The project's key (`TM`). */
  key: string;
  /** Its name. */
  name: string;
  /** Open tasks in this project alone. */
  open_tasks: number;
  /** Open tasks in this project and all its sub-projects. */
  open_tasks_with_subprojects: number;
  /** The parent's key; left out for a top-level project. */
  parent?: string;
  /** The names from the top-level project down to this one, joined by " / ". */
  path: string;
  /** Active or archived. */
  state: ProjectState;
};

/** Changes to a project; fields left out keep their value. Everything is one change with one history group. Removing a status, type or label can't be undone, so it needs the `permanent` class (the person confirms it; an agent needs the full profile). */
export type ProjectUpdateInput = {
  /** Place it after this sibling project. */
  after?: string | null;
  /** Archive (`true`) or unarchive (`false`) it with its sub-projects. */
  archived?: boolean | null;
  /** Place it first or last among its siblings. */
  at?: Edge | null;
  /** Place it before this sibling project. */
  before?: string | null;
  /** Fields to clear. */
  clear?: ProjectField[];
  /** A new description (Markdown). */
  description?: string | null;
  /** A new colour. */
  hue?: Hue | null;
  /** A new icon name. */
  icon?: string | null;
  /** A new key; the old one keeps finding the project and its tasks. */
  key?: string | null;
  /** Labels to add, change or remove, in order. */
  labels?: LabelChange[];
  /** Modules to switch on or off. */
  modules?: ModulesPatch | null;
  /** A new name, 1 to 100 characters. */
  name?: string | null;
  /** Move it under this project (at most 3 levels, never under its own sub-project). To make it a top-level project, list `parent` in `clear`. */
  parent?: string | null;
  /** The project: its key (current or old) or ID. */
  project: string;
  /** Statuses to add, change, reorder or remove, in order. */
  statuses?: StatusChange[];
  /** Task types to add, change, reorder or remove, in order. */
  types?: TypeChange[];
};

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

/** A setting that can be set back to its default. */
export type SettingKey = "trash_days" | "reminder_time" | "hours_per_day" | "days_per_week" | "week_start";

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
  /** Settings to set back to their default. */
  clear?: SettingKey[];
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

/** How a column's cards are ordered. */
export type SortMode = "manual" | "priority" | "due" | "updated";

/** A status: one board column. */
export type Status = {
  /** What it means for the work in it. */
  category: Category;
  /** Its colour. */
  hue: Hue;
  /** The status's ID. */
  id: string;
  /** Its name. */
  name: string;
  /** Tasks in this status. */
  tasks: number;
  /** Most tasks the column should hold; left out when there is no limit. */
  wip_limit?: number;
};

/** Changes to a status, a task type or a label inside [`ProjectUpdateInput`]. Without `status`/`type`/`label` (`name`) it adds one; with `remove` it deletes the named one; otherwise it changes it. */
export type StatusChange = {
  /** Place it after this status. */
  after?: string | null;
  /** Place it first or last. */
  at?: Edge | null;
  /** Place it before this status. */
  before?: string | null;
  /** Its category. */
  category?: Category | null;
  /** Fields to clear. */
  clear?: StatusField[];
  /** Its colour. */
  hue?: Hue | null;
  /** When removing a status that has tasks: the status they move to. */
  move_tasks_to?: string | null;
  /** Its (new) name. */
  name?: string | null;
  /** Delete the status. Can't be undone. */
  remove?: boolean;
  /** The status to change or remove; left out to add one. */
  status?: string | null;
  /** Its WIP limit, 1 or more. */
  wip_limit?: number | null;
};

/** A new status. */
export type StatusCreateInput = {
  /** Place it after this status. */
  after?: string | null;
  /** Place it first or last; default: last. */
  at?: Edge | null;
  /** Place it before this status. */
  before?: string | null;
  /** Its category; default: to do. */
  category?: Category | null;
  /** Its colour; default: grey. */
  hue?: Hue | null;
  /** The name, 1 to 50 characters, unique in the project ignoring case. */
  name: string;
  /** The project: its key (current or old) or ID. */
  project: string;
  /** Its WIP limit, 1 or more. */
  wip_limit?: number | null;
};

/** A status to delete. */
export type StatusDeleteInput = {
  /** The status its tasks move to; needed when it has tasks. */
  move_tasks_to?: string | null;
  /** The project: its key (current or old) or ID. */
  project: string;
  /** The status: its name or ID. */
  status: string;
};

/** A status field that can be cleared. */
export type StatusField = "wip_limit";

/** A task's status, as tasks carry it. */
export type StatusRef = {
  /** Its category. */
  category: Category;
  /** The status's ID. */
  id: string;
  /** Its name. */
  name: string;
};

/** Where to move a status, a task type or a label in its list. */
export type StatusReorderInput = {
  /** Place it after this status. */
  after?: string | null;
  /** Place it first or last. */
  at?: Edge | null;
  /** Place it before this status. */
  before?: string | null;
  /** The project: its key (current or old) or ID. */
  project: string;
  /** The status: its name or ID. */
  status: string;
};

/** Changes to a status; fields left out keep their value. */
export type StatusUpdateInput = {
  /** A new category. Tasks in it close or reopen with it. */
  category?: Category | null;
  /** Fields to clear. */
  clear?: StatusField[];
  /** A new colour. */
  hue?: Hue | null;
  /** A new name. */
  name?: string | null;
  /** The project: its key (current or old) or ID. */
  project: string;
  /** The status: its name or ID. */
  status: string;
  /** A new WIP limit, 1 or more. */
  wip_limit?: number | null;
};

/** One task. */
export type Task = {
  /** The description's checklist. */
  checklist: Checklist;
  /** When it was closed; left out while it is open. */
  closed_at?: string;
  /** When it was created (RFC 3339, UTC). */
  created_at: string;
  /** Who created it. */
  created_by: Actor;
  /** Its description (Markdown). */
  description: string;
  /** The due date. */
  due_date?: string;
  /** When its own fields last changed. */
  edited_at: string;
  /** The estimate, as a duration (`1d 4h`). */
  estimate?: string;
  /** The estimate in minutes. */
  estimate_minutes?: number;
  /** The task's ID. */
  id: string;
  /** The task's key (`TM-41`). */
  key: string;
  /** Its labels, in the project's order. */
  labels: LabelRef[];
  /** Keys it had before; they still find it. */
  old_keys?: string[];
  /** The parent task's key, for a subtask. */
  parent?: string;
  /** Its priority. */
  priority: Priority;
  /** Progress set by hand, 0 to 100. */
  progress?: number;
  /** Its project's key. */
  project: string;
  /** Its place in its status column. */
  rank: string;
  /** Logged time in minutes. */
  spent_minutes: number;
  /** The start date. */
  start_date?: string;
  /** Its status. */
  status: StatusRef;
  /** When it entered its status. */
  status_entered_at: string;
  /** Its title. */
  title: string;
  /** Its type. */
  type: TypeRef;
  /** When anything about it last changed. */
  updated_at: string;
  /** Goes up by one with every change; pass it as `expected_version` to make sure nobody changed the task since you read it. */
  version: number;
};

/** A task changed, with the history group of the change. */
export type TaskChange = {
  /** The history group, for revert and Undo; left out when nothing changed. */
  group?: string;
  /** The task after the change. */
  task: Task;
  /** Problems that did not stop the change, such as closing a task whose subtasks are still open. */
  warnings?: string[];
};

/** A new task. Only the project and the title are needed. */
export type TaskCreateInput = {
  /** Place it after this task of the same status. */
  after?: string | null;
  /** Place it first or last in its status; default: last. */
  at?: Edge | null;
  /** Place it before this task of the same status. */
  before?: string | null;
  /** Your own ID for this request; repeating it within 24 hours returns the first answer instead of creating a second task. */
  client_request_id?: string | null;
  /** The description (Markdown). */
  description?: string | null;
  /** The due date, `YYYY-MM-DD`. */
  due_date?: string | null;
  /** The estimate, as a duration (`1d 4h`, `90m`, `1.5h`). */
  estimate?: string | null;
  /** Labels, by name or ID. */
  labels?: string[];
  /** The parent task, for a subtask (same project, at most 5 levels). */
  parent?: string | null;
  /** The priority; default: none. */
  priority?: Priority | null;
  /** The project: its key (current or old) or ID. */
  project: string;
  /** The start date, `YYYY-MM-DD`. */
  start_date?: string | null;
  /** The status; default: the project's first to-do status. */
  status?: string | null;
  /** The title, 1 to 500 characters; tabs and line breaks become spaces. */
  title: string;
  /** The type; default: the project's default type. */
  type?: string | null;
};

/** A task field that can be cleared. */
export type TaskField = "description" | "parent" | "start_date" | "due_date" | "estimate" | "progress" | "labels";

/** Which tasks to read. */
export type TaskGetInput = {
  /** One to 20 task keys or IDs. */
  keys: string[];
};

/** The tasks asked for, in the order asked. */
export type TaskList = {
  /** The tasks. */
  items: Task[];
  /** How many. */
  total: number;
};

/** Where to move a task: another status, another place in its column, or another project. */
export type TaskMoveInput = {
  /** Place it after this task of the target status. */
  after?: string | null;
  /** Place it first or last in the target status; default when the status changes: last. */
  at?: Edge | null;
  /** Place it before this task of the target status. */
  before?: string | null;
  /** Refuse the move unless the task still has this version. */
  expected_version?: number | null;
  /** The task: its key or ID. */
  key: string;
  /** Move it to another project: it gets the next key there, and the old key keeps finding it. Its subtasks move with it. */
  project?: string | null;
  /** The new status, by name or ID. */
  status?: string | null;
};

/** A task type. */
export type TaskType = {
  /** Its shape. */
  glyph: Glyph;
  /** Its colour. */
  hue: Hue;
  /** The type's ID. */
  id: string;
  /** Whether new tasks get it unless told otherwise. */
  is_default: boolean;
  /** Its name. */
  name: string;
  /** Tasks of this type. */
  tasks: number;
};

/** Changes to a task; fields left out keep their value. Status and position change with `task.move`. */
export type TaskUpdateInput = {
  /** Labels to add. */
  add_labels?: string[];
  /** Fields to clear. */
  clear?: TaskField[];
  /** A new description (Markdown), replacing the old one. */
  description?: string | null;
  /** A new due date, `YYYY-MM-DD`. */
  due_date?: string | null;
  /** A new estimate, as a duration. */
  estimate?: string | null;
  /** Refuse the change unless the task still has this version. */
  expected_version?: number | null;
  /** The task: its key or ID. */
  key: string;
  /** Replace the labels with these. */
  labels?: string[] | null;
  /** A new parent task (same project, at most 5 levels, no loops). */
  parent?: string | null;
  /** A new priority. */
  priority?: Priority | null;
  /** Progress set by hand, 0 to 100. */
  progress?: number | null;
  /** Labels to take off. */
  remove_labels?: string[];
  /** A new start date, `YYYY-MM-DD`. */
  start_date?: string | null;
  /** A new title. */
  title?: string | null;
  /** A new type. */
  type?: string | null;
};

/** Adds, changes or removes a task type inside [`ProjectUpdateInput`]. */
export type TypeChange = {
  /** Place it after this type. */
  after?: string | null;
  /** Place it first or last. */
  at?: Edge | null;
  /** Place it before this type. */
  before?: string | null;
  /** Its shape. */
  glyph?: Glyph | null;
  /** Its colour. */
  hue?: Hue | null;
  /** Make it the type new tasks get. */
  make_default?: boolean;
  /** When removing a type that tasks have: the type they get instead; default: the default type. */
  move_tasks_to?: string | null;
  /** Its (new) name. */
  name?: string | null;
  /** Delete the type. Can't be undone. */
  remove?: boolean;
  /** The type to change or remove; left out to add one. */
  type?: string | null;
};

/** A new task type. */
export type TypeCreateInput = {
  /** Its shape; default: task. */
  glyph?: Glyph | null;
  /** Its colour; default: grey. */
  hue?: Hue | null;
  /** Make it the type new tasks get. */
  make_default?: boolean;
  /** The name, 1 to 50 characters, unique in the project ignoring case. */
  name: string;
  /** The project: its key (current or old) or ID. */
  project: string;
};

/** A task type to delete. */
export type TypeDeleteInput = {
  /** The type its tasks get instead; default: the default type. */
  move_tasks_to?: string | null;
  /** The project: its key (current or old) or ID. */
  project: string;
  /** The type: its name or ID. The default type can't be deleted. */
  type: string;
};

/** A task's type, as tasks carry it. */
export type TypeRef = {
  /** Its shape. */
  glyph: Glyph;
  /** The type's ID. */
  id: string;
  /** Its name. */
  name: string;
};

/** Changes to a task type; fields left out keep their value. */
export type TypeUpdateInput = {
  /** Place it after this type. */
  after?: string | null;
  /** Place it first or last. */
  at?: Edge | null;
  /** Place it before this type. */
  before?: string | null;
  /** A new shape. */
  glyph?: Glyph | null;
  /** A new colour. */
  hue?: Hue | null;
  /** Make it the type new tasks get. */
  make_default?: boolean;
  /** A new name. */
  name?: string | null;
  /** The project: its key (current or old) or ID. */
  project: string;
  /** The type: its name or ID. */
  type: string;
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
  "project.list": { input: ProjectListInput; output: ProjectList; safety: "read"; kind: "query" };
  "project.get": { input: ProjectRef; output: Project; safety: "read"; kind: "query" };
  "project.create": { input: ProjectCreateInput; output: ProjectChange; safety: "write"; kind: "command" };
  "project.update": { input: ProjectUpdateInput; output: ProjectChange; safety: "write"; kind: "command" };
  "project.move": { input: ProjectMoveInput; output: ProjectChange; safety: "write"; kind: "command" };
  "project.change_key": { input: ProjectRekeyInput; output: ProjectChange; safety: "write"; kind: "command" };
  "project.archive": { input: ProjectRef; output: ProjectChange; safety: "write"; kind: "command" };
  "project.unarchive": { input: ProjectRef; output: ProjectChange; safety: "write"; kind: "command" };
  "status.create": { input: StatusCreateInput; output: ProjectChange; safety: "write"; kind: "command" };
  "status.update": { input: StatusUpdateInput; output: ProjectChange; safety: "write"; kind: "command" };
  "status.reorder": { input: StatusReorderInput; output: ProjectChange; safety: "write"; kind: "command" };
  "status.delete": { input: StatusDeleteInput; output: ProjectChange; safety: "permanent"; kind: "command" };
  "type.create": { input: TypeCreateInput; output: ProjectChange; safety: "write"; kind: "command" };
  "type.update": { input: TypeUpdateInput; output: ProjectChange; safety: "write"; kind: "command" };
  "type.delete": { input: TypeDeleteInput; output: ProjectChange; safety: "permanent"; kind: "command" };
  "label.create": { input: LabelCreateInput; output: ProjectChange; safety: "write"; kind: "command" };
  "label.update": { input: LabelUpdateInput; output: ProjectChange; safety: "write"; kind: "command" };
  "label.delete": { input: LabelDeleteInput; output: ProjectChange; safety: "permanent"; kind: "command" };
  "task.get": { input: TaskGetInput; output: TaskList; safety: "read"; kind: "query" };
  "task.create": { input: TaskCreateInput; output: TaskChange; safety: "write"; kind: "command" };
  "task.update": { input: TaskUpdateInput; output: TaskChange; safety: "write"; kind: "command" };
  "task.move": { input: TaskMoveInput; output: TaskChange; safety: "write"; kind: "command" };
  "board.get": { input: BoardGetInput; output: Board; safety: "read"; kind: "query" };
  "layout.get": { input: LayoutGetInput; output: LayoutChange; safety: "read"; kind: "query" };
  "layout.update": { input: LayoutUpdateInput; output: LayoutChange; safety: "write"; kind: "command" };
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
  "project.list": "read",
  "project.get": "read",
  "project.create": "write",
  "project.update": "write",
  "project.move": "write",
  "project.change_key": "write",
  "project.archive": "write",
  "project.unarchive": "write",
  "status.create": "write",
  "status.update": "write",
  "status.reorder": "write",
  "status.delete": "permanent",
  "type.create": "write",
  "type.update": "write",
  "type.delete": "permanent",
  "label.create": "write",
  "label.update": "write",
  "label.delete": "permanent",
  "task.get": "read",
  "task.create": "write",
  "task.update": "write",
  "task.move": "write",
  "board.get": "read",
  "layout.get": "read",
  "layout.update": "write",
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
    board: {
      /** A project's board as the person sees it: columns in the personal order with hidden ones flagged, WIP state and compact cards. (read) */
      get: (input: OpMap["board.get"]["input"]) => call("board.get", input),
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
    label: {
      /** Add a label to a project. (write) */
      create: (input: OpMap["label.create"]["input"]) => call("label.create", input),
      /** Rename or recolour a label. (write) */
      update: (input: OpMap["label.update"]["input"]) => call("label.update", input),
      /** Delete a label and take it off every task. Can't be undone. (permanent) */
      delete: (input: OpMap["label.delete"]["input"]) => call("label.delete", input),
    },
    layout: {
      /** The person's layout of a project's board: column order, hidden and collapsed columns, sort modes, card fields. (read) */
      get: (input: OpMap["layout.get"]["input"]) => call("layout.get", input),
      /** Hide, show, collapse or reorder board columns, set sort modes, card fields and older closed cards, or reset to the project's layout. (write) */
      update: (input: OpMap["layout.update"]["input"]) => call("layout.update", input),
    },
    project: {
      /** The project tree in sidebar order, with keys, names and open-task counts; archived projects on request. (read) */
      list: (input: OpMap["project.list"]["input"] = {}) => call("project.list", input),
      /** One project with its statuses (board order, category, WIP limit), task types, labels, modules, sub-projects and counts. (read) */
      get: (input: OpMap["project.get"]["input"]) => call("project.get", input),
      /** Create a project, or a sub-project that copies its parent's statuses, types and labels. (write) */
      create: (input: OpMap["project.create"]["input"]) => call("project.create", input),
      /** Change a project: name, description, colour, icon, modules, key, place in the tree, archived; add, change, reorder or remove statuses, types and labels, all in one change. (write) */
      update: (input: OpMap["project.update"]["input"]) => call("project.update", input),
      /** Move a project under another one (at most 3 levels), to the top level, or among its siblings. (write) */
      move: (input: OpMap["project.move"]["input"]) => call("project.move", input),
      /** Give a project a new key; the old key and the old task keys keep working. (write) */
      changeKey: (input: OpMap["project.change_key"]["input"]) => call("project.change_key", input),
      /** Archive a project and its sub-projects: they become read-only and leave the sidebar. (write) */
      archive: (input: OpMap["project.archive"]["input"]) => call("project.archive", input),
      /** Unarchive a project and its sub-projects. (write) */
      unarchive: (input: OpMap["project.unarchive"]["input"]) => call("project.unarchive", input),
    },
    settings: {
      /** Read the workspace settings. (read) */
      get: (input: OpMap["settings.get"]["input"] = {}) => call("settings.get", input),
      /** Change workspace settings that are not about agents: trash retention, reminder time, working hours and days, first day of the week. (write) */
      update: (input: OpMap["settings.update"]["input"] = {}) => call("settings.update", input),
    },
    status: {
      /** Add a status (a board column) to a project. (write) */
      create: (input: OpMap["status.create"]["input"]) => call("status.create", input),
      /** Rename or recolour a status, change its category or its WIP limit. (write) */
      update: (input: OpMap["status.update"]["input"]) => call("status.update", input),
      /** Move a status in the project's board order. (write) */
      reorder: (input: OpMap["status.reorder"]["input"]) => call("status.reorder", input),
      /** Delete a status; its tasks move to the status named. Can't be undone. (permanent) */
      delete: (input: OpMap["status.delete"]["input"]) => call("status.delete", input),
    },
    task: {
      /** One to 20 tasks by key, with their fields, description, labels and version. (read) */
      get: (input: OpMap["task.get"]["input"]) => call("task.get", input),
      /** Create a task from a title alone or with its fields; it gets the project's next key. (write) */
      create: (input: OpMap["task.create"]["input"]) => call("task.create", input),
      /** Change a task's title, description, type, priority, labels, parent, dates, estimate or progress. (write) */
      update: (input: OpMap["task.update"]["input"]) => call("task.update", input),
      /** Move a task to another status or place in its column, or to another project (new key; the old one forwards). (write) */
      move: (input: OpMap["task.move"]["input"]) => call("task.move", input),
    },
    type: {
      /** Add a task type to a project. (write) */
      create: (input: OpMap["type.create"]["input"]) => call("type.create", input),
      /** Rename, reshape, recolour or reorder a task type, or make it the default. (write) */
      update: (input: OpMap["type.update"]["input"]) => call("type.update", input),
      /** Delete a task type; its tasks get another type. Can't be undone. (permanent) */
      delete: (input: OpMap["type.delete"]["input"]) => call("type.delete", input),
    },
    workspace: {
      /** The open workspace: its name and ID, the schema and program versions, who is asking with which rights, the settings and a short guide. (read) */
      get: (input: OpMap["workspace.get"]["input"] = {}) => call("workspace.get", input),
    },
  };
}

/** The typed operations. */
export type Ops = ReturnType<typeof makeOps>;
