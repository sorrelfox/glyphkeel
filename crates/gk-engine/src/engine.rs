// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! [`Engine`]: one open workspace in one process, and the pipeline every
//! operation runs through.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use gk_api::ops::backup::BackupKind;
use gk_api::ops::common::Via;
use gk_api::ops::history::RevertResult;
use gk_api::{
    Changed, ErrorCode, OpDescriptor, OpError, OpKind, Operation, Profile, Safety, descriptor,
};
use gk_core::Id;
use gk_store::rusqlite::params;
use gk_store::{
    CheckpointOutcome, CheckpointPolicy, CheckpointSchedule, Checkpointer, LockFile, RequestKey,
    SCHEMA_VERSION, SchemaState, Store, StoreConfig, WatcherHandle, WorkspaceLayout,
};
use serde_json::{Value, json};

use crate::backup::{BackupFolder, default_folder};
use crate::context::{ConfirmRequest, Confirmation, Context};
use crate::events::{self, Bus};
use crate::failure::{Failure, store_error};
use crate::messages;
use crate::ops::{history as history_ops, proposal as proposal_ops, settings};
use crate::registry::{CommandFn, Entry, Env, Handler, QueryCtx, QueryFn, Registry};
use crate::time::{Clock, system_clock};
use crate::write::{CommandMeta, WriteCtx};

/// Which program opens the workspace. It decides the locks the engine
/// takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The desktop app: holds the app lock (one app per workspace) and
    /// tries for the jobs lock.
    App,
    /// A one-shot `glyphkeel` command: no long-held lock.
    Cli,
    /// `glyphkeel mcp`: tries for the jobs lock when no app holds it.
    Mcp,
}

/// How the engine opens a workspace.
#[derive(Clone)]
pub struct EngineConfig {
    /// The program opening it.
    pub role: Role,
    /// The storage settings.
    pub store: StoreConfig,
    /// The program's version, as `workspace.get` reports it.
    pub program_version: String,
    /// The clock.
    pub clock: Clock,
    /// The handlers; [`Registry::standard`] unless a test adds its own.
    pub registry: Registry,
    /// When the write-ahead log is checkpointed by the jobs-lock holder.
    pub checkpoints: CheckpointPolicy,
    /// The time zone of "today", reminder times and the days of time
    /// entries, by its IANA name; `None`: the computer's. Programs pass the
    /// person's choice in ([`crate::personal_time_zone`]).
    pub time_zone: Option<String>,
    /// The folder of this workspace's backups, ahead of the workspace's
    /// `backup_folder` setting; `None`: that setting, else the default
    /// ([`crate::backup::default_folder`]).
    pub backups: Option<std::path::PathBuf>,
    /// The folders agents may name files in, besides the workspace's
    /// `exports` folder; by default none. The programs pass
    /// [`crate::AgentFolders::Personal`], so that tests and other embedders
    /// never read the person's own settings file.
    pub agent_folders: crate::AgentFolders,
    /// The limits attached files are read within for search; tests make
    /// them tighter.
    pub file_text_limits: gk_extract::Limits,
}

impl EngineConfig {
    /// The defaults for `role`.
    #[must_use]
    pub fn new(role: Role) -> Self {
        Self {
            role,
            store: StoreConfig::default(),
            program_version: env!("CARGO_PKG_VERSION").to_owned(),
            clock: system_clock(),
            registry: Registry::standard(),
            checkpoints: CheckpointPolicy::default(),
            time_zone: None,
            backups: None,
            agent_folders: crate::AgentFolders::Fixed(Vec::new()),
            file_text_limits: gk_extract::Limits::default(),
        }
    }
}

impl std::fmt::Debug for EngineConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineConfig")
            .field("role", &self.role)
            .field("store", &self.store)
            .field("program_version", &self.program_version)
            .finish_non_exhaustive()
    }
}

/// How many groups the app's Undo stack keeps.
const UNDO_DEPTH: usize = 200;
/// How often a session's count of refused reads is written while its agent
/// keeps being refused.
const LIMITED_WRITE_EVERY: Duration = Duration::from_secs(2);

/// An agent's change that touches more items than this is preceded by a
/// safety backup of the workspace.
pub(crate) const SAFETY_BACKUP_ABOVE: usize = 50;

/// What an agent session records when it starts.
#[derive(Debug, Clone)]
pub struct SessionStart {
    /// The agent's display name.
    pub name: String,
    /// The client's own ID, if it sent one.
    pub client_id: Option<String>,
    /// The client's version, if it sent one.
    pub client_version: Option<String>,
    /// The profile it asked for.
    pub profile: Profile,
}

/// How long one turn of the background jobs reads attached files' text in
/// the app, and in the MCP server.
const FILE_TEXT_TURN_APP: Duration = Duration::from_secs(3);
const FILE_TEXT_TURN_MCP: Duration = Duration::from_millis(300);

/// One open workspace. Every operation of every adapter runs through
/// [`Engine::call`]:
///
/// 1. find the operation; refuse when a newer program upgraded the schema;
/// 2. decode the input (`invalid` with the JSON path);
/// 3. authorize: app-only operations, the agent's profile capped by the
///    workspace setting, the confirmation of permanent operations;
/// 4. queries run in one read transaction; commands in one write
///    transaction with their history rows, under a group registered before
///    the commit;
/// 5. after the commit, publish one change event and, for the app, push
///    the group onto the Undo stack.
pub struct Engine {
    pub(crate) store: Store,
    pub(crate) env: Env,
    pub(crate) role: Role,
    pub(crate) clock: Clock,
    registry: Registry,
    bus: Arc<Bus>,
    /// Held across a command's commit and its event, so events leave in
    /// commit order.
    commit: Mutex<()>,
    pub(crate) upgraded: AtomicBool,
    undo: Mutex<UndoStacks>,
    /// Held while proposals are approved.
    approvals: Mutex<()>,
    pending_reads: Mutex<HashMap<Id, i64>>,
    /// Reads refused by each session's read limits, not yet recorded, and
    /// when the session's count was last written.
    pending_limited: Mutex<HashMap<Id, (i64, Option<Instant>)>>,
    jobs: Mutex<Jobs>,
    /// Where backups go when neither the configuration nor the workspace's
    /// setting names a folder, or the one the configuration fixes.
    pub(crate) backups: BackupFolder,
    /// Whether the configuration fixed the folder, so the setting is not
    /// read.
    pub(crate) backups_fixed: bool,
    /// When this process last ran the daily jobs (engine clock, ms).
    pub(crate) last_daily: Mutex<Option<i64>>,
    /// When this process last ran the scheduled automation rules (engine
    /// clock, ms).
    pub(crate) last_automation: Mutex<Option<i64>>,
    /// The attached file whose text is being read for search, across the
    /// turns of the background jobs.
    pub(crate) extracting: Mutex<crate::file_text::Extractions>,
    /// The limits attached files are read within.
    pub(crate) file_text_limits: gk_extract::Limits,
    _app_lock: Option<LockFile>,
    /// Shared: the workspace is in use; see `WorkspaceLayout::open_lock`.
    _open_lock: LockFile,
}

#[derive(Debug, Default)]
struct UndoStacks {
    undo: Vec<Id>,
    redo: Vec<Id>,
}

struct Jobs {
    lock: Option<LockFile>,
    checkpoints: Option<(Checkpointer, CheckpointSchedule)>,
    policy: CheckpointPolicy,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("workspace", &self.env.name)
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

impl Engine {
    /// Opens the workspace at `layout`, creating it if it doesn't exist,
    /// and takes the locks of `config.role`.
    pub fn open(layout: &WorkspaceLayout, config: EngineConfig) -> Result<Self, OpError> {
        layout.create_dirs().map_err(|e| store_error(&e))?;
        // Shared with every other process using the workspace; a rename
        // waits for all of them to let go.
        let open_lock = LockFile::try_acquire_shared(&layout.open_lock())
            .map_err(|e| store_error(&e))?
            .ok_or_else(|| {
                messages::hint(
                    messages::error(ErrorCode::Busy, "engine-workspace-renaming", &[]),
                    "engine-workspace-renaming-hint",
                    &[],
                )
            })?;
        let app_lock = if config.role == Role::App {
            let lock = LockFile::try_acquire(&layout.app_lock()).map_err(|e| store_error(&e))?;
            Some(lock.ok_or_else(|| {
                messages::hint(
                    messages::error(ErrorCode::Busy, "engine-app-open", &[]),
                    "engine-app-open-hint",
                    &[],
                )
            })?)
        } else {
            None
        };
        let backups_fixed = config.backups.is_some();
        let backups = BackupFolder::new(
            config
                .backups
                .clone()
                .unwrap_or_else(|| default_folder(layout)),
        );
        // An encrypted workspace opens with a passphrase this process
        // knows, the environment's or the one saved in the keyring; the
        // caller asks the person when none is there.
        gk_store::unlock_without_asking(&layout.database()).map_err(|e| store_error(&e))?;
        let mut store_config = config.store.clone();
        let mut before_upgrade = None;
        if config.role == Role::App
            && let SchemaState::Older(found) = Store::probe(layout).map_err(|e| store_error(&e))?
        {
            // The setting is read straight from the old database, which
            // may not open yet.
            let chosen = if backups_fixed {
                None
            } else {
                Store::probe_setting(layout, crate::backup::SETTING)
                    .ok()
                    .flatten()
                    .and_then(|text| serde_json::from_str::<Option<String>>(&text).ok())
                    .flatten()
            };
            let folder = chosen.map_or_else(|| backups.clone(), BackupFolder::new);
            let made = folder.create(
                layout,
                gk_api::ops::backup::BackupKind::PreMigration,
                (config.clock)(),
                &config.program_version,
            )?;
            tracing::info!(name = %made.name, found, "workspace backed up before the schema upgrade");
            store_config.allow_upgrade = true;
            before_upgrade = Some((folder, made));
        }
        let store = match Store::open(layout, store_config) {
            Ok(store) => store,
            Err(e) => {
                return Err(match before_upgrade {
                    Some((folder, made)) => put_back_after_failed_upgrade(
                        layout,
                        &folder,
                        &made.name,
                        &e,
                        (config.clock)(),
                    ),
                    None => store_error(&e),
                });
            }
        };
        crate::ops::links::index_pending(&store);
        let jobs_lock = if config.role == Role::Cli {
            None
        } else {
            LockFile::try_acquire(&layout.jobs_lock()).map_err(|e| store_error(&e))?
        };
        let name = layout
            .root()
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let env = Env {
            workspace_id: store.workspace_id(),
            name,
            program_version: config.program_version,
            schema_version: SCHEMA_VERSION,
            layout: layout.clone(),
            zone: crate::time::Zone::new(config.time_zone.as_deref()),
            agent_folders: config.agent_folders,
        };
        Ok(Self {
            store,
            env,
            role: config.role,
            clock: config.clock,
            registry: config.registry,
            bus: Arc::new(Bus::default()),
            commit: Mutex::new(()),
            upgraded: AtomicBool::new(false),
            undo: Mutex::new(UndoStacks::default()),
            approvals: Mutex::new(()),
            pending_reads: Mutex::new(HashMap::new()),
            pending_limited: Mutex::new(HashMap::new()),
            jobs: Mutex::new(Jobs {
                lock: jobs_lock,
                checkpoints: None,
                policy: config.checkpoints,
            }),
            backups,
            backups_fixed,
            last_daily: Mutex::new(None),
            last_automation: Mutex::new(None),
            extracting: Mutex::new(crate::file_text::Extractions::default()),
            file_text_limits: config.file_text_limits,
            _app_lock: app_lock,
            _open_lock: open_lock,
        })
    }

    /// The storage layer, for diagnostics and tests.
    #[must_use]
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The open workspace's facts.
    #[must_use]
    pub fn env(&self) -> &Env {
        &self.env
    }

    /// The time zone of "today", reminder times and the days of time
    /// entries, by its IANA name; `None` when the zone has no name (a
    /// system zone given only by its rules).
    #[must_use]
    pub fn time_zone(&self) -> Option<String> {
        self.env.zone.name()
    }

    /// Changes the time zone while the workspace is open, for a person who
    /// chooses another one: `"system"` (or blank) for the computer's own,
    /// or an IANA name. Operations that start afterwards use it.
    ///
    /// # Errors
    /// `invalid` when the name is no known zone; the zone stays as it was.
    pub fn set_time_zone(&self, name: &str) -> Result<(), OpError> {
        let zone = crate::time::check_time_zone(name)?;
        self.env.zone.set(zone.as_deref());
        Ok(())
    }

    /// Whether a newer program upgraded the schema while this one ran. The
    /// agent server exits when it sees this; every later call fails with
    /// `workspace_upgraded`.
    #[must_use]
    pub fn must_exit(&self) -> bool {
        self.upgraded.load(Ordering::Acquire)
    }

    /// A receiver of every change event from now on: this process's
    /// commands, and other processes' once [`Engine::watch_other_processes`]
    /// runs.
    #[must_use]
    pub fn subscribe(&self) -> Receiver<Changed> {
        self.bus.subscribe()
    }

    /// Runs operation `op` with JSON `input` for `ctx`. `null` input counts
    /// as `{}`.
    pub fn call(&self, ctx: &Context, op: &str, input: Value) -> Result<Value, OpError> {
        let entry = self.registry.get(op).ok_or_else(|| {
            let names = self.registry.names();
            messages::with("engine-no-operation", &[("op", op.to_owned())], |m| {
                OpError::not_found(m, &names)
            })
        })?;
        let result = self.call_entry(ctx, entry, input);
        if let Err(e) = &result
            && e.code == ErrorCode::WorkspaceUpgraded
        {
            self.upgraded.store(true, Ordering::Release);
        }
        result
    }

    /// Runs operation `O` with typed input and output.
    pub fn run<O: Operation>(&self, ctx: &Context, input: &O::Input) -> Result<O::Output, OpError> {
        let input = serde_json::to_value(input).map_err(|e| {
            messages::error(
                ErrorCode::Invalid,
                "engine-input-unencodable",
                &[("error", e.to_string())],
            )
        })?;
        let output = self.call(ctx, O::ID, input)?;
        serde_json::from_value(output).map_err(|e| {
            messages::storage(
                "engine-result-unreadable",
                &[("op", O::ID.to_owned()), ("error", e.to_string())],
            )
        })
    }

    fn call_entry(&self, ctx: &Context, entry: &Entry, input: Value) -> Result<Value, OpError> {
        if self.must_exit() {
            return Err(messages::hint(
                messages::error(
                    ErrorCode::WorkspaceUpgraded,
                    "engine-upgraded-meanwhile",
                    &[],
                ),
                "engine-restart-hint",
                &[],
            ));
        }
        let input = if input.is_null() { json!({}) } else { input };
        let input = crate::scope::narrow_input(ctx, entry.desc.id, input);
        (entry.validate)(&input)?;
        // Every path an agent names passes here, whatever the operation.
        let input = crate::agent_files::confine(
            ctx,
            &self.env.layout,
            &self.env.agent_folders,
            entry.desc.id,
            input,
        )?;
        let parts: Vec<&'static OpDescriptor> = entry
            .parts
            .map(|parts| parts(&input).into_iter().filter_map(descriptor).collect())
            .unwrap_or_default();
        let (profile, proposing) = self.authorize(ctx, entry.desc, &parts)?;
        self.confirm(ctx, entry.desc, &parts, &input)?;
        if proposing {
            return Err(match self.propose(ctx, entry, input) {
                Ok(pending) | Err(pending) => pending,
            });
        }

        match &entry.handler {
            Handler::Query(f) => self.run_query(ctx, profile, f, input),
            Handler::Command(f) => {
                let request = request_key(entry.desc, ctx, &input);
                let f = if ctx.approved.is_some() {
                    proposal_ops::settling(f)
                } else {
                    Arc::clone(f)
                };
                self.execute_once(ctx, entry.desc.id, f, input, request)
                    .map(|(output, _)| output)
            }
            Handler::Engine(f) => {
                if ctx.approved.is_none() {
                    return f(self, ctx, input);
                }
                // Claimed first, so no other program runs it too; given
                // back when it fails, marked approved when it ran.
                proposal_ops::claim(self, ctx)?;
                match f(self, ctx, input) {
                    Ok(output) => {
                        proposal_ops::settle_after(self, ctx, &output)?;
                        Ok(output)
                    }
                    Err(e) => {
                        if let Err(again) = proposal_ops::release(self, ctx, &e) {
                            tracing::warn!(error = %again.message, "a failed approval stays claimed");
                        }
                        Err(e)
                    }
                }
            }
        }
    }

    /// Keeps an agent's change as a proposal for the person instead of
    /// making it: `Ok` with the `pending_approval` answer, or why it can't
    /// be proposed (its input or the status rules refuse it).
    fn propose(&self, ctx: &Context, entry: &Entry, input: Value) -> Result<OpError, OpError> {
        let desc = entry.desc;
        let request = request_key(desc, ctx, &input).map(|(caller, id)| format!("{caller} {id}"));
        let handler = match &entry.handler {
            Handler::Command(f) => Some(Arc::clone(f)),
            Handler::Query(_) | Handler::Engine(_) => None,
        };
        let f: CommandFn = Arc::new(move |w, _| {
            proposal_ops::create(w, desc, handler.as_ref(), &input, request.as_deref())
        });
        let ctx = ctx.clone().proposing();
        let (output, _) = self.execute(&ctx, "proposal.create", f, Value::Null)?;
        let id = output["id"].as_str().unwrap_or_default();
        Ok(proposal_ops::pending(id))
    }

    /// Serializes approvals in this process, so one proposal never runs
    /// twice side by side.
    pub(crate) fn lock_approvals(&self) -> MutexGuard<'_, ()> {
        self.approvals
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Puts `group` on the Undo stack, for a change the person made in the
    /// app through another caller (an approved proposal).
    pub(crate) fn push_undo(&self, group: Id) {
        let mut stacks = self.lock_undo();
        stacks.undo.push(group);
        if stacks.undo.len() > UNDO_DEPTH {
            stacks.undo.remove(0);
        }
        stacks.redo.clear();
    }

    /// Who may run `desc` (and each of the `parts` it runs), the agent's
    /// effective profile, and whether the agent's change waits for the
    /// person's approval instead of running.
    fn authorize(
        &self,
        ctx: &Context,
        desc: &OpDescriptor,
        parts: &[&'static OpDescriptor],
    ) -> Result<(Option<Profile>, bool), OpError> {
        if desc.is_app_only() {
            let reachable = match ctx.via {
                Via::App => !ctx.is_agent(),
                Via::Cli => !ctx.is_agent() && desc.cli.is_some(),
                Via::Mcp => false,
            };
            if !reachable {
                return Err(messages::error(
                    ErrorCode::Forbidden,
                    "op-app-only",
                    &[("op", desc.id.to_owned())],
                ));
            }
        }
        if !ctx.is_agent() {
            return Ok((None, false));
        }
        let settings = self
            .store
            .read(|tx| settings::load(tx).map_err(Failure::from))
            .map_err(OpError::from)?;
        let cap = settings.agent_max_profile;
        let profile = ctx.agent_profile_under(cap);
        // A change the person allowed when asked, or one carrying out an
        // approved proposal, runs; any other change the approvals setting
        // covers for this profile is kept as a proposal, and a read-only
        // agent then proposes what a standard one may do.
        let riskiest = std::iter::once(desc)
            .chain(parts.iter().copied())
            .map(|d| d.safety)
            .max()
            .unwrap_or(desc.safety);
        let proposing = desc.kind == OpKind::Command
            && !ctx.person_confirmed
            && ctx.approved.is_none()
            && !ctx.proposing
            && settings.agent_approvals.mode(profile).covers(riskiest);
        let profile = if proposing {
            ctx.clone().proposing().agent_profile_under(cap)
        } else {
            profile
        };
        let refused = std::iter::once(desc)
            .chain(parts.iter().copied())
            .find(|d| !profile.allows(d.safety));
        if let Some(refused) = refused {
            let refusal = messages::forbidden(
                "engine-profile-refused",
                &[
                    ("profile", profile.to_string()),
                    ("safety", refused.safety.as_str().to_owned()),
                    ("op", refused.id.to_owned()),
                ],
            );
            let hint = if profile < cap {
                "engine-profile-raise-hint"
            } else {
                "engine-profile-capped-hint"
            };
            Err(messages::hint(refusal, hint, &[]).with_detail("profile", profile.as_str()))
        } else {
            Ok((Some(profile), proposing))
        }
    }

    /// Permanent operations by the person need a confirmation; agents are
    /// governed by their profile instead.
    fn confirm(
        &self,
        ctx: &Context,
        desc: &'static OpDescriptor,
        parts: &[&'static OpDescriptor],
        input: &Value,
    ) -> Result<(), OpError> {
        let permanent = std::iter::once(desc)
            .chain(parts.iter().copied())
            .find(|d| d.safety == Safety::Permanent);
        let Some(permanent) = permanent else {
            return Ok(());
        };
        if ctx.is_agent() {
            return Ok(());
        }
        self.ask_confirmation(ctx, desc, permanent, input)
    }

    /// Asks the person to confirm `desc` (for its part `what`) as the
    /// context says: refused, given, or by asking.
    pub(crate) fn ask_confirmation(
        &self,
        ctx: &Context,
        desc: &'static OpDescriptor,
        what: &'static OpDescriptor,
        input: &Value,
    ) -> Result<(), OpError> {
        let permanent = what;
        let confirmed = match &ctx.confirm {
            Confirmation::Refuse => false,
            Confirmation::Given => true,
            Confirmation::Ask(ask) => ask(&ConfirmRequest {
                op: desc,
                input: input.clone(),
            }),
        };
        if confirmed {
            Ok(())
        } else {
            Err(messages::hint(
                messages::forbidden("engine-not-confirmed", &[("op", permanent.id.to_owned())]),
                "engine-not-confirmed-hint",
                &[],
            ))
        }
    }

    fn run_query(
        &self,
        ctx: &Context,
        profile: Option<Profile>,
        f: &QueryFn,
        input: Value,
    ) -> Result<Value, OpError> {
        let output = self.store.read(|tx| {
            let _scope = crate::scope::enter(tx, ctx)?;
            f(
                &QueryCtx {
                    tx,
                    caller: ctx,
                    profile,
                    env: &self.env,
                    now: (self.clock)(),
                },
                input,
            )
        })?;
        if let Some(session) = ctx.session {
            *self.lock_reads().entry(session).or_insert(0) += 1;
        }
        Ok(output)
    }

    /// Runs one command: one write transaction with its history rows, then
    /// the change event. Returns the output and the group, when something
    /// changed.
    pub(crate) fn execute(
        &self,
        ctx: &Context,
        op: &'static str,
        f: CommandFn,
        input: Value,
    ) -> Result<(Value, Option<Id>), OpError> {
        self.execute_once(ctx, op, f, input, None)
    }

    /// Runs `f` as the command `desc` with `input`, answering a repeated
    /// `client_request_id` as a catalog command would. For operations that
    /// prepare work outside the transaction first.
    pub(crate) fn execute_command(
        &self,
        ctx: &Context,
        desc: &'static OpDescriptor,
        f: CommandFn,
        input: Value,
    ) -> Result<Value, OpError> {
        let request = request_key(desc, ctx, &input);
        self.execute_once(ctx, desc.id, f, input, request)
            .map(|(output, _)| output)
    }

    /// As [`Engine::execute`]; with a request key, a repeat of the same
    /// request within a day, from any process, gets the first answer back
    /// without running again.
    fn execute_once(
        &self,
        ctx: &Context,
        op: &'static str,
        f: CommandFn,
        input: Value,
        request: Option<(String, String)>,
    ) -> Result<(Value, Option<Id>), OpError> {
        let at = (self.clock)();
        let group = Id::generate_at(u64::try_from(at).unwrap_or(0));
        let meta = CommandMeta {
            op,
            group,
            at,
            ctx: ctx.clone(),
            workspace_id: self.env.workspace_id,
            layout: self.env.layout.clone(),
            zone: self.env.zone.clone(),
            as_agent: false,
        };
        let reads = ctx
            .session
            .and_then(|s| self.lock_reads().remove(&s))
            .unwrap_or(0);

        let order = self.commit.lock().unwrap_or_else(PoisonError::into_inner);
        // Registered before the commit, so the change watcher never reports
        // this process's own command as another program's.
        self.store.own_groups().insert(group);
        // An agent's large change runs twice: the first run counts the
        // items, rolls back and asks for a safety backup; the second, after
        // the backup, commits.
        let mut safety_backup = !ctx.is_agent();
        let result = loop {
            let attempt = self.write_command(
                op,
                &meta,
                &f,
                &input,
                request.as_ref(),
                reads,
                safety_backup,
            );
            match attempt {
                Err(Failure::SnapshotFirst { items }) => {
                    if let Err(e) = self.create_backup(BackupKind::Safety) {
                        if let (Some(session), true) = (ctx.session, reads > 0) {
                            *self.lock_reads().entry(session).or_insert(0) += reads;
                        }
                        return Err(messages::hint(
                            messages::error(
                                e.code,
                                "backup-safety-failed",
                                &[("count", items.to_string()), ("error", e.message)],
                            ),
                            "backup-safety-failed-hint",
                            &[],
                        ));
                    }
                    safety_backup = true;
                }
                other => break other,
            }
        };

        let (output, finished) = match result {
            Ok(done) => done,
            Err(e) => {
                if let (Some(session), true) = (ctx.session, reads > 0) {
                    *self.lock_reads().entry(session).or_insert(0) += reads;
                }
                return Err(e.into());
            }
        };
        let Some(event) = finished.event(&meta) else {
            return Ok((output, None));
        };
        self.bus.publish(&event);
        drop(order);
        if ctx.via == Via::App
            && !ctx.is_agent()
            && !op.starts_with("history.")
            && !op.starts_with("proposal.")
            && op != "automation.run"
        {
            let mut stacks = self.lock_undo();
            stacks.undo.push(group);
            if stacks.undo.len() > UNDO_DEPTH {
                stacks.undo.remove(0);
            }
            stacks.redo.clear();
        }
        Ok((output, Some(group)))
    }

    /// One run of a command in a write transaction; see
    /// [`Engine::execute_once`]. Without `safety_backup`, a change touching
    /// more than [`SAFETY_BACKUP_ABOVE`] items is rolled back with
    /// [`Failure::SnapshotFirst`].
    #[allow(clippy::too_many_arguments)]
    fn write_command(
        &self,
        op: &'static str,
        meta: &CommandMeta,
        f: &CommandFn,
        input: &Value,
        request: Option<&(String, String)>,
        reads: i64,
        safety_backup: bool,
    ) -> Result<(Value, crate::write::Finished), Failure> {
        let in_tx = meta.clone();
        let f = Arc::clone(f);
        let input = input.clone();
        let request = request.cloned();
        self.store.write(op, move |tx| {
            let key = request.as_ref().map(|(caller, id)| RequestKey {
                op,
                caller,
                request_id: id,
            });
            if let Some(key) = key
                && let Some(answer) = gk_store::remembered(tx, key, in_tx.at)?
            {
                let answer = serde_json::from_str(&answer).map_err(|e| {
                    Failure::Op(messages::storage(
                        "storage-json",
                        &[("error", e.to_string())],
                    ))
                })?;
                return Ok((answer, crate::write::Finished::nothing()));
            }
            let _scope = crate::scope::enter(tx, &in_tx.ctx)?;
            let mut w = WriteCtx::new(tx, &in_tx);
            let mut output = f(&mut w, input.clone())?;
            let followed = crate::ops::recurrence::after_command(&mut w)?;
            crate::ops::recurrence::update_output(w.tx(), &mut output, &followed)?;
            crate::ops::rules::enforce(&mut w)?;
            let events = crate::ops::automation::events_of(&mut w)?;
            let mut finished = w.finish()?;
            crate::ops::automation::follow(tx, &in_tx, events, &mut finished)?;
            if !safety_backup {
                let items = finished.items();
                if items > SAFETY_BACKUP_ABOVE {
                    return Err(Failure::SnapshotFirst { items });
                }
            }
            if let Some(key) = key {
                gk_store::remember(tx, key, &output.to_string(), in_tx.at)?;
            }
            if let (Some(session), true) = (in_tx.ctx.session, reads > 0) {
                tx.execute(
                    "UPDATE agent_sessions SET reads = reads + ?2, last_seen_at = ?3 WHERE id = ?1",
                    params![session.as_bytes().as_slice(), reads, in_tx.at],
                )?;
            }
            Ok::<_, Failure>((output, finished))
        })
    }

    /// Reverts `groups` (newest first) as one command named `op`.
    pub(crate) fn revert(
        &self,
        ctx: &Context,
        op: &'static str,
        groups: Vec<Id>,
        dry_run: bool,
    ) -> Result<RevertResult, OpError> {
        let f: CommandFn = Arc::new(move |w, _| {
            let result = history_ops::revert_groups(w, &groups, dry_run)?;
            serde_json::to_value(result).map_err(|e| {
                Failure::Op(messages::storage(
                    "storage-json",
                    &[("error", e.to_string())],
                ))
            })
        });
        let (output, _) = self.execute(ctx, op, f, Value::Null)?;
        serde_json::from_value(output)
            .map_err(|e| messages::storage("storage-json", &[("error", e.to_string())]))
    }

    /// Takes the last group off the Undo stack (or the Redo stack) and
    /// reverts it. A group that something else changed since is refused
    /// with the reason and dropped from the stack.
    pub(crate) fn undo_or_redo(
        &self,
        ctx: &Context,
        redo: bool,
    ) -> Result<(Id, Id, String), OpError> {
        let op = if redo { "history.redo" } else { "history.undo" };
        let target = {
            let mut stacks = self.lock_undo();
            let stack = if redo {
                &mut stacks.redo
            } else {
                &mut stacks.undo
            };
            stack.pop()
        }
        .ok_or_else(|| {
            messages::not_found(
                if redo {
                    "history-nothing-to-redo"
                } else {
                    "history-nothing-to-undo"
                },
                &[],
            )
        })?;
        let original_op = self
            .store
            .read(|tx| history_ops::group_op(tx, target).map_err(Failure::from))
            .map_err(OpError::from)?
            .unwrap_or_default();
        let result = self.revert(ctx, op, vec![target], false)?;
        if let Some(skipped) = result.skipped.first() {
            let id = if redo {
                "history-redo-blocked"
            } else {
                "history-undo-blocked"
            };
            return Err(
                messages::conflict(id, &[("reason", skipped.reason.clone())])
                    .with_detail("group", target.to_string()),
            );
        }
        let group: Id = result
            .group
            .as_deref()
            .and_then(|g| g.parse().ok())
            .ok_or_else(|| messages::storage("history-revert-no-group", &[]))?;
        let mut stacks = self.lock_undo();
        if redo {
            stacks.undo.push(group);
        } else {
            stacks.redo.push(group);
        }
        Ok((target, group, original_op))
    }

    /// Starts an agent session and returns its ID.
    pub fn start_session(&self, start: &SessionStart) -> Result<Id, OpError> {
        let at = (self.clock)();
        let id = Id::generate_at(u64::try_from(at).unwrap_or(0));
        let start = start.clone();
        let pid = i64::from(std::process::id());
        self.store
            .write("agent.session_start", move |tx| {
                tx.execute(
                    "INSERT INTO agent_sessions (id, client_id, client_version, display_name,
                                                 profile, pid, started_at, last_seen_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                    params![
                        id.as_bytes().as_slice(),
                        start.client_id,
                        start.client_version,
                        start.name,
                        start.profile.as_str(),
                        pid,
                        at
                    ],
                )?;
                Ok::<_, Failure>(())
            })
            .map_err(OpError::from)?;
        Ok(id)
    }

    /// Counts a read that `session`'s read limits refused. The count is
    /// written at once, then at most every [`LIMITED_WRITE_EVERY`] while
    /// the agent keeps being refused, and when the session ends.
    pub fn note_rate_limited(&self, session: Id) {
        let now = Instant::now();
        let due = {
            let mut pending = self.lock_limited();
            let entry = pending.entry(session).or_insert((0, None));
            entry.0 += 1;
            if entry
                .1
                .is_none_or(|t| now.saturating_duration_since(t) >= LIMITED_WRITE_EVERY)
            {
                let count = entry.0;
                *entry = (0, Some(now));
                Some(count)
            } else {
                None
            }
        };
        if let Some(count) = due {
            self.write_limited(session, count);
        }
    }

    fn write_limited(&self, session: Id, count: i64) {
        if count == 0 {
            return;
        }
        let written = self.store.write("agent.session_limited", move |tx| {
            tx.execute(
                "UPDATE agent_sessions SET rate_limited = rate_limited + ?2 WHERE id = ?1",
                params![session.as_bytes().as_slice(), count],
            )?;
            Ok::<_, Failure>(())
        });
        if let Err(e) = written.map_err(OpError::from) {
            tracing::warn!(session = %session, error = %e, "counting refused reads failed");
        }
    }

    /// Writes the refused reads not yet recorded for `session`.
    fn flush_limited(&self, session: Id) {
        let count = self.lock_limited().remove(&session).map_or(0, |(n, _)| n);
        self.write_limited(session, count);
    }

    /// Ends an agent session: records the end time and the reads not yet
    /// counted.
    pub fn end_session(&self, session: Id) -> Result<(), OpError> {
        self.flush_limited(session);
        let at = (self.clock)();
        let reads = self.lock_reads().remove(&session).unwrap_or(0);
        self.store
            .write("agent.session_end", move |tx| {
                tx.execute(
                    "UPDATE agent_sessions SET reads = reads + ?2, ended_at = ?3, last_seen_at = ?3
                      WHERE id = ?1",
                    params![session.as_bytes().as_slice(), reads, at],
                )?;
                Ok::<_, Failure>(())
            })
            .map_err(OpError::from)
    }

    /// Publishes other processes' commits as change events, polling every
    /// `interval`, until the handle is dropped.
    pub fn watch_other_processes(&self, interval: Duration) -> Result<WatcherHandle, OpError> {
        let bus = Arc::clone(&self.bus);
        self.store
            .watch_changes(None)
            .and_then(|w| {
                w.spawn(interval, move |batch| match batch {
                    Ok(batch) => {
                        for event in events::from_records(&batch.records) {
                            bus.publish(&event);
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "reading other programs' changes failed"),
                })
            })
            .map_err(|e| store_error(&e))
    }

    /// Whether this process holds the jobs lock (checkpoints, backups,
    /// purging the trash).
    #[must_use]
    pub fn holds_jobs_lock(&self) -> bool {
        self.lock_jobs().lock.is_some()
    }

    /// One round of background work, called every few seconds by
    /// long-running programs: takes the jobs lock when it is free (another
    /// holder may have ended) and, while holding it, checkpoints the
    /// write-ahead log when it is due, runs the daily jobs (backup,
    /// trash purge, unused files) when they are due, every minute the
    /// automation rules set off by due dates and the schedule, and reads
    /// the text inside attached files that wait for it.
    pub fn jobs_tick(&self) -> Result<Option<CheckpointOutcome>, OpError> {
        if self.role == Role::Cli {
            return Ok(None);
        }
        let mut jobs = self.lock_jobs();
        if jobs.lock.is_none() {
            jobs.lock = LockFile::try_acquire(&self.store.layout().jobs_lock())
                .map_err(|e| store_error(&e))?;
            if jobs.lock.is_none() {
                return Ok(None);
            }
        }
        if jobs.checkpoints.is_none() {
            let checkpointer = self.store.checkpointer().map_err(|e| store_error(&e))?;
            jobs.checkpoints = Some((checkpointer, CheckpointSchedule::new(jobs.policy)));
        }
        let idle_for = self
            .store
            .writer_stats()
            .last_commit_at
            .map_or(Duration::MAX, |t| t.elapsed());
        let Some((checkpointer, schedule)) = jobs.checkpoints.as_mut() else {
            return Ok(None);
        };
        let outcome = schedule
            .tick(Instant::now(), idle_for, checkpointer)
            .map_err(|e| store_error(&e));
        drop(jobs);
        if self.daily_jobs_due()
            && let Err(e) = self.run_daily_jobs()
        {
            tracing::warn!(error = %e, "the daily jobs failed");
        }
        if self.automation_due()
            && let Err(e) = self.run_automation()
        {
            tracing::warn!(error = %e, "the scheduled automation rules failed");
        }
        // The app's jobs have a thread of their own; the MCP server's share
        // its loop with the requests, so they take a short turn.
        let budget = if self.role == Role::App {
            FILE_TEXT_TURN_APP
        } else {
            FILE_TEXT_TURN_MCP
        };
        if let Err(e) = self.index_files(budget) {
            tracing::warn!(error = %e, "reading the text inside attached files failed");
        }
        outcome
    }

    /// Closes the workspace: counts the open sessions' reads, checkpoints
    /// the log if this process holds the jobs lock, and stops the writer.
    pub fn close(self) {
        let limited: Vec<Id> = self.lock_limited().keys().copied().collect();
        for session in limited {
            self.flush_limited(session);
        }
        let sessions: Vec<Id> = self.lock_reads().keys().copied().collect();
        for session in sessions {
            let reads = self.lock_reads().remove(&session).unwrap_or(0);
            let _ = self.store.write("agent.session_reads", move |tx| {
                tx.execute(
                    "UPDATE agent_sessions SET reads = reads + ?2 WHERE id = ?1",
                    params![session.as_bytes().as_slice(), reads],
                )?;
                Ok::<_, Failure>(())
            });
        }
        let jobs = self
            .jobs
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        if jobs.lock.is_some()
            && let Ok(checkpointer) = self.store.checkpointer()
            && let Err(e) = checkpointer.truncate()
        {
            tracing::debug!(error = %e, "checkpoint on close failed");
        }
        self.store.close();
    }

    fn lock_reads(&self) -> MutexGuard<'_, HashMap<Id, i64>> {
        self.pending_reads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_limited(&self) -> MutexGuard<'_, HashMap<Id, (i64, Option<Instant>)>> {
        self.pending_limited
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_undo(&self) -> MutexGuard<'_, UndoStacks> {
        self.undo.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_jobs(&self) -> MutexGuard<'_, Jobs> {
        self.jobs.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Commands that carry a `client_request_id` answer a repeat of the same ID
/// by the same caller with the first answer instead of running again. The
/// caller is the agent session, or the program and the person.
fn request_key(desc: &OpDescriptor, ctx: &Context, input: &Value) -> Option<(String, String)> {
    if desc.kind != OpKind::Command {
        return None;
    }
    let id = input
        .get("client_request_id")?
        .as_str()
        .filter(|id| !id.is_empty())?;
    let caller = ctx.session.map_or_else(
        || format!("{}:{}", ctx.via.as_str(), ctx.actor.name),
        |s| s.to_string(),
    );
    Some((caller, id.to_owned()))
}

/// Puts the workspace back from the backup `name` made just before a schema
/// upgrade that failed with `failure`, and explains what happened. The
/// workspace is then exactly as the older program left it.
fn put_back_after_failed_upgrade(
    layout: &WorkspaceLayout,
    folder: &BackupFolder,
    name: &str,
    failure: &gk_store::StoreError,
    now: i64,
) -> OpError {
    tracing::error!(error = %failure, backup = name, "the schema upgrade failed");
    let failed = store_error(failure);
    let args = [("name", name.to_owned()), ("error", failed.message.clone())];
    let put_back = (|| -> Result<(), OpError> {
        let (path, _) = folder.database_of(name)?;
        let _migrate = LockFile::acquire(&layout.migrate_lock()).map_err(|e| store_error(&e))?;
        let copy = layout.root().join(format!(".upgrade-undo-{now}.db"));
        let result = (|| {
            std::fs::copy(&path, &copy).map_err(|e| {
                messages::error(
                    ErrorCode::Storage,
                    "backup-copy-failed",
                    &[
                        ("path", path.display().to_string()),
                        ("error", e.to_string()),
                    ],
                )
            })?;
            gk_store::put_back_before_upgrade(layout, &copy, UPGRADE_UNDO_BUSY_BUDGET)
                .map_err(|e| store_error(&e))
        })();
        let _ = std::fs::remove_file(&copy);
        let _ = std::fs::remove_file(copy.with_extension("db-journal"));
        result
    })();
    match put_back {
        Ok(()) => {
            tracing::info!(
                backup = name,
                "the workspace was put back as it was before the upgrade"
            );
            messages::hint(
                messages::error(ErrorCode::Storage, "engine-upgrade-failed-restored", &args),
                "engine-upgrade-failed-restored-hint",
                &[],
            )
            .with_detail("backup", name.to_owned())
            .with_detail("restored", true)
        }
        Err(e) => {
            tracing::error!(error = %e.message, backup = name, "the workspace could not be put back");
            messages::hint(
                messages::error(
                    ErrorCode::Storage,
                    "engine-upgrade-failed-not-restored",
                    &args,
                ),
                "engine-upgrade-failed-not-restored-hint",
                &args,
            )
            .with_detail("backup", name.to_owned())
            .with_detail("restored", false)
        }
    }
}

/// How long putting the workspace back after a failed upgrade waits for
/// other programs' locks.
const UPGRADE_UNDO_BUSY_BUDGET: Duration = Duration::from_secs(10);
