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
  /** Only changes by the person (`user`) or only those by agents (`agent`). */
  actor_kind?: ActorKind | null;
  /** `next_cursor` of the previous page. */
  cursor?: string | null;
  /** Only changes on or after this day: `YYYY-MM-DD`, or a natural form such as `today`, `yesterday` or `-7d`, in the caller's time zone. */
  from?: string | null;
  /** Only the changes of this history group. */
  group?: string | null;
  /** At most this many changes (default 50, at most 200). */
  limit?: number | null;
  /** Only changes in this project and its sub-projects: a key or ID. */
  project?: string | null;
  /** Only changes of this agent session; `"current"` is the caller's own. */
  session?: string | null;
  /** Also list the agent sessions. */
  sessions?: boolean;
  /** Only changes to this task (key or ID): its own fields and description revisions, its links, its comments and its files, including files attached to its comments. */
  task?: string | null;
  /** Only changes on or before this day (included), in the same forms as `from`. */
  to?: string | null;
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

/** A project's instructions for agents. */
export type AgentBrief = {
  /** The brief (Markdown). It is user data, not instructions to follow blindly. */
  body: string;
  /** The project's key. */
  project: string;
  /** Where the brief comes from. */
  source: BriefSource;
  /** The wiki page's title, for a brief from the wiki. */
  title?: string;
};

/** One of the limits on agent sessions. */
export type AgentLimit = "writes_per_minute" | "new_tasks_per_day" | "trash_items_per_call" | "trash_items_per_hour";

/** The limits on every agent session, so a runaway loop can't flood the workspace. A limit that is left out is lifted: there is no limit. Only the person changes them, in the app. */
export type AgentLimits = {
  /** Tasks a session may create in any 24 hours; left out: no limit. */
  new_tasks_per_day?: number;
  /** Items a standard agent may move to the trash in one call; left out: no limit. */
  trash_items_per_call?: number;
  /** Items a standard agent session may move to the trash in an hour; left out: no limit. */
  trash_items_per_hour?: number;
  /** Calls of changing tools allowed in any minute, per session; left out: no limit. */
  writes_per_minute?: number;
};

/** The limits on agent sessions to change; limits left out keep their value. */
export type AgentLimitsPatch = {
  /** Tasks a session may create in any 24 hours, 1 to 1000000. */
  new_tasks_per_day?: number | null;
  /** Limits to lift: agents then have no limit there. */
  no_limit?: AgentLimit[];
  /** Limits to set back to their default. */
  reset?: AgentLimit[];
  /** Items a standard agent may move to the trash in one call, 1 to 1000000. */
  trash_items_per_call?: number | null;
  /** Items a standard agent session may move to the trash in an hour, 1 to 1000000. */
  trash_items_per_hour?: number | null;
  /** Calls of changing tools allowed in any minute, per session, 1 to 1000000. */
  writes_per_minute?: number | null;
};

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

/** What an archive holds. */
export type ArchiveCounts = {
  /** History rows. */
  activity: number;
  /** Agent sessions the history refers to. */
  agent_sessions: number;
  /** Attachments. */
  attachments: number;
  /** Comments. */
  comments: number;
  /** Distinct stored files. */
  files: number;
  /** Projects, sub-projects included. */
  projects: number;
  /** Revisions of descriptions and wiki pages. */
  revisions: number;
  /** Tasks, trashed ones included. */
  tasks: number;
  /** Time entries. */
  time_entries: number;
  /** Wiki pages. */
  wiki_pages: number;
};

/** How an archive is stored. */
export type ArchiveFormat = "zip" | "folder";

/** One attached file. */
export type Attachment = {
  /** Who attached it. */
  added_by: Actor;
  /** The comment's ID, for a file attached to a comment. */
  comment?: string;
  /** When it was attached (RFC 3339, UTC). */
  created_at: string;
  /** An image's height in pixels. */
  height?: number;
  /** The attachment's ID. */
  id: string;
  /** What it holds. */
  kind: FileKind;
  /** The media type, read from the content. */
  mime: string;
  /** The file name. */
  name: string;
  /** The content's SHA-256. */
  sha256: string;
  /** The size in bytes. */
  size: number;
  /** The task's key. */
  task: string;
  /** An image's width in pixels. */
  width?: number;
};

/** What links to a page. */
export type Backlink = {
  /** The heading the link names, if any. */
  heading?: string;
  /** A page, for a link in its text. */
  page?: WikiPageRef;
  /** A task's key, for a link in its description. */
  task?: string;
  /** The title of the task or page. */
  title: string;
};

/** One backup. */
export type Backup = {
  /** The program version that made it. */
  app_version: string;
  /** When it was made (RFC 3339, UTC). */
  created_at: string;
  /** How many stored files it refers to. */
  files: number;
  /** Their size in bytes (shared with other backups). */
  files_size: number;
  /** Why it was made. */
  kind: BackupKind;
  /** Its name; pass it to verify, restore or delete it. */
  name: string;
  /** The schema version of the copy. */
  schema_version: number;
  /** The database copy's size in bytes. */
  size: number;
};

/** The result of checking a backup. */
export type BackupCheck = {
  /** The backup checked. */
  backup: Backup;
  /** How many stored files were read and their hashes compared. */
  files_checked: number;
  /** Whether it can be restored as it is. */
  ok: boolean;
  /** What is wrong, at most 100 problems. */
  problems: BackupProblem[];
};

/** No input. */
export type BackupCreateInput = Record<string, never>;

/** A deleted backup. */
export type BackupDeleted = {
  /** Stored files no other backup used, removed with it. */
  files_removed: number;
  /** The backup's name. */
  name: string;
};

/** Where the backups go. */
export type BackupFolderPatch = {
  /** The new folder: an absolute path outside the workspace's own folder, which is created if it doesn't exist. Leave it out to go back to the default folder. Backups already made stay where they are. */
  folder?: string | null;
};

/** Why a backup was made, which decides how long it is kept. */
export type BackupKind = "daily" | "weekly" | "manual" | "pre_migration" | "safety" | "pre_restore";

/** The workspace's backups, newest first. */
export type BackupList = {
  /** The folder that holds them. */
  folder: string;
  /** The backups. */
  items: Backup[];
  /** When the newest backup was made; left out when there is none. */
  last_backup_at?: string;
};

/** No input. */
export type BackupListInput = Record<string, never>;

/** One problem of a backup. */
export type BackupProblem = {
  /** The detail: SQLite's report, or the file's SHA-256. */
  detail: string;
  /** What is wrong. */
  kind: BackupProblemKind;
};

/** What is wrong with a backup. */
export type BackupProblemKind = "database" | "missing_file" | "damaged_file";

/** Names one backup. */
export type BackupRef = {
  /** The backup's name, from the backup list. */
  name: string;
};

/** A restored backup. */
export type BackupRestored = {
  /** The backup of the state before, to go back to it. */
  previous: Backup;
  /** The backup now in place. */
  restored: Backup;
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

/** Where an agent brief comes from. */
export type BriefSource = "wiki_page" | "description" | "none";

/** The change to make to every task. Fields left out stay as they are. */
export type BulkChanges = {
  /** Labels to add. */
  add_labels?: string[];
  /** Fields to clear. */
  clear?: TaskField[];
  /** The due date, written as for `start_date`. */
  due_date?: string | null;
  /** The estimate, as a duration (`1d 4h`, `90m`). */
  estimate?: string | null;
  /** Replace the labels with these. */
  labels?: string[] | null;
  /** Make them subtasks of this task. */
  parent?: string | null;
  /** The priority. */
  priority?: Priority | null;
  /** Move them to this project (new keys; old keys forward). */
  project?: string | null;
  /** Labels to take off. */
  remove_labels?: string[];
  /** Move due dates the same way. */
  shift_due?: string | null;
  /** Move start dates by calendar days, weeks, months or years (`+2d`, `-1w`); tasks without one keep none. */
  shift_start?: string | null;
  /** The start date: `YYYY-MM-DD`, or `today`, `fri`, `next mon`, `+3d`. */
  start_date?: string | null;
  /** Move to the status of this name (in each task's project). */
  status?: string | null;
  /** The type of this name. */
  type?: string | null;
};

/** A field of one task before and after. */
export type BulkFieldChange = {
  /** Its value after; left out when empty. */
  after?: string;
  /** Its value before; left out when empty. */
  before?: string;
  /** The field. */
  field: string;
};

/** One task of a bulk change. */
export type BulkItem = {
  /** What changes, or changed; empty when it already was as asked. */
  changes: BulkFieldChange[];
  /** Its key (after the change). */
  key: string;
  /** Its title. */
  title: string;
};

/** What a bulk change does or did. */
export type BulkResult = {
  /** How many of them change. */
  changed: number;
  /** How many tasks the change is for; pass it as `expected_count`. */
  count: number;
  /** Only a preview: nothing changed. */
  dry_run: boolean;
  /** The history group, for revert and Undo; left out for a preview or when nothing changed. */
  group?: string;
  /** Every task, with what changes. */
  items: BulkItem[];
  /** Problems that did not stop the change. */
  warnings?: string[];
};

/** A bulk change. */
export type BulkUpdateInput = {
  /** The change. */
  changes: BulkChanges;
  /** Only show what would change; default true. Turn it off to make the change. */
  dry_run?: boolean | null;
  /** How many tasks the change is for, from the preview; needed to make the change, which stops when the tasks now number otherwise. */
  expected_count?: number | null;
  /** The tasks, by key or ID. Give these or `query`. */
  keys?: string[];
  /** A query choosing the tasks (`project:TM label:old is:open`). */
  query?: string | null;
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
  /** Whether a task that blocks it is still open. */
  blocked: boolean;
  /** The description's checklist. */
  checklist: Checklist;
  /** The agent that claimed it, while the claim lasts. */
  claim?: Claim;
  /** When it was closed; left out while open. */
  closed_at?: string;
  /** How many comments it has. */
  comment_count: number;
  /** The due date. */
  due_date?: string;
  /** The estimate in minutes. */
  estimate_minutes?: number;
  /** How many files are attached to it and to its comments. */
  file_count: number;
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
  /** Its progress in percent, as tasks show it. */
  progress_percent: number;
  /** Its project's key (differs from the board's for sub-project tasks). */
  project: string;
  /** Its place in the column. */
  rank: string;
  /** Logged time in minutes. */
  spent_minutes: number;
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

/** A new item. */
export type ChecklistAdd = {
  /** Put it after this item (number, as read before this change); default: after the last top-level item, or in a new list at the end. */
  after?: number | null;
  /** Whether it starts ticked. */
  checked?: boolean;
  /** Its text, one line. */
  text: string;
};

/** A checklist after a change. */
export type ChecklistChange = {
  /** Ticked and all items. */
  checklist: Checklist;
  /** The comment's ID, for a comment's checklist. */
  comment?: string;
  /** The history group, for revert and Undo; left out when nothing changed. */
  group?: string;
  /** The items now. */
  items: ChecklistItem[];
  /** The task's key. */
  key: string;
  /** The description's revision now, for a task. */
  revision?: string;
  /** The task's (or comment's) version now. */
  version: number;
};

/** One checklist item. */
export type ChecklistItem = {
  /** Whether it is ticked. */
  checked: boolean;
  /** How many items it is nested in. */
  depth: number;
  /** Its number, from 1, in reading order; the reader's checkboxes carry the same number. */
  number: number;
  /** Its text, without formatting. */
  text: string;
};

/** An item to remove, with its nested items. */
export type ChecklistRemove = {
  /** The item's number. */
  number: number;
  /** The item's text as you read it. */
  text: string;
};

/** Tick or clear one item. */
export type ChecklistToggle = {
  /** Tick (`true`) or clear (`false`); left out, the item flips. */
  checked?: boolean | null;
  /** The item's number. */
  number: number;
  /** The item's text as you read it. */
  text: string;
};

/** Changes to the checklist in a task's description or a comment. Numbers are as read before the change. When an item's number now points at other text, the item with that text is used if there is exactly one; otherwise nothing changes. */
export type ChecklistUpdateInput = {
  /** Items to add. */
  add?: ChecklistAdd[];
  /** Or the comment that holds it: its ID. */
  comment?: string | null;
  /** The task whose description holds the checklist: its key or ID. */
  key?: string | null;
  /** Items to remove. */
  remove?: ChecklistRemove[];
  /** Items to tick or clear. */
  toggle?: ChecklistToggle[];
};

/** An agent's claim on a task. */
export type Claim = {
  /** The agent's name. */
  agent: string;
  /** The agent session holding it. */
  session: string;
  /** When the claim ends unless renewed (RFC 3339, UTC). */
  until: string;
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

/** One comment. */
export type Comment = {
  /** Who wrote it. */
  author: Actor;
  /** The text (Markdown). */
  body: string;
  /** When it was written (RFC 3339, UTC). */
  created_at: string;
  /** When it was last edited; left out when never. */
  edited_at?: string;
  /** Files attached to it. */
  files?: Attachment[];
  /** The comment's ID. */
  id: string;
  /** The comment it answers. */
  reply_to?: string;
  /** How many earlier texts it has. */
  revision_count: number;
  /** Its earlier texts, newest first, when asked for. */
  revisions?: CommentRevision[];
  /** The task's key. */
  task: string;
  /** Goes up by one with every edit; pass it as `expected_version`. */
  version: number;
  /** The program it came through. */
  via: Via;
};

/** A new comment. */
export type CommentAddInput = {
  /** The text (Markdown), at most 256 KB. */
  body: string;
  /** Your own ID for this request; repeating it within 24 hours returns the first answer instead of adding a second comment. */
  client_request_id?: string | null;
  /** The task: its key or ID. */
  key: string;
  /** The ID of the comment this answers (same task). */
  reply_to?: string | null;
};

/** A comment changed, with the history group of the change. */
export type CommentChange = {
  /** The comment after the change. */
  comment: Comment;
  /** Whether it is in the trash now. */
  deleted: boolean;
  /** The history group, for revert and Undo; left out when nothing changed. */
  group?: string;
};

/** A task's comments, oldest first. */
export type CommentList = {
  /** The comments. */
  items: Comment[];
  /** How many. */
  total: number;
};

/** Which comments to list. */
export type CommentListInput = {
  /** The task: its key or ID. */
  key: string;
  /** Add each comment's earlier texts. */
  revisions?: boolean;
};

/** Names one comment. */
export type CommentRef = {
  /** The comment's ID. */
  comment: string;
};

/** An earlier text of a comment. */
export type CommentRevision = {
  /** The text as it was. */
  body: string;
  /** When it was replaced (RFC 3339, UTC). */
  replaced_at: string;
  /** Who replaced it with a newer text. */
  replaced_by: Actor;
};

/** A comment's new text. */
export type CommentUpdateInput = {
  /** The new text (Markdown); the earlier one is kept. */
  body: string;
  /** The comment's ID. */
  comment: string;
  /** Refuse the change unless the comment still has this version. */
  expected_version?: number | null;
};

/** What to do with a conflict copy. */
export type ConflictAction = "keep_current" | "use_copy" | "use_text";

/** How to resolve a conflict copy, inside `task.update`. */
export type ConflictResolution = {
  /** What to do with it. */
  action: ConflictAction;
  /** The conflict copy's revision ID. */
  copy: string;
  /** The text to save, for `use_text`. */
  text?: string | null;
};

/** Whose description history to list: a task's or a project's. */
export type DescriptionHistoryInput = {
  /** The project: its key or ID. */
  project?: string | null;
  /** The task: its key or ID. */
  task?: string | null;
};

/** One revision to read. */
export type DescriptionRevisionInput = {
  /** Another revision of the same description: adds the differences from it to this one. */
  diff_from?: string | null;
  /** The revision's ID. */
  revision: string;
};

/** The result of saving a description. */
export type DescriptionSave = {
  /** The conflict copy that holds the edit, after a conflict. */
  conflict_copy?: string;
  /** What became of the edit. */
  outcome: SaveOutcome;
  /** The description's revision now; pass it as `base_revision` next time. Left out while the description has none. */
  revision?: string;
};

/** One reminder that is due. */
export type DueReminder = {
  /** When it came due (RFC 3339, UTC). */
  at: string;
  /** The task's due date. */
  due_date?: string;
  /** The ID to acknowledge it with. */
  id: string;
  /** What reminds. */
  kind: ReminderKind;
  /** Whether it came due before `since`: missed while no program showed it. */
  missed: boolean;
  /** The reminder's text; left out when empty. */
  note?: string;
  /** The task. */
  task: TaskRef;
};

/** The reminders that are due and not acknowledged, oldest first, and when the next one comes. */
export type DueReminders = {
  /** The reminders due, oldest first. */
  items: DueReminder[];
  /** When the next reminder after `until` comes due (RFC 3339, UTC); left out when none is set. Schedule the next check for then. */
  next_at?: string;
  /** How many are due, also beyond `limit`. */
  total: number;
  /** The moment the list was made for (`until`). */
  until: string;
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

/** What to export. */
export type ExportInput = {
  /** A `.zip` file (the default) or a folder. */
  format?: ArchiveFormat | null;
  /** Where to write it: a new file or folder. Agents give a plain name, which goes into the workspace's `exports` folder. Left out: a name with the date in the `exports` folder. */
  path?: string | null;
  /** The projects (keys or IDs), each with its sub-projects; the whole workspace when empty. */
  projects?: string[];
};

/** A finished export. */
export type ExportResult = {
  /** What it holds. */
  counts: ArchiveCounts;
  /** How it is stored. */
  format: ArchiveFormat;
  /** The archive written. */
  path: string;
  /** The keys of the projects in it. */
  projects: string[];
  /** Its size in bytes. */
  size: number;
};

/** One field's change. A side is left out when the field had no value. */
export type FieldChange = {
  /** The value after. */
  after?: unknown;
  /** The value before. */
  before?: unknown;
};

/** A file to attach: from a local path, or as Base64 data with a name. */
export type FileAttachInput = {
  /** Your own ID for this request; repeating it within 24 hours returns the first answer instead of attaching the file again. */
  client_request_id?: string | null;
  /** The comment to attach it to: its ID. */
  comment?: string | null;
  /** The content as Base64, instead of `path`. */
  data_base64?: string | null;
  /** The file name; default: the path's last part. */
  name?: string | null;
  /** The path of a local file to read. */
  path?: string | null;
  /** The task to attach it to: its key or ID. */
  task?: string | null;
};

/** A file attached or deleted, with the history group of the change. */
export type FileChange = {
  /** Whether it is in the trash now. */
  deleted: boolean;
  /** The file. */
  file: Attachment;
  /** The history group, for revert and Undo; left out when nothing changed. */
  group?: string;
};

/** An attached file's content. */
export type FileContent = {
  /** The content as Base64, for an image (at most 5 MB). */
  data_base64?: string;
  /** The file. */
  file: Attachment;
  /** Why no content is given, for other files and large images. */
  note?: string;
  /** The text, for a text file (at most 256 KB). */
  text?: string;
  /** Whether the text was cut at the limit. */
  truncated: boolean;
};

/** What a file holds, which decides how it is shown. */
export type FileKind = "image" | "text" | "executable" | "other";

/** Names one attached file. */
export type FileRef = {
  /** The attachment's ID. */
  attachment: string;
};

/** The shape that shows a task's type. */
export type Glyph = "task" | "bug" | "feature" | "milestone";

/** The design's nine colours, for projects, statuses, types and labels, in palette order: a new label or project without a colour gets the first one its neighbours don't use yet. */
export type Hue = "red" | "orange" | "yellow" | "green" | "teal" | "blue" | "purple" | "pink" | "grey";

/** What to import. */
export type ImportInput = {
  /** Check the archive and report what would be imported, without importing it. */
  dry_run?: boolean;
  /** New keys for projects whose key is taken in this workspace. */
  keys?: KeyChange[];
  /** The archive: a `.zip` file or a folder. Agents give a name in the workspace's `exports` folder. */
  path: string;
};

/** The result of an import, or of checking one. */
export type ImportResult = {
  /** The program version that wrote the archive. */
  app_version: string;
  /** What the archive holds. */
  counts: ArchiveCounts;
  /** Whether this only checked the archive. */
  dry_run: boolean;
  /** The archive's format version. */
  format_version: number;
  /** The history group, for revert and Undo (which moves the imported projects to the trash); left out for a check. */
  group?: string;
  /** How many IDs existed here already and were replaced. */
  new_ids: number;
  /** The projects, as they are (or would be) in this workspace. */
  projects: ImportedProject[];
  /** Keys in the archive that are taken here and have no new key yet. */
  taken_keys?: string[];
};

/** One project of an import. */
export type ImportedProject = {
  /** Its key in the archive. */
  from_key: string;
  /** Its ID in this workspace. */
  id: string;
  /** Its key in this workspace. */
  key: string;
  /** Its name. */
  name: string;
};

/** A new key for a project whose key is taken. */
export type KeyChange = {
  /** The key in the archive. */
  from: string;
  /** The key to use instead. */
  to: string;
};

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
  /** Its colour; for a new label, default: the first palette colour no label of the project uses yet. */
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
  /** Its colour; default: the first palette colour no label of the project uses yet, else the least-used one (the project's `next_label_hue`). */
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

/** A link from one task to another. */
export type Link = {
  /** The link's ID. */
  id: string;
  /** How the link reads from this task. */
  kind: LinkKind;
  /** The task at the other end. */
  task: TaskRef;
};

/** How a link reads from one task. */
export type LinkKind = "blocks" | "blocked_by" | "relates" | "duplicates" | "duplicated_by" | "clones" | "cloned_by";

/** Totals over every matching task, not only the page. */
export type ListTotals = {
  /** Matching tasks. */
  count: number;
  /** Their estimates added up, in minutes. */
  estimate_minutes: number;
  /** Of them, open. */
  open: number;
  /** Estimate minus logged time over the tasks with an estimate, in minutes. */
  remaining_minutes: number;
  /** Their logged time added up, in minutes. */
  spent_minutes: number;
};

/** A task as lists show it. */
export type ListedTask = {
  /** Whether a task that blocks it is still open. */
  blocked: boolean;
  /** The description's checklist. */
  checklist: Checklist;
  /** The agent that claimed it, while the claim lasts. */
  claim?: Claim;
  /** When it was closed; left out while open. */
  closed_at?: string;
  /** How many comments it has. */
  comment_count: number;
  /** When it was created (RFC 3339, UTC). */
  created_at: string;
  /** Who created it. */
  created_by: Actor;
  /** The due date. */
  due_date?: string;
  /** The estimate in minutes. */
  estimate_minutes?: number;
  /** How many files are attached to it and to its comments. */
  file_count: number;
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
  /** Its progress in percent. */
  progress_percent: number;
  /** Its project's key. */
  project: string;
  /** Its place in its status column. */
  rank: string;
  /** Estimate minus logged time, in minutes; only with an estimate. */
  remaining_minutes?: number;
  /** Logged time in minutes. */
  spent_minutes: number;
  /** The start date. */
  start_date?: string;
  /** Its status. */
  status: StatusRef;
  /** Its subtasks: closed and all. */
  subtask_count: Checklist;
  /** Its title. */
  title: string;
  /** Its type. */
  type: TypeRef;
  /** When anything about it last changed. */
  updated_at: string;
  /** Its version, for `expected_version`. */
  version: number;
};

/** Where a result matched. */
export type MatchedIn = "key" | "title" | "text" | "fragment";

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

/** A query as the query field shows it. */
export type ParsedQuery = {
  /** The query has `OR` or parentheses: it is edited as text. */
  advanced: boolean;
  /** The chips, when the query is a plain AND of conditions. */
  chips?: QueryChip[];
  /** The first mistake, if any. */
  error?: QueryProblem;
  /** The field registry, when asked for. */
  fields?: QueryField[];
  /** The query in its normal form; left out when it doesn't read. */
  normalized?: string;
  /** The sort keys of its `sort:`, `-` for descending. */
  sort?: string[];
  /** The highlighted pieces, in order. */
  tokens: QueryToken[];
  /** Whether the query can run: it reads, fits the fields and names only what exists. */
  valid: boolean;
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
  /** Its icon, if one was chosen; without one it shows as a circle. */
  icon?: ProjectIcon;
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
  /** The colour a new label gets when none is given: the first palette colour no label uses yet, else the least-used one. */
  next_label_hue: Hue;
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
  /** The colour; default: the first palette colour that no sibling project uses yet (the least-used one when all are taken). */
  hue?: Hue | null;
  /** An icon shown instead of the circle; default: none (the circle). */
  icon?: ProjectIcon | null;
  /** The key: 2 to 10 capital letters and digits, starting with a letter (`TM`). Task keys start with it. */
  key: string;
  /** The name, 1 to 100 characters. */
  name: string;
  /** The parent, for a sub-project (at most 3 levels). It copies the parent's statuses, types and labels. */
  parent?: string | null;
};

/** A project to delete for good, with its sub-projects. */
export type ProjectDeleteInput = {
  /** The number of tasks you expect it and its sub-projects to hold, trashed ones included; the deletion stops if the number differs. */
  expected_task_count?: number | null;
  /** The project: its key (current or old) or ID. */
  project: string;
};

/** What a project deletion moved to the trash. */
export type ProjectDeleted = {
  /** Comments in them. */
  comments: number;
  /** Attached files in them. */
  files: number;
  /** The history group that records the deletion, for revert and Undo. */
  group: string;
  /** The deleted project's key. */
  project: string;
  /** The keys of the deleted projects: it and its sub-projects. */
  projects: string[];
  /** Tasks in them, trashed ones included. */
  tasks: number;
};

/** A project field that can be cleared. */
export type ProjectField = "icon" | "description" | "parent";

/** An icon that stands for a project instead of its coloured circle, drawn in the project's colour. The names are those of the app's icon family (Lucide), so each variant's name says what it shows. */
export type ProjectIcon = "folder" | "star" | "bug" | "rocket" | "book" | "house" | "briefcase" | "code" | "terminal" | "globe" | "heart" | "flag" | "zap" | "target" | "lightbulb" | "wrench" | "hammer" | "palette" | "music" | "camera" | "graduation-cap" | "shopping-cart" | "wallet" | "chart-line" | "calendar" | "users" | "building" | "plane" | "leaf" | "coffee" | "gamepad" | "puzzle" | "shield" | "flask-conical" | "layers" | "package" | "compass" | "mountain" | "sparkles" | "database";

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
  /** Its icon, if one was chosen; without one it shows as a circle. */
  icon?: ProjectIcon;
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
  /** A new icon, shown instead of the circle. To go back to the circle, list `icon` in `clear`. */
  icon?: ProjectIcon | null;
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

/** One chip of the condition builder. */
export type QueryChip = {
  /** The field; left out for the free-text chip. */
  field?: string;
  /** "is not" or "doesn't contain". */
  negated: boolean;
  /** The operator (`:`, `=`, `<`, `<=`, `>`, `>=`); left out for text. */
  op?: string;
  /** The text the chip stands for; editing the chip replaces only these. */
  ranges: TextRange[];
  /** The values, or the words and phrases of the text chip. */
  values: string[];
};

/** One field of the query language. */
export type QueryField = {
  /** Other names for it. */
  aliases?: string[];
  /** Its fixed values, when it has some. */
  choices?: string[];
  /** Its name. */
  name: string;
  /** The operators it takes. */
  operators: string[];
  /** Whether `a..b` ranges work. */
  ranges: boolean;
  /** Whether it can be a sort key. */
  sortable: boolean;
  /** The values it takes, in words. */
  values: string;
};

/** A query to read for the query field. */
export type QueryParseInput = {
  /** Also return the field registry, for completion. */
  fields?: boolean;
  /** The query text. */
  query: string;
};

/** A mistake in a query. */
export type QueryProblem = {
  /** The message's arguments. */
  args?: Record<string, string>;
  /** The message in English. */
  message: string;
  /** The message's ID in the message files, for translation. */
  message_id: string;
  /** Where the mistake is. */
  range: TextRange;
};

/** A highlighted piece of the query text. */
export type QueryToken = {
  /** What it is. */
  kind: QueryTokenKind;
  /** Where it is. */
  range: TextRange;
};

/** What a piece of the query text is. */
export type QueryTokenKind = "field" | "operator" | "value" | "comma" | "keyword" | "negation" | "paren" | "text" | "key" | "sort" | "error";

/** A task that changed recently, with who changed it last. */
export type RecentTask = {
  /** When (RFC 3339, UTC). */
  changed_at: string;
  /** Who made its last change. */
  changed_by: Actor;
  /** The task. */
  task: ListedTask;
};

/** A link to add or remove. */
export type RelationInput = {
  /** The task: its key or ID. */
  key: string;
  /** How the link reads from this task. `subtask_of` and `parent_of` change the parent instead of adding a link. */
  kind: RelationKind;
  /** The other task: its key or ID. */
  other: string;
};

/** The kinds of link, as read from the first task. */
export type RelationKind = "blocks" | "blocked_by" | "relates" | "duplicates" | "duplicated_by" | "clones" | "cloned_by" | "subtask_of" | "parent_of";

/** A task's personal reminder. */
export type Reminder = {
  /** Whether it has fired and was acknowledged. */
  acknowledged: boolean;
  /** When it fires (RFC 3339, UTC); left out while a reminder relative to the due date has no due date to follow. */
  at?: string;
  /** When it was set (RFC 3339, UTC). */
  created_at: string;
  /** For a reminder that follows the due date: how many days before it. */
  days_before_due?: number;
  /** The reminder's ID. */
  id: string;
  /** Its text; left out when empty. */
  note?: string;
  /** Who set it. */
  set_by: Actor;
  /** The task's key. */
  task: string;
  /** For a reminder that follows the due date: the time of day (`HH:MM`); left out, the workspace's reminder time. */
  time?: string;
};

/** Reminders that were shown. */
export type ReminderAckInput = {
  /** The `id`s of due reminders, as the list of due reminders gives them. */
  ids: string[];
};

/** What became of the acknowledged reminders. */
export type ReminderAcknowledged = {
  /** The reminders acknowledged; they won't come due again. */
  acknowledged: string[];
  /** IDs that name no due reminder (already acknowledged, changed or removed). */
  unknown?: string[];
};

/** A task's reminder after a change. */
export type ReminderChange = {
  /** The history group, for revert and Undo; left out when nothing changed. */
  group?: string;
  /** The reminder now; left out when it was removed. */
  reminder?: Reminder;
  /** The task's key. */
  task: string;
};

/** Names the task whose reminder to remove. */
export type ReminderClearInput = {
  /** The task: its key or ID. */
  key: string;
};

/** Which reminders are due. */
export type ReminderDueInput = {
  /** At most this many, 1 to 500; default 100. */
  limit?: number | null;
  /** Reminders due before this moment are marked `missed`: pass the time your program started, so what came due while it was closed shows as missed. Default: due more than 5 minutes ago. */
  since?: string | null;
  /** Reminders due up to this moment (RFC 3339 or local `YYYY-MM-DD HH:MM`); default now. A later moment lists what is coming. */
  until?: string | null;
};

/** What reminds. */
export type ReminderKind = "reminder" | "deadline" | "timer";

/** A personal reminder on a task; it replaces the one the task had. */
export type ReminderSetInput = {
  /** When: RFC 3339, or a local `YYYY-MM-DD HH:MM`. Give this or `days_before_due`. */
  at?: string | null;
  /** Fire this many days (0 to 365) before the due date, and again when the due date changes. */
  days_before_due?: number | null;
  /** The task: its key or ID. */
  key: string;
  /** Its text, at most 500 characters. */
  note?: string | null;
  /** With `days_before_due`: the time of day, `HH:MM`; default the workspace's reminder time. */
  time?: string | null;
};

/** Texts to render for the reader. */
export type RenderInput = {
  /** One to 200 texts. */
  documents: RenderSource[];
};

/** The rendered texts, in the order asked. */
export type RenderOutput = {
  /** The texts. */
  items: Rendered[];
};

/** One text to render. Give exactly one of `source`, `task`, `comment`, `project` and `revision`. */
export type RenderSource = {
  /** A comment's body: its ID. */
  comment?: string | null;
  /** The id that keeps this text's footnote anchors apart from others on the page; default: the item's ID (`preview` for `source`). */
  document_id?: string | null;
  /** A project's description: its key or ID. */
  project?: string | null;
  /** A description revision: its ID. */
  revision?: string | null;
  /** Markdown to render as it is, such as an editor's preview. */
  source?: string | null;
  /** A task's description: its key or ID. */
  task?: string | null;
};

/** One rendered text. */
export type Rendered = {
  /** Its checklist; the HTML's checkboxes carry the same numbers. */
  checklist: ChecklistItem[];
  /** The id its footnote anchors are made from. */
  document_id: string;
  /** Sanitized HTML, safe to insert as it is. */
  html: string;
  /** The description's revision that was rendered, for a task or project. */
  revision?: string;
  /** The item's version (task or comment) that was rendered. */
  version?: number;
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

/** One revision with its text. */
export type RevisionDetail = {
  /** The text (Markdown). */
  body: string;
  /** The differences from `diff_from` as a unified diff; empty when the texts are the same. */
  diff?: string;
  /** The task's or project's key, or the wiki page's title. */
  owner: string;
  /** The revision. */
  revision: RevisionInfo;
};

/** One revision of a description, without its text. */
export type RevisionInfo = {
  /** Who saved it. */
  author: Actor;
  /** The revision it was edited from. */
  base?: string;
  /** Its size in bytes. */
  bytes: number;
  /** When (RFC 3339, UTC). */
  created_at: string;
  /** Whether it is the description now. */
  current: boolean;
  /** The revision's ID. */
  id: string;
  /** How it came about. */
  kind: RevisionKind;
  /** A note saved with it. */
  message?: string;
  /** For a conflict copy: when it was resolved; left out while it is open. */
  resolved_at?: string;
};

/** How a revision came about. */
export type RevisionKind = "normal" | "merged" | "conflict" | "restored" | "resolved";

/** A description's revisions, newest first, conflict copies included. */
export type RevisionList = {
  /** The revisions. */
  items: RevisionInfo[];
  /** How many. */
  total: number;
};

/** What became of a description that was saved. */
export type SaveOutcome = "saved" | "merged" | "conflict" | "unchanged";

/** A saved view. */
export type SavedView = {
  /** The columns shown, in order. */
  columns?: string[];
  /** When it was created (RFC 3339, UTC). */
  created_at: string;
  /** What it is for. */
  description?: string;
  /** The field rows are grouped by. */
  group_by?: string;
  /** Its ID. */
  id: string;
  /** Shown first when its screen opens. */
  is_default: boolean;
  /** The screen it is for. */
  kind: ViewKind;
  /** Its name. */
  name: string;
  /** The project's key; left out for a view of the whole workspace. */
  project?: string;
  /** The query text. */
  query: string;
  /** Its place among the views. */
  rank: string;
  /** The order when the query has no `sort:` (`due,-priority`). */
  sort?: string;
  /** The columns totalled (`estimate`, `spent`, `remaining`). */
  totals?: string[];
  /** When it last changed. */
  updated_at: string;
};

/** One search result. */
export type SearchHit = {
  /** The task's status category. */
  category?: Category;
  /** The ID of what was found: the task, comment, page, file or project. */
  id: string;
  /** What it is. */
  kind: SearchKind;
  /** Where it matched; results are grouped by it, best first. */
  matched_in: MatchedIn;
  /** The project's key. */
  project: string;
  /** The wiki page's slug. */
  slug?: string;
  /** A piece of the text around the matches; left out when only the title matched. */
  snippet?: string;
  /** The matches in `snippet`, in UTF-16 code units. */
  snippet_matches?: TextRange[];
  /** The task's status name, for tasks and their comments and files. */
  status?: string;
  /** The task's key, for tasks and their comments and files. */
  task?: string;
  /** The title as shown: the task's title, the page's title, the file's name, the project's name; a comment shows its task's title. */
  title: string;
  /** The matches in `title`, in UTF-16 code units. */
  title_matches?: TextRange[];
  /** When it last changed (RFC 3339, UTC). */
  updated_at: string;
};

/** What to search for. */
export type SearchInput = {
  /** The `next_cursor` of the page before. */
  cursor?: string | null;
  /** With `project`: its sub-projects too. */
  include_subprojects?: boolean;
  /** Only these kinds; default: all. */
  kinds?: SearchKind[];
  /** Results per page, 1 to 200; default 50. */
  limit?: number | null;
  /** Only this project (key or ID). */
  project?: string | null;
  /** Only tasks matching this task query (`is:open label:bug`), with their comments and files; wiki pages are left out. */
  query?: string | null;
  /** Words (each found as a whole word or a word's beginning) and "quoted phrases"; all must occur. A task key such as `TM-41` also finds that task. */
  text: string;
};

/** What a search result is. */
export type SearchKind = "project" | "task" | "comment" | "wiki" | "file";

/** One page of search results. */
export type SearchResult = {
  /** The results of this page, best first. */
  items: SearchHit[];
  /** Pass as `cursor` for the next page; left out on the last page. */
  next_cursor?: string;
  /** How many results there are. */
  total: number;
};

/** A setting that can be set back to its default. */
export type SettingKey = "trash_days" | "reminder_time" | "hours_per_day" | "days_per_week" | "week_start" | "file_max_mb" | "timer_reminder_hours";

/** The workspace's settings, with defaults filled in. */
export type Settings = {
  /** The limits on every agent session. Only the person sets them, in the app. */
  agent_limits: AgentLimits;
  /** The most an agent may do, whatever profile it asks for. */
  agent_max_profile: Profile;
  /** The folder that holds this workspace's backups, as an absolute path; left out: the default folder. Only the person sets it, in the app. */
  backup_folder?: string;
  /** Working days in a week, for durations written in weeks (`1w`). */
  days_per_week: number;
  /** The largest file that can be attached, in megabytes. */
  file_max_mb: number;
  /** Working hours in a day, for durations written in days (`2d`). */
  hours_per_day: number;
  /** When reminders for a due date fire, as `HH:MM`. */
  reminder_time: string;
  /** Hours a timer runs before it reminds that it is still running; 0 never. */
  timer_reminder_hours: number;
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
  /** The largest file that can be attached, in megabytes, 1 to 1024. */
  file_max_mb?: number | null;
  /** Working hours in a day, 1 to 24. */
  hours_per_day?: number | null;
  /** Reminder time, `HH:MM` (24-hour). */
  reminder_time?: string | null;
  /** Hours a timer runs before it reminds that it is still running, 0 (never) to 24. */
  timer_reminder_hours?: number | null;
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
  /** The reason's arguments, by name. */
  reason_args?: Record<string, string>;
  /** The reason's message ID, for showing it in another language. */
  reason_id: string;
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

/** Where to put a subtask among its parent's subtasks. Subtasks keep an order of their own, apart from their places on the board. */
export type SubtaskReorderInput = {
  /** Place it after this subtask of the same parent. */
  after?: string | null;
  /** Place it first or last among the parent's subtasks. */
  at?: Edge | null;
  /** Place it before this subtask of the same parent. */
  before?: string | null;
  /** Refuse the change unless the subtask still has this version. */
  expected_version?: number | null;
  /** The subtask: its key or ID. */
  key: string;
  /** Reorder it although another agent claimed it (full profile only). */
  override_claim?: boolean;
};

/** One task. */
export type Task = {
  /** Whether a task that blocks it is still open. */
  blocked: boolean;
  /** The description's checklist. */
  checklist: Checklist;
  /** The description's checklist items, numbered, when asked for with `include`. */
  checklist_items?: ChecklistItem[];
  /** The agent that claimed it, while the claim lasts. */
  claim?: Claim;
  /** When it was closed; left out while it is open. */
  closed_at?: string;
  /** How many comments it has. */
  comment_count: number;
  /** Its comments, oldest first, when asked for with `include`. */
  comments?: Comment[];
  /** Edits of the description that clashed with another and wait to be resolved (conflict copies). */
  conflict_copies: number;
  /** When it was created (RFC 3339, UTC). */
  created_at: string;
  /** Who created it. */
  created_by: Actor;
  /** Its description (Markdown). */
  description: string;
  /** The description's revision; pass it as `base_revision` when you save a new description. Left out while it has none. */
  description_revision?: string;
  /** The due date. */
  due_date?: string;
  /** When its own fields last changed. */
  edited_at: string;
  /** The estimate, as a duration (`1d 4h`). */
  estimate?: string;
  /** The estimate in minutes. */
  estimate_minutes?: number;
  /** How many files are attached to it and to its comments. */
  file_count: number;
  /** The files attached to it, when asked for with `include`. */
  files?: Attachment[];
  /** The task's ID. */
  id: string;
  /** The task's key (`TM-41`). */
  key: string;
  /** Its labels, in the project's order. */
  labels: LabelRef[];
  /** Its links to other tasks, when asked for with `include`. */
  links?: Link[];
  /** Keys it had before; they still find it. */
  old_keys?: string[];
  /** The parent task's key, for a subtask. */
  parent?: string;
  /** Its priority. */
  priority: Priority;
  /** Progress set by hand, 0 to 100. */
  progress?: number;
  /** Its progress in percent: the progress set by hand, else 100 when closed, else the share of ticked checklist items, else its subtasks' progress weighted by their estimates, else 0. */
  progress_percent: number;
  /** Its project's key. */
  project: string;
  /** Its place in its status column. */
  rank: string;
  /** Its personal reminder, when asked for with `include` and it has one. */
  reminder?: Reminder;
  /** The description's revisions, newest first, conflict copies included, when asked for with `include`. */
  revisions?: RevisionInfo[];
  /** Logged time in minutes. */
  spent_minutes: number;
  /** The start date. */
  start_date?: string;
  /** Its status. */
  status: StatusRef;
  /** When it entered its status. */
  status_entered_at: string;
  /** Its subtasks: closed and all. */
  subtask_count: Checklist;
  /** Its subtasks in order, when asked for with `include`. */
  subtasks?: TaskRef[];
  /** Its time entries, newest day first, when asked for with `include`. */
  time_entries?: TimeEntry[];
  /** Its title. */
  title: string;
  /** Totals over the task and all its subtasks; only for a task with subtasks. */
  totals?: Totals;
  /** Its type. */
  type: TypeRef;
  /** When anything about it last changed. */
  updated_at: string;
  /** Goes up by one with every change; pass it as `expected_version` to make sure nobody changed the task since you read it. */
  version: number;
};

/** A task changed, with the history group of the change. */
export type TaskChange = {
  /** What became of a new description: saved, merged with another edit, or kept as a conflict copy. */
  description_save?: DescriptionSave;
  /** The history group, for revert and Undo; left out when nothing changed. */
  group?: string;
  /** The task after the change. */
  task: Task;
  /** Problems that did not stop the change, such as closing a task whose subtasks are still open. */
  warnings?: string[];
};

/** A claim on a task, for an agent working on it. */
export type TaskClaimInput = {
  /** Leave the status as it is. */
  keep_status?: boolean;
  /** The task: its key or ID. */
  key: string;
  /** How long the claim lasts unless renewed, 1 to 480 minutes; default 60. Every change you make to the task renews it. */
  lease_minutes?: number | null;
  /** Take the claim over from another agent (full profile only). */
  override_claim?: boolean;
  /** The status to move it to; default: the first in-progress status, unless it is in progress already. */
  status?: string | null;
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
  /** The due date: `YYYY-MM-DD`, or `today`, `fri`, `next mon`, `+3d`, `in 2 weeks`, `dec 1`, in the workspace's time zone. */
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
  /** The start date: `YYYY-MM-DD`, or `today`, `fri`, `next mon`, `+3d`, `in 2 weeks`, `dec 1`, in the workspace's time zone. */
  start_date?: string | null;
  /** The status; default: the project's first to-do status. */
  status?: string | null;
  /** The title, 1 to 500 characters; tabs and line breaks become spaces. */
  title: string;
  /** The type; default: the project's default type. */
  type?: string | null;
};

/** Tasks to move to the trash. */
export type TaskDeleteInput = {
  /** The tasks: keys or IDs, 1 to 500. Their subtasks go with them. */
  keys: string[];
};

/** A copy of a task. */
export type TaskDuplicateInput = {
  /** Your own ID for this request; repeating it within 24 hours returns the first answer instead of making a second copy. */
  client_request_id?: string | null;
  /** The task to copy: its key or ID. */
  key: string;
  /** The copy's status; default: the project's first to-do status. */
  status?: string | null;
  /** The copy's title; default: the same title. */
  title?: string | null;
  /** Leave the subtasks out; by default they are copied too, in order. */
  without_subtasks?: boolean;
};

/** A task field that can be cleared. */
export type TaskField = "description" | "parent" | "start_date" | "due_date" | "estimate" | "progress" | "labels";

/** Which tasks to read. */
export type TaskGetInput = {
  /** What to add to each task: `subtasks`, `links`, `comments`, `files`, `checklist`, `revisions`, `time`, `reminder`. */
  include?: TaskPart[];
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
  /** Move the task although another agent claimed it (full profile only). */
  override_claim?: boolean;
  /** Move it to another project: it gets the next key there, and the old key keeps finding it. Its subtasks move with it. */
  project?: string | null;
  /** The new status, by name or ID. */
  status?: string | null;
};

/** What `task.get` adds to each task. */
export type TaskPart = "subtasks" | "links" | "comments" | "files" | "checklist" | "revisions" | "time" | "reminder";

/** Another task, as links and subtask lists show it. */
export type TaskRef = {
  /** Whether it is closed (done or cancelled). */
  closed: boolean;
  /** Its ID. */
  id: string;
  /** Its key. */
  key: string;
  /** Its status. */
  status: StatusRef;
  /** Its title. */
  title: string;
};

/** Giving up a claim. */
export type TaskReleaseInput = {
  /** A comment to add (Markdown), for example what was done. */
  comment?: string | null;
  /** The task: its key or ID. */
  key: string;
  /** Release another agent's claim (full profile only; the person can always). */
  override_claim?: boolean;
  /** A status to move it to, for example Review. */
  status?: string | null;
};

/** Resolving a task's conflict copy. */
export type TaskResolveInput = {
  /** What to do with it. */
  action: ConflictAction;
  /** Refuse unless the description is still at this revision. */
  base_revision?: string | null;
  /** The conflict copy's revision ID. */
  copy: string;
  /** The task: its key or ID. */
  key: string;
  /** The text to save, for `use_text`. */
  text?: string | null;
};

/** Which tasks to list. */
export type TaskSearchInput = {
  /** The `next_cursor` of the page before. */
  cursor?: string | null;
  /** With `project`: its sub-projects' tasks too. */
  include_subprojects?: boolean;
  /** Tasks per page, 1 to 200; default 50. */
  limit?: number | null;
  /** Only this project (key or ID). */
  project?: string | null;
  /** The query, for example `is:open due<+7d sort:due`; empty lists every open and closed task of active projects. */
  query?: string;
  /** The order when the query has no `sort:`, written like its keys (`due,-priority`). Default: by relevance for text searches, else by status, then place on the board. */
  sort?: string | null;
};

/** One page of matching tasks. */
export type TaskSearchResult = {
  /** The tasks of this page, in order. */
  items: ListedTask[];
  /** Pass as `cursor` for the next page; left out on the last page. */
  next_cursor?: string;
  /** The query in its normal form. */
  query: string;
  /** The order used, as sort keys (`-` for descending). */
  sort: string[];
  /** How many tasks match. */
  total: number;
  /** Totals over every matching task. */
  totals: ListTotals;
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
  /** The description revision your new description was edited from. If somebody saved in between, the two edits are merged; when they changed the same lines, yours is kept as a conflict copy and the description stays as it is. */
  base_revision?: string | null;
  /** Fields to clear. */
  clear?: TaskField[];
  /** A new description (Markdown), replacing the old one. */
  description?: string | null;
  /** Targeted changes to the description instead of a new one: each `old_text` must occur exactly once. */
  description_edits?: TextEdit[];
  /** A new due date: `YYYY-MM-DD`, or a natural date such as `next fri` or `+3d`. */
  due_date?: string | null;
  /** A new estimate, as a duration. */
  estimate?: string | null;
  /** Refuse the change unless the task still has this version. */
  expected_version?: number | null;
  /** The task: its key or ID. */
  key: string;
  /** Replace the labels with these. */
  labels?: string[] | null;
  /** Change the task although another agent claimed it (full profile only). */
  override_claim?: boolean;
  /** A new parent task (same project, at most 5 levels, no loops). */
  parent?: string | null;
  /** A new priority. */
  priority?: Priority | null;
  /** Progress set by hand, 0 to 100. */
  progress?: number | null;
  /** Labels to take off. */
  remove_labels?: string[];
  /** Resolve a conflict copy of the description. */
  resolve_conflict?: ConflictResolution | null;
  /** A new start date: `YYYY-MM-DD`, or a natural date such as `fri` or `in 3d`. */
  start_date?: string | null;
  /** A new title. */
  title?: string | null;
  /** A new type. */
  type?: string | null;
};

/** A targeted change to a text: `old_text` must occur exactly once. */
export type TextEdit = {
  /** What replaces it. */
  new_text: string;
  /** The text to replace; it must occur exactly once. */
  old_text: string;
};

/** A range of the query text, in UTF-16 code units (as the page's editor counts). */
export type TextRange = {
  /** The first unit. */
  from: number;
  /** One past the last unit. */
  to: number;
};

/** A thumbnail of an attached image. */
export type Thumbnail = {
  /** The attachment's ID. */
  attachment: string;
  /** Its height in pixels. */
  height: number;
  /** Its media type: `image/png`. */
  mime: string;
  /** The thumbnail's file in the workspace's cache. */
  path: string;
  /** Its width in pixels. */
  width: number;
};

/** One time entry. */
export type TimeEntry = {
  /** Who logged it. */
  author: Actor;
  /** When it was logged (RFC 3339, UTC). */
  created_at: string;
  /** The day the work was done (`YYYY-MM-DD`). */
  date: string;
  /** The time as a duration in the workspace's units (`1h 30m`). */
  duration: string;
  /** When it was last edited; left out when never. */
  edited_at?: string;
  /** The entry's ID. */
  id: string;
  /** The time in minutes. */
  minutes: number;
  /** What was done; left out when empty. */
  note?: string;
  /** The task's key. */
  task: string;
  /** Goes up by one with every edit; pass it as `expected_version`. */
  version: number;
};

/** A time entry changed, with the history group of the change. */
export type TimeEntryChange = {
  /** Whether it is in the trash now. */
  deleted: boolean;
  /** The entry after the change. */
  entry: TimeEntry;
  /** The history group, for revert and Undo; left out when nothing changed. */
  group?: string;
  /** The task's logged time after the change, in minutes. */
  spent_minutes: number;
};

/** Names one time entry. */
export type TimeEntryRef = {
  /** The entry's ID. */
  entry: string;
};

/** How the time report groups the entries. */
export type TimeGroupBy = "day" | "task" | "project";

/** Time to log on a task. */
export type TimeLogInput = {
  /** Your own ID for this request; repeating it within 24 hours returns the first answer instead of logging the time twice. */
  client_request_id?: string | null;
  /** The day of the work: `YYYY-MM-DD`, `today` or `yesterday`; default today. */
  date?: string | null;
  /** How long: `1h 30m`, `90m`, `1.5h`, `1:30`, `+30m`; at most 24 hours. */
  duration: string;
  /** The task: its key or ID. */
  key: string;
  /** What was done, at most 2000 characters. */
  note?: string | null;
};

/** Logged time over a range of days, grouped, with the total. */
export type TimeReport = {
  /** The working week the units use, in days. */
  days_per_week: number;
  /** The entries, oldest first, when asked for. */
  entries?: TimeEntry[];
  /** The first day. */
  from: string;
  /** How the rows are grouped. */
  group_by: TimeGroupBy;
  /** The working day the units use, in hours. */
  hours_per_day: number;
  /** The groups: days in order; tasks and projects by most time first. */
  items: TimeReportRow[];
  /** The last day, included. */
  to: string;
  /** The total in the workspace's units. */
  total_duration: string;
  /** The total in minutes. */
  total_minutes: number;
};

/** Which time to report. */
export type TimeReportInput = {
  /** Also list the entries, oldest first. */
  entries?: boolean;
  /** The first day: `YYYY-MM-DD`, `today` or `yesterday`; default the first day of the current week. */
  from?: string | null;
  /** How to group: `day`, `task` (default) or `project`. */
  group_by?: TimeGroupBy | null;
  /** Only this project and its sub-projects: a key or ID. */
  project?: string | null;
  /** Only this task and its subtasks: a key or ID. */
  task?: string | null;
  /** The last day, included; default six days after `from`. At most 366 days after `from`. */
  to?: string | null;
};

/** One group of the time report. */
export type TimeReportRow = {
  /** The time in the workspace's units. */
  duration: string;
  /** How many entries. */
  entry_count: number;
  /** The day, the task's key or the project's key. */
  key: string;
  /** The time in minutes. */
  minutes: number;
  /** The weekday, the task's title or the project's name. */
  title: string;
};

/** Changes to a time entry; fields left out keep their value. */
export type TimeUpdateInput = {
  /** The new day: `YYYY-MM-DD`, `today` or `yesterday`. */
  date?: string | null;
  /** The new duration. */
  duration?: string | null;
  /** The entry's ID. */
  entry: string;
  /** Refuse the change unless the entry still has this version. */
  expected_version?: number | null;
  /** The new note; an empty text removes it. */
  note?: string | null;
};

/** The workspace's timer. */
export type Timer = {
  /** The time so far in the workspace's units, rounded to minutes. */
  elapsed: string;
  /** The time so far, in seconds. */
  elapsed_seconds: number;
  /** The note the entry will get; left out when empty. */
  note?: string;
  /** When the current running stretch began; left out while paused. */
  running_since?: string;
  /** When it was first started (RFC 3339, UTC). */
  started_at: string;
  /** Who started it. */
  started_by: Actor;
  /** Running or paused. */
  state: TimerState;
  /** The task it times. */
  task: TaskRef;
  /** Whether the task is in the trash; stop the timer with `discard`. */
  task_trashed?: boolean;
};

/** The timer after a change. */
export type TimerChange = {
  /** The entry the stopped timer logged. */
  entry?: TimeEntry;
  /** The history group, for revert and Undo; left out when nothing changed. */
  group?: string;
  /** The timer now; left out when it was stopped. */
  timer?: Timer;
  /** What didn't stop the change, such as time under half a minute that was not logged. */
  warnings?: string[];
};

/** No input. */
export type TimerGetInput = Record<string, never>;

/** No input. */
export type TimerPauseInput = Record<string, never>;

/** Start the timer, or start a paused one again. */
export type TimerStartInput = {
  /** The task: its key or ID. Left out, the paused timer runs again. A timer on another task is stopped first and its time logged. */
  key?: string | null;
  /** The note the entry will get, at most 2000 characters. */
  note?: string | null;
};

/** Whether the timer runs. */
export type TimerState = "running" | "paused";

/** The timer, if there is one. */
export type TimerStatus = {
  /** The timer; left out when none runs or is paused. */
  timer?: Timer;
};

/** Stop the timer and log its time. */
export type TimerStopInput = {
  /** Throw the time away instead of logging it. */
  discard?: boolean;
  /** Log this duration instead of the time measured, for example when the timer ran on by mistake. */
  duration?: string | null;
  /** The entry's note; default the timer's own note. */
  note?: string | null;
};

/** What needs attention today. */
export type Today = {
  /** Tasks agents are working on (claimed), by when the claim ends. */
  claimed: ListedTask[];
  /** The size of every list. */
  counts: TodayCounts;
  /** Today's date in the workspace's time zone (`YYYY-MM-DD`). */
  date: string;
  /** Open tasks due today, by priority. */
  due_today: ListedTask[];
  /** Open tasks in an in-progress status that are not in the two lists above, by project and place on the board. */
  in_progress: ListedTask[];
  /** Open tasks due before today, the oldest first. */
  overdue: ListedTask[];
  /** Tasks changed in the last 24 hours, the newest first. */
  recent: RecentTask[];
  /** Personal reminders that come due today or were missed and not acknowledged, the earliest first. */
  reminders: DueReminder[];
  /** The timer, while one runs or is paused. */
  timer?: Timer;
};

/** How many items each list has, also beyond the limit. */
export type TodayCounts = {
  /** Tasks claimed by agents. */
  claimed: number;
  /** Open tasks due today. */
  due_today: number;
  /** Open tasks in an in-progress status, not due today or before. */
  in_progress: number;
  /** Every open task of the active projects; the sidebar shows it next to All tasks. */
  open: number;
  /** Open tasks overdue. */
  overdue: number;
  /** Tasks changed in the last 24 hours. */
  recent: number;
  /** Personal reminders due today or missed, not acknowledged. */
  reminders: number;
  /** Tasks on Today: overdue, due today and in progress, each once. The sidebar shows it next to Today. */
  today: number;
};

/** Which part of the workspace. */
export type TodayInput = {
  /** At most this many tasks per list, 1 to 200; default 50. The counts are always complete. */
  limit?: number | null;
  /** Only this project (key or ID), with its sub-projects; default: every active project. */
  project?: string | null;
};

/** Estimate and time over a task and all its subtasks. */
export type Totals = {
  /** The estimates added up, in minutes. */
  estimate_minutes: number;
  /** Estimate minus spent time, in minutes; below 0 when over. */
  remaining_minutes: number;
  /** The logged time added up, in minutes. */
  spent_minutes: number;
};

/** What a deletion, restore or purge touched. */
export type TrashChange = {
  /** The history group, for revert and Undo; left out when nothing changed. */
  group?: string;
  /** The entries, as the trash lists them (for a purge: as they were). */
  items: TrashEntry[];
  /** How many items moved, counting what went with each entry. */
  total: number;
};

/** One entry of the trash. */
export type TrashEntry = {
  /** How many other items went to the trash with it: subtasks, pages inside a page, sub-projects. */
  contains: number;
  /** When it was deleted (RFC 3339, UTC). */
  deleted_at: string;
  /** Who deleted it, when history knows. */
  deleted_by?: Actor;
  /** The history group of the deletion. */
  group: string;
  /** The item's ID; pass it to restore or purge the entry. */
  id: string;
  /** A task's or project's key. */
  key?: string;
  /** What it is. */
  kind: TrashKind;
  /** For a time entry: its minutes. */
  minutes?: number;
  /** The key of the project it belongs to. */
  project?: string;
  /** When it will be purged (RFC 3339, UTC). */
  purge_at: string;
  /** For a file: its size in bytes. */
  size?: number;
  /** For a comment, file or time entry: the task's key. */
  task?: string;
  /** Its title: a task's or page's title, a project's name, a file's name, the beginning of a comment, a time entry's note. */
  title: string;
};

/** What kind of item an entry of the trash is. */
export type TrashKind = "project" | "task" | "comment" | "file" | "time_entry" | "wiki_page";

/** Entries of the trash, newest deletion first. */
export type TrashList = {
  /** The entries. */
  items: TrashEntry[];
  /** The cursor of the next page; left out on the last one. */
  next_cursor?: string;
  /** How many entries match, on all pages. */
  total: number;
  /** Days an item stays in the trash (the `trash_days` setting). */
  trash_days: number;
};

/** Which entries of the trash to list. */
export type TrashListInput = {
  /** The `next_cursor` of the page before. */
  cursor?: string | null;
  /** Only these kinds; all when empty. */
  kinds?: TrashKind[];
  /** Entries per page, 1 to 200 (50). */
  limit?: number | null;
  /** Only entries of this project (key or ID), trashed projects included. */
  project?: string | null;
};

/** Entries of the trash to delete for good. */
export type TrashPurgeInput = {
  /** Empty the whole trash instead (not for agents). */
  all?: boolean;
  /** The entries' IDs, up to 100. */
  items?: string[];
};

/** Entries of the trash to restore. */
export type TrashRestoreInput = {
  /** The entries' IDs, 1 to 100. */
  items: string[];
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

/** A view after a change. */
export type ViewChange = {
  /** Whether it was deleted. */
  deleted?: boolean;
  /** The history group, for revert and Undo; left out when nothing changed. */
  group?: string;
  /** The view after the change (before it, for a deletion). */
  view: SavedView;
};

/** A new saved view. */
export type ViewCreateInput = {
  /** The columns shown, in order. */
  columns?: string[];
  /** What it is for. */
  description?: string | null;
  /** The field rows are grouped by. */
  group_by?: string | null;
  /** Make it the screen's default view. */
  is_default?: boolean;
  /** The screen; default: the list. */
  kind?: ViewKind | null;
  /** Its name, unique among the views of its project and screen. */
  name: string;
  /** The project (key or ID); left out: the whole workspace. */
  project?: string | null;
  /** The query text; it must be valid. */
  query: string;
  /** The order when the query has no `sort:`. */
  sort?: string | null;
  /** The columns totalled. */
  totals?: string[];
};

/** Which view to delete. */
export type ViewDeleteInput = {
  /** With a name: the view's project (key or ID). */
  project?: string | null;
  /** The view: its ID, or its name. */
  view: string;
};

/** A view field that can be cleared. */
export type ViewField = "description" | "columns" | "sort" | "group_by" | "totals";

/** Which screen a view is for. */
export type ViewKind = "list" | "board" | "gantt" | "calendar";

/** Saved views, in order. */
export type ViewList = {
  /** The views. */
  items: SavedView[];
  /** How many. */
  total: number;
};

/** Which views to list. */
export type ViewListInput = {
  /** Every view, of every project and the workspace. */
  all?: boolean;
  /** Only views for this screen. */
  kind?: ViewKind | null;
  /** The views of this project (key or ID); left out: the views of the whole workspace. */
  project?: string | null;
};

/** Changes to a saved view; fields left out keep their value. */
export type ViewUpdateInput = {
  /** Fields to clear. */
  clear?: ViewField[];
  /** New columns. */
  columns?: string[] | null;
  /** A new description. */
  description?: string | null;
  /** A new grouping. */
  group_by?: string | null;
  /** Make it, or stop it being, the screen's default view. */
  is_default?: boolean | null;
  /** A new name. */
  name?: string | null;
  /** With a name: the view's project (key or ID). */
  project?: string | null;
  /** A new query. */
  query?: string | null;
  /** A new order. */
  sort?: string | null;
  /** New totals. */
  totals?: string[] | null;
  /** The view: its ID, or its name. */
  view: string;
};

/** The first day of the week. */
export type WeekStart = "monday" | "saturday" | "sunday";

/** A page changed, with the history group of the change. */
export type WikiChange = {
  /** The history group, for revert and Undo; left out when nothing changed. */
  group?: string;
  /** The page after the change. */
  page: WikiPage;
  /** What became of a new text: saved, merged with another edit, or kept as a conflict copy. */
  save?: DescriptionSave;
};

/** A new page. */
export type WikiCreateInput = {
  /** Place it after this page of the same parent. */
  after?: string | null;
  /** Place it first or last; default: last. */
  at?: Edge | null;
  /** Place it before this page of the same parent. */
  before?: string | null;
  /** The text (Markdown). */
  body?: string | null;
  /** Your own ID for this request; repeating it within 24 hours returns the first answer instead of creating a second page. */
  client_request_id?: string | null;
  /** A note saved with the first revision. */
  message?: string | null;
  /** The page to put it in (title, slug or ID); default: at the top. */
  parent?: string | null;
  /** The project: its key or ID. */
  project: string;
  /** The title, 1 to 200 characters, unique in the project ignoring case, without `[`, `]`, `#` or `|`. */
  title: string;
};

/** Names one page to move to the trash. */
export type WikiDeleteInput = {
  /** The page: its title (any case), slug or ID. */
  page: string;
  /** The project (key or ID); needed unless `page` is an ID. */
  project?: string | null;
};

/** A page field that can be cleared. */
export type WikiField = "parent";

/** A page to read. */
export type WikiGetInput = {
  /** What to add: `backlinks`. */
  include?: WikiPart[];
  /** The page: its title (any case), slug or ID. */
  page: string;
  /** The project (key or ID); needed unless `page` is an ID. */
  project?: string | null;
  /** An older revision to read instead of the current text. */
  revision?: string | null;
};

/** One heading of a page. */
export type WikiHeading = {
  /** 1 to 6. */
  level: number;
  /** The line it starts on, from 1. */
  line: number;
  /** Its text; a link to it is `[[Page#text]]`. */
  text: string;
};

/** A page whose revisions to list. */
export type WikiHistoryInput = {
  /** The page: its title (any case), slug or ID. */
  page: string;
  /** The project (key or ID); needed unless `page` is an ID. */
  project?: string | null;
};

/** A project's pages. */
export type WikiListInput = {
  /** The project: its key or ID. */
  project: string;
};

/** Where a page matched. */
export type WikiMatch = "title" | "heading" | "text" | "recent";

/** One wiki page. */
export type WikiPage = {
  /** Whether it is the project's instructions for agents. */
  agent_brief?: boolean;
  /** What links to it, when asked for with `include`. */
  backlinks?: Backlink[];
  /** Its text (Markdown). */
  body: string;
  /** The pages it holds, in order. */
  children?: WikiPageRef[];
  /** Edits that clashed with another and wait to be resolved (conflict copies). */
  conflict_copies: number;
  /** When it was created (RFC 3339, UTC). */
  created_at: string;
  /** Whether `body` is the current text (false when an older revision was asked for). */
  current: boolean;
  /** The headings of `body`, for links to a heading. */
  headings?: WikiHeading[];
  /** The page's ID. */
  id: string;
  /** The parent page; left out for a page at the top. */
  parent?: WikiPageRef;
  /** The pages it is nested in, outermost first. */
  path?: WikiPageRef[];
  /** Its project's key. */
  project: string;
  /** The revision of `body`; pass it as `base_revision` when you save a new text. Left out while the page has no text yet. */
  revision?: string;
  /** Its name in addresses, made from the title. */
  slug: string;
  /** The page's title. */
  title: string;
  /** When it last changed. */
  updated_at: string;
};

/** Another page, as a page names its parent and children. */
export type WikiPageRef = {
  /** Its ID. */
  id: string;
  /** Its slug. */
  slug: string;
  /** Its title. */
  title: string;
};

/** A page as lists show it. */
export type WikiPageSummary = {
  /** Whether it is the project's instructions for agents. */
  agent_brief?: boolean;
  /** How many pages it holds directly. */
  children: number;
  /** How many pages it is nested in. */
  depth: number;
  /** The page's ID. */
  id: string;
  /** The parent page's ID; left out for a page at the top. */
  parent?: string;
  /** Its project's key. */
  project: string;
  /** Its name in addresses, made from the title. */
  slug: string;
  /** The page's title. */
  title: string;
  /** When it last changed (RFC 3339, UTC). */
  updated_at: string;
};

/** What `wiki.get` adds to a page. */
export type WikiPart = "backlinks";

/** One page found. */
export type WikiSearchHit = {
  /** The heading that matched. */
  heading?: WikiHeading;
  /** The matches in the heading's text, in UTF-16 code units. */
  heading_matches?: TextRange[];
  /** Where it matched. */
  matched_in: WikiMatch;
  /** The page. */
  page: WikiPageSummary;
  /** A piece of the text around the matches, for a text match. */
  snippet?: string;
  /** The matches in `snippet`, in UTF-16 code units. */
  snippet_matches?: TextRange[];
  /** The matches in the page's title, in UTF-16 code units. */
  title_matches?: TextRange[];
};

/** Pages to find, for example to complete a `[[` link. */
export type WikiSearchInput = {
  /** At most this many results, 1 to 200; default 20. */
  limit?: number | null;
  /** Only this project (key or ID); default: every active project. */
  project?: string | null;
  /** Words to find in titles, headings and text. Written `Page#Heading`, it finds the headings of the pages whose titles match `Page`. Empty lists the most recently changed pages. */
  text?: string;
};

/** The pages found, best first: title matches (beginnings first), then headings, then text. */
export type WikiSearchResult = {
  /** The results. */
  items: WikiSearchHit[];
  /** How many results there are, also beyond `limit`. */
  total: number;
};

/** A project's pages as a tree: each page followed by the pages it holds, in their order. */
export type WikiTree = {
  /** The pages, in tree order. */
  items: WikiPageSummary[];
  /** How many. */
  total: number;
};

/** Changes to a page; fields left out keep their value. */
export type WikiUpdateInput = {
  /** Place it after this page of the same parent. */
  after?: string | null;
  /** Place it first or last among its parent's pages. */
  at?: Edge | null;
  /** The revision your new text was edited from. If somebody saved in between, the two edits are merged; when they changed the same lines, yours is kept as a conflict copy and the page stays as it is. */
  base_revision?: string | null;
  /** Place it before this page of the same parent. */
  before?: string | null;
  /** A new text (Markdown), replacing the old one. */
  body?: string | null;
  /** Fields to clear. */
  clear?: WikiField[];
  /** Targeted changes to the text instead of a new one: each `old_text` must occur exactly once. */
  edits?: TextEdit[];
  /** A note saved with the new revision. */
  message?: string | null;
  /** The page: its title (any case), slug or ID. */
  page: string;
  /** A new parent page (title, slug or ID, same project). */
  parent?: string | null;
  /** The project (key or ID); needed unless `page` is an ID. */
  project?: string | null;
  /** Resolve a conflict copy of the text. */
  resolve_conflict?: ConflictResolution | null;
  /** A new title; old links by the old title stop working. */
  title?: string | null;
};

/** No input. */
export type WorkspaceGetInput = Record<string, never>;

/** The open workspace and who is asking. */
export type WorkspaceInfo = {
  /** The limits on agent sessions enforced now, as in the settings; a limit left out is lifted. */
  agent_limits: AgentLimits;
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
  /** The timer, while one runs or is paused. */
  timer?: Timer;
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
  "search.query": { input: SearchInput; output: SearchResult; safety: "read"; kind: "query" };
  "project.list": { input: ProjectListInput; output: ProjectList; safety: "read"; kind: "query" };
  "project.get": { input: ProjectRef; output: Project; safety: "read"; kind: "query" };
  "project.create": { input: ProjectCreateInput; output: ProjectChange; safety: "write"; kind: "command" };
  "project.update": { input: ProjectUpdateInput; output: ProjectChange; safety: "write"; kind: "command" };
  "project.move": { input: ProjectMoveInput; output: ProjectChange; safety: "write"; kind: "command" };
  "project.change_key": { input: ProjectRekeyInput; output: ProjectChange; safety: "write"; kind: "command" };
  "project.archive": { input: ProjectRef; output: ProjectChange; safety: "write"; kind: "command" };
  "project.unarchive": { input: ProjectRef; output: ProjectChange; safety: "write"; kind: "command" };
  "project.delete": { input: ProjectDeleteInput; output: ProjectDeleted; safety: "trash"; kind: "command" };
  "project.brief": { input: ProjectRef; output: AgentBrief; safety: "read"; kind: "query" };
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
  "task.search": { input: TaskSearchInput; output: TaskSearchResult; safety: "read"; kind: "query" };
  "task.get": { input: TaskGetInput; output: TaskList; safety: "read"; kind: "query" };
  "task.create": { input: TaskCreateInput; output: TaskChange; safety: "write"; kind: "command" };
  "task.update": { input: TaskUpdateInput; output: TaskChange; safety: "write"; kind: "command" };
  "task.move": { input: TaskMoveInput; output: TaskChange; safety: "write"; kind: "command" };
  "task.delete": { input: TaskDeleteInput; output: TrashChange; safety: "trash"; kind: "command" };
  "task.bulk_update": { input: BulkUpdateInput; output: BulkResult; safety: "write"; kind: "command" };
  "task.reorder_subtask": { input: SubtaskReorderInput; output: TaskChange; safety: "write"; kind: "command" };
  "task.resolve_conflict": { input: TaskResolveInput; output: TaskChange; safety: "write"; kind: "command" };
  "checklist.update": { input: ChecklistUpdateInput; output: ChecklistChange; safety: "write"; kind: "command" };
  "task.duplicate": { input: TaskDuplicateInput; output: TaskChange; safety: "write"; kind: "command" };
  "task.claim": { input: TaskClaimInput; output: TaskChange; safety: "write"; kind: "command" };
  "task.release": { input: TaskReleaseInput; output: TaskChange; safety: "write"; kind: "command" };
  "relation.add": { input: RelationInput; output: TaskChange; safety: "write"; kind: "command" };
  "relation.remove": { input: RelationInput; output: TaskChange; safety: "write"; kind: "command" };
  "description.history": { input: DescriptionHistoryInput; output: RevisionList; safety: "read"; kind: "query" };
  "description.revision": { input: DescriptionRevisionInput; output: RevisionDetail; safety: "read"; kind: "query" };
  "markdown.render": { input: RenderInput; output: RenderOutput; safety: "read"; kind: "query" };
  "comment.list": { input: CommentListInput; output: CommentList; safety: "read"; kind: "query" };
  "comment.add": { input: CommentAddInput; output: CommentChange; safety: "write"; kind: "command" };
  "comment.update": { input: CommentUpdateInput; output: CommentChange; safety: "write"; kind: "command" };
  "comment.delete": { input: CommentRef; output: CommentChange; safety: "trash"; kind: "command" };
  "time.log": { input: TimeLogInput; output: TimeEntryChange; safety: "write"; kind: "command" };
  "time.update": { input: TimeUpdateInput; output: TimeEntryChange; safety: "write"; kind: "command" };
  "time.delete": { input: TimeEntryRef; output: TimeEntryChange; safety: "trash"; kind: "command" };
  "time.report": { input: TimeReportInput; output: TimeReport; safety: "read"; kind: "query" };
  "timer.get": { input: TimerGetInput; output: TimerStatus; safety: "read"; kind: "query" };
  "timer.start": { input: TimerStartInput; output: TimerChange; safety: "write"; kind: "command" };
  "timer.pause": { input: TimerPauseInput; output: TimerChange; safety: "write"; kind: "command" };
  "timer.stop": { input: TimerStopInput; output: TimerChange; safety: "write"; kind: "command" };
  "reminder.set": { input: ReminderSetInput; output: ReminderChange; safety: "write"; kind: "command" };
  "reminder.clear": { input: ReminderClearInput; output: ReminderChange; safety: "write"; kind: "command" };
  "reminder.due": { input: ReminderDueInput; output: DueReminders; safety: "read"; kind: "query" };
  "reminder.acknowledge": { input: ReminderAckInput; output: ReminderAcknowledged; safety: "write"; kind: "command" };
  "today.get": { input: TodayInput; output: Today; safety: "read"; kind: "query" };
  "board.get": { input: BoardGetInput; output: Board; safety: "read"; kind: "query" };
  "layout.get": { input: LayoutGetInput; output: LayoutChange; safety: "read"; kind: "query" };
  "layout.update": { input: LayoutUpdateInput; output: LayoutChange; safety: "write"; kind: "command" };
  "view.list": { input: ViewListInput; output: ViewList; safety: "read"; kind: "query" };
  "view.create": { input: ViewCreateInput; output: ViewChange; safety: "write"; kind: "command" };
  "view.update": { input: ViewUpdateInput; output: ViewChange; safety: "write"; kind: "command" };
  "view.delete": { input: ViewDeleteInput; output: ViewChange; safety: "write"; kind: "command" };
  "query.parse": { input: QueryParseInput; output: ParsedQuery; safety: "read"; kind: "query" };
  "wiki.list": { input: WikiListInput; output: WikiTree; safety: "read"; kind: "query" };
  "wiki.search": { input: WikiSearchInput; output: WikiSearchResult; safety: "read"; kind: "query" };
  "wiki.get": { input: WikiGetInput; output: WikiPage; safety: "read"; kind: "query" };
  "wiki.create": { input: WikiCreateInput; output: WikiChange; safety: "write"; kind: "command" };
  "wiki.update": { input: WikiUpdateInput; output: WikiChange; safety: "write"; kind: "command" };
  "wiki.delete": { input: WikiDeleteInput; output: TrashChange; safety: "trash"; kind: "command" };
  "wiki.history": { input: WikiHistoryInput; output: RevisionList; safety: "read"; kind: "query" };
  "file.get": { input: FileRef; output: Attachment; safety: "read"; kind: "query" };
  "file.read": { input: FileRef; output: FileContent; safety: "read"; kind: "query" };
  "file.thumbnail": { input: FileRef; output: Thumbnail; safety: "read"; kind: "query" };
  "file.attach": { input: FileAttachInput; output: FileChange; safety: "write"; kind: "command" };
  "file.delete": { input: FileRef; output: FileChange; safety: "trash"; kind: "command" };
  "agent.sessions": { input: AgentSessionsInput; output: AgentSessionList; safety: "read"; kind: "query" };
  "agent.policy.update": { input: AgentPolicyPatch; output: SettingsChange; safety: "app"; kind: "command" };
  "agent.limits.update": { input: AgentLimitsPatch; output: SettingsChange; safety: "app"; kind: "command" };
  "history.feed": { input: ActivityQuery; output: ActivityPage; safety: "read"; kind: "query" };
  "history.revert": { input: RevertInput; output: RevertResult; safety: "write"; kind: "command" };
  "history.undo": { input: UndoInput; output: UndoResult; safety: "app"; kind: "command" };
  "history.redo": { input: UndoInput; output: UndoResult; safety: "app"; kind: "command" };
  "trash.list": { input: TrashListInput; output: TrashList; safety: "read"; kind: "query" };
  "trash.restore": { input: TrashRestoreInput; output: TrashChange; safety: "write"; kind: "command" };
  "trash.purge": { input: TrashPurgeInput; output: TrashChange; safety: "permanent"; kind: "command" };
  "backup.create": { input: BackupCreateInput; output: Backup; safety: "write"; kind: "command" };
  "backup.list": { input: BackupListInput; output: BackupList; safety: "read"; kind: "query" };
  "backup.verify": { input: BackupRef; output: BackupCheck; safety: "read"; kind: "query" };
  "backup.restore": { input: BackupRef; output: BackupRestored; safety: "app"; kind: "command" };
  "backup.delete": { input: BackupRef; output: BackupDeleted; safety: "permanent"; kind: "command" };
  "backup.folder.update": { input: BackupFolderPatch; output: SettingsChange; safety: "app"; kind: "command" };
  "transfer.export": { input: ExportInput; output: ExportResult; safety: "write"; kind: "command" };
  "transfer.import": { input: ImportInput; output: ImportResult; safety: "write"; kind: "command" };
}

/** The name of an operation. */
export type OpId = keyof OpMap;

/** The safety class of every operation. */
export const OP_SAFETY = {
  "workspace.get": "read",
  "settings.get": "read",
  "settings.update": "write",
  "search.query": "read",
  "project.list": "read",
  "project.get": "read",
  "project.create": "write",
  "project.update": "write",
  "project.move": "write",
  "project.change_key": "write",
  "project.archive": "write",
  "project.unarchive": "write",
  "project.delete": "trash",
  "project.brief": "read",
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
  "task.search": "read",
  "task.get": "read",
  "task.create": "write",
  "task.update": "write",
  "task.move": "write",
  "task.delete": "trash",
  "task.bulk_update": "write",
  "task.reorder_subtask": "write",
  "task.resolve_conflict": "write",
  "checklist.update": "write",
  "task.duplicate": "write",
  "task.claim": "write",
  "task.release": "write",
  "relation.add": "write",
  "relation.remove": "write",
  "description.history": "read",
  "description.revision": "read",
  "markdown.render": "read",
  "comment.list": "read",
  "comment.add": "write",
  "comment.update": "write",
  "comment.delete": "trash",
  "time.log": "write",
  "time.update": "write",
  "time.delete": "trash",
  "time.report": "read",
  "timer.get": "read",
  "timer.start": "write",
  "timer.pause": "write",
  "timer.stop": "write",
  "reminder.set": "write",
  "reminder.clear": "write",
  "reminder.due": "read",
  "reminder.acknowledge": "write",
  "today.get": "read",
  "board.get": "read",
  "layout.get": "read",
  "layout.update": "write",
  "view.list": "read",
  "view.create": "write",
  "view.update": "write",
  "view.delete": "write",
  "query.parse": "read",
  "wiki.list": "read",
  "wiki.search": "read",
  "wiki.get": "read",
  "wiki.create": "write",
  "wiki.update": "write",
  "wiki.delete": "trash",
  "wiki.history": "read",
  "file.get": "read",
  "file.read": "read",
  "file.thumbnail": "read",
  "file.attach": "write",
  "file.delete": "trash",
  "agent.sessions": "read",
  "agent.policy.update": "app",
  "agent.limits.update": "app",
  "history.feed": "read",
  "history.revert": "write",
  "history.undo": "app",
  "history.redo": "app",
  "trash.list": "read",
  "trash.restore": "write",
  "trash.purge": "permanent",
  "backup.create": "write",
  "backup.list": "read",
  "backup.verify": "read",
  "backup.restore": "app",
  "backup.delete": "permanent",
  "backup.folder.update": "app",
  "transfer.export": "write",
  "transfer.import": "write",
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
      /** Change the limits on agent sessions: changes a minute, new tasks a day, and items moved to the trash per call and per hour. Each is a number or no limit. (app) */
      limitsUpdate: (input: OpMap["agent.limits.update"]["input"] = {}) => call("agent.limits.update", input),
    },
    backup: {
      /** Make a backup of the workspace now: a checked copy of the database and its stored files. (write) */
      create: (input: OpMap["backup.create"]["input"] = {}) => call("backup.create", input),
      /** The workspace's backups, newest first, with their kind and size. (read) */
      list: (input: OpMap["backup.list"]["input"] = {}) => call("backup.list", input),
      /** Check a backup: the database copy's integrity and every stored file it needs. (read) */
      verify: (input: OpMap["backup.verify"]["input"]) => call("backup.verify", input),
      /** Replace the workspace with a backup, after backing up the current state. Other programs with the workspace open must restart. (app) */
      restore: (input: OpMap["backup.restore"]["input"]) => call("backup.restore", input),
      /** Delete a backup and the stored files only it used. (permanent) */
      delete: (input: OpMap["backup.delete"]["input"]) => call("backup.delete", input),
      /** Move the backup folder to another place, or back to the default; backups already made stay where they are. (app) */
      folderUpdate: (input: OpMap["backup.folder.update"]["input"] = {}) => call("backup.folder.update", input),
    },
    board: {
      /** A project's board as the person sees it: columns in the personal order with hidden ones flagged, WIP state and compact cards. (read) */
      get: (input: OpMap["board.get"]["input"]) => call("board.get", input),
    },
    checklist: {
      /** Tick, clear, add or remove checklist items in a task's description or a comment, by number plus expected text. (write) */
      update: (input: OpMap["checklist.update"]["input"] = {}) => call("checklist.update", input),
    },
    comment: {
      /** A task's comments, oldest first, optionally with their earlier texts. (read) */
      list: (input: OpMap["comment.list"]["input"]) => call("comment.list", input),
      /** Add a Markdown comment to a task, optionally answering another. (write) */
      add: (input: OpMap["comment.add"]["input"]) => call("comment.add", input),
      /** Change a comment's text; the earlier text is kept and can be viewed. (write) */
      update: (input: OpMap["comment.update"]["input"]) => call("comment.update", input),
      /** Move a comment to the trash. (trash) */
      delete: (input: OpMap["comment.delete"]["input"]) => call("comment.delete", input),
    },
    description: {
      /** List the revisions of a task's or project's description, newest first, conflict copies included. (read) */
      history: (input: OpMap["description.history"]["input"] = {}) => call("description.history", input),
      /** One description revision with its text, and the differences from another revision on request. (read) */
      revision: (input: OpMap["description.revision"]["input"]) => call("description.revision", input),
    },
    file: {
      /** An attached file's name, type, size and owner. (read) */
      get: (input: OpMap["file.get"]["input"]) => call("file.get", input),
      /** An attached file's content: text for text files, Base64 for images, otherwise only its details. (read) */
      read: (input: OpMap["file.read"]["input"]) => call("file.read", input),
      /** A small PNG of an attached PNG, JPEG, GIF or WebP image, at most 256 pixels wide and high, from the workspace's cache; made when the image is attached, and again when the cache has lost it. (read) */
      thumbnail: (input: OpMap["file.thumbnail"]["input"]) => call("file.thumbnail", input),
      /** Attach a local file, or Base64 data with a name, to a task or a comment. (write) */
      attach: (input: OpMap["file.attach"]["input"] = {}) => call("file.attach", input),
      /** Move an attached file to the trash. (trash) */
      delete: (input: OpMap["file.delete"]["input"]) => call("file.delete", input),
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
      /** Add a label to a project; without a colour it gets the first palette colour the project's labels don't use yet. (write) */
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
    markdown: {
      /** Render descriptions, comments or editor text to the reader's sanitized HTML, with numbered checkboxes and linked task keys. (read) */
      render: (input: OpMap["markdown.render"]["input"]) => call("markdown.render", input),
    },
    project: {
      /** The project tree in sidebar order, with keys, names and open-task counts; archived projects on request. (read) */
      list: (input: OpMap["project.list"]["input"] = {}) => call("project.list", input),
      /** One project with its statuses (board order, category, WIP limit), task types, labels, modules, sub-projects and counts. (read) */
      get: (input: OpMap["project.get"]["input"]) => call("project.get", input),
      /** Create a project, or a sub-project that copies its parent's statuses, types and labels; without a colour it gets one its sibling projects don't use yet. (write) */
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
      /** Move a project and its sub-projects, with everything in them, to the trash. Agents need the full profile. (trash) */
      delete: (input: OpMap["project.delete"]["input"]) => call("project.delete", input),
      /** The project's instructions for agents: the wiki page marked as the brief, else the project's description. (read) */
      brief: (input: OpMap["project.brief"]["input"]) => call("project.brief", input),
    },
    query: {
      /** Read a task query for the query field: highlighted pieces, the chips of the condition builder, the normal form and the first mistake with its place. (read) */
      parse: (input: OpMap["query.parse"]["input"]) => call("query.parse", input),
    },
    relation: {
      /** Link two tasks (blocks, relates, duplicates, clones, or make one a subtask of the other). (write) */
      add: (input: OpMap["relation.add"]["input"]) => call("relation.add", input),
      /** Remove a link between two tasks, or a subtask from its parent. (write) */
      remove: (input: OpMap["relation.remove"]["input"]) => call("relation.remove", input),
    },
    reminder: {
      /** Set a task's personal reminder: a moment, or a number of days before its due date. (write) */
      set: (input: OpMap["reminder.set"]["input"]) => call("reminder.set", input),
      /** Remove a task's personal reminder. (write) */
      clear: (input: OpMap["reminder.clear"]["input"]) => call("reminder.clear", input),
      /** The reminders, deadline alerts and timer reminders that are due and not acknowledged, with those missed marked, and when the next one comes. (read) */
      due: (input: OpMap["reminder.due"]["input"] = {}) => call("reminder.due", input),
      /** Acknowledge reminders that were shown, so they don't come due again. (write) */
      acknowledge: (input: OpMap["reminder.acknowledge"]["input"]) => call("reminder.acknowledge", input),
    },
    search: {
      /** Search task titles and descriptions, comments, wiki pages and file names for words and fragments, ranked, with the matches marked; a task key finds that task. (read) */
      query: (input: OpMap["search.query"]["input"]) => call("search.query", input),
    },
    settings: {
      /** Read the workspace settings. (read) */
      get: (input: OpMap["settings.get"]["input"] = {}) => call("settings.get", input),
      /** Change workspace settings that are not about agents: trash retention, reminder time, working hours and days, first day of the week, the timer reminder. (write) */
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
      /** List the tasks matching a query (is:open due<+7d label:bug sort:due), a page at a time, with totals of estimates and logged time. (read) */
      search: (input: OpMap["task.search"]["input"] = {}) => call("task.search", input),
      /** One to 20 tasks by key, with their fields, description, labels and version; subtasks, links, comments and files on request. (read) */
      get: (input: OpMap["task.get"]["input"]) => call("task.get", input),
      /** Create a task from a title alone or with its fields; it gets the project's next key. (write) */
      create: (input: OpMap["task.create"]["input"]) => call("task.create", input),
      /** Change a task's title, description (whole, with a base revision, or by targeted edits), type, priority, labels, parent, dates, estimate or progress. (write) */
      update: (input: OpMap["task.update"]["input"]) => call("task.update", input),
      /** Move a task to another status or place in its column, or to another project (new key; the old one forwards). (write) */
      move: (input: OpMap["task.move"]["input"]) => call("task.move", input),
      /** Move tasks to the trash, each with its subtasks; they can be restored from there. (trash) */
      delete: (input: OpMap["task.delete"]["input"]) => call("task.delete", input),
      /** Make the same change to many tasks chosen by key or query (status, type, priority, labels, parent, dates set or shifted, estimate, project), previewed first, as one change for Undo and revert. (write) */
      bulkUpdate: (input: OpMap["task.bulk_update"]["input"]) => call("task.bulk_update", input),
      /** Move a subtask to another place among its parent's subtasks (before or after another one, first or last); returns the parent with its subtasks in the new order. (write) */
      reorderSubtask: (input: OpMap["task.reorder_subtask"]["input"]) => call("task.reorder_subtask", input),
      /** Resolve a conflict copy of a task's description: keep the description, use the copy, or save a combined text. (write) */
      resolveConflict: (input: OpMap["task.resolve_conflict"]["input"]) => call("task.resolve_conflict", input),
      /** Copy a task with its subtasks (in order), labels, checklist and files; the copy is linked as a clone. (write) */
      duplicate: (input: OpMap["task.duplicate"]["input"]) => call("task.duplicate", input),
      /** Claim a task for your agent session for a while, so other agents leave it alone; moves it into progress. (write) */
      claim: (input: OpMap["task.claim"]["input"]) => call("task.claim", input),
      /** Give up a claim on a task, optionally moving it and adding a comment. (write) */
      release: (input: OpMap["task.release"]["input"]) => call("task.release", input),
    },
    time: {
      /** Log time on a task: a duration, the day of the work (default today) and a note. (write) */
      log: (input: OpMap["time.log"]["input"]) => call("time.log", input),
      /** Change a time entry's duration, day or note. (write) */
      update: (input: OpMap["time.update"]["input"]) => call("time.update", input),
      /** Move a time entry to the trash. (trash) */
      delete: (input: OpMap["time.delete"]["input"]) => call("time.delete", input),
      /** Logged time over a range of days, grouped by day, task or project, with totals in the workspace's units. (read) */
      report: (input: OpMap["time.report"]["input"] = {}) => call("time.report", input),
    },
    timer: {
      /** The workspace's timer: its task, whether it runs and the time so far. (read) */
      get: (input: OpMap["timer.get"]["input"] = {}) => call("timer.get", input),
      /** Start the timer on a task, or start the paused timer again; a timer on another task is stopped and its time logged. (write) */
      start: (input: OpMap["timer.start"]["input"] = {}) => call("timer.start", input),
      /** Pause the running timer; it keeps its time. (write) */
      pause: (input: OpMap["timer.pause"]["input"] = {}) => call("timer.pause", input),
      /** Stop the timer and log its time as an entry on its task. (write) */
      stop: (input: OpMap["timer.stop"]["input"] = {}) => call("timer.stop", input),
    },
    today: {
      /** What needs attention today across projects: overdue tasks, tasks due today, tasks in progress, today's reminders, agents' claims, recent changes and the timer, with counts for the sidebar. (read) */
      get: (input: OpMap["today.get"]["input"] = {}) => call("today.get", input),
    },
    transfer: {
      /** Export projects, or the whole workspace, to an archive of JSON and Markdown files with the attachments. (write) */
      export: (input: OpMap["transfer.export"]["input"] = {}) => call("transfer.export", input),
      /** Import an archive as new projects; checks it completely first and never changes existing data. (write) */
      import: (input: OpMap["transfer.import"]["input"]) => call("transfer.import", input),
    },
    trash: {
      /** The trash: deleted items with what went with them, who deleted them and when they will be purged, newest first. (read) */
      list: (input: OpMap["trash.list"]["input"] = {}) => call("trash.list", input),
      /** Bring items back out of the trash, with everything that was deleted with them. (write) */
      restore: (input: OpMap["trash.restore"]["input"]) => call("trash.restore", input),
      /** Delete items in the trash for good, and their text from the history. Can't be undone. (permanent) */
      purge: (input: OpMap["trash.purge"]["input"] = {}) => call("trash.purge", input),
    },
    type: {
      /** Add a task type to a project. (write) */
      create: (input: OpMap["type.create"]["input"]) => call("type.create", input),
      /** Rename, reshape, recolour or reorder a task type, or make it the default. (write) */
      update: (input: OpMap["type.update"]["input"]) => call("type.update", input),
      /** Delete a task type; its tasks get another type. Can't be undone. (permanent) */
      delete: (input: OpMap["type.delete"]["input"]) => call("type.delete", input),
    },
    view: {
      /** The saved views of a project or of the whole workspace: name, query, columns, order, grouping, totals and which is the default. (read) */
      list: (input: OpMap["view.list"]["input"] = {}) => call("view.list", input),
      /** Save a query as a named view of a project or of the whole workspace, with its columns, order, grouping and totals. (write) */
      create: (input: OpMap["view.create"]["input"]) => call("view.create", input),
      /** Rename a saved view, change its query, columns, order, grouping or totals, or make it the default. (write) */
      update: (input: OpMap["view.update"]["input"]) => call("view.update", input),
      /** Delete a saved view; it can be brought back with revert. (write) */
      delete: (input: OpMap["view.delete"]["input"]) => call("view.delete", input),
    },
    wiki: {
      /** A project's wiki pages as a tree: titles, slugs, nesting and when each changed. (read) */
      list: (input: OpMap["wiki.list"]["input"]) => call("wiki.list", input),
      /** Find wiki pages by title, heading or text, for example to link one; `Page#Heading` lists a page's headings. (read) */
      search: (input: OpMap["wiki.search"]["input"] = {}) => call("wiki.search", input),
      /** One wiki page with its text, revision, headings, place in the tree and, on request, what links to it; or an older revision of it. (read) */
      get: (input: OpMap["wiki.get"]["input"]) => call("wiki.get", input),
      /** Create a wiki page with a title unique in its project, optionally inside another page and with its text. (write) */
      create: (input: OpMap["wiki.create"]["input"]) => call("wiki.create", input),
      /** Change a wiki page: its text (whole, with a base revision, or by targeted edits), title, parent or place; or resolve a conflict copy. (write) */
      update: (input: OpMap["wiki.update"]["input"]) => call("wiki.update", input),
      /** Move a wiki page, with the pages inside it, to the trash; links to it stop resolving until it is restored. (trash) */
      delete: (input: OpMap["wiki.delete"]["input"]) => call("wiki.delete", input),
      /** List the revisions of a wiki page, newest first, conflict copies included; read one with wiki.get and its revision. (read) */
      history: (input: OpMap["wiki.history"]["input"]) => call("wiki.history", input),
    },
    workspace: {
      /** The open workspace: its name and ID, the schema and program versions, who is asking with which rights, the settings, the limits on agents and a short guide. (read) */
      get: (input: OpMap["workspace.get"]["input"] = {}) => call("workspace.get", input),
    },
  };
}

/** The typed operations. */
export type Ops = ReturnType<typeof makeOps>;
