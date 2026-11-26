// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The open workspace: the engine, the change events it forwards to the
//! windows, the watcher for other programs' commits and the background
//! jobs. After a restore from a backup the engine that ran it must not go
//! on, so the workspace closes it and opens the restored data afresh. When
//! the person opens another workspace, the same happens with the other
//! folder ([`Workspace::switch`]).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::thread::JoinHandle;
use std::time::Duration;

use gk_api::Profile;
use gk_api::ops::agent::{AgentLimits, AgentLimitsPatch};
use gk_api::ops::common::{Actor, ActorKind, Via};
use gk_api::{Changed, EntityChange, ErrorCode, OpError, Origin};
use gk_engine::{ConfirmRequest, Confirmation, Context, Engine, EngineConfig, system_clock};
use gk_store::{WatcherHandle, WorkspaceLayout};
use serde_json::Value;
use tauri::ipc::Channel;

use super::windows::Windows;
use crate::messages::text;
use crate::reminders::{self, Notifier};

/// How often other programs' commits are looked for.
const WATCH_INTERVAL: Duration = Duration::from_millis(250);
/// How often the background jobs get a turn.
const JOBS_INTERVAL: Duration = Duration::from_secs(5);
/// How long a reopen waits for calls still running on the old engine.
const REOPEN_WAIT: Duration = Duration::from_secs(10);
/// The operation after which the engine must be replaced.
const RESTORE_OP: &str = "backup.restore";
/// The operation that sets the most an agent may do.
const POLICY_OP: &str = "agent.policy.update";
/// The operation that sets the limits on agent sessions.
const LIMITS_OP: &str = "agent.limits.update";

/// Whether going from `current` to `wanted` lets agents do more.
fn raises(current: Profile, wanted: Profile) -> bool {
    wanted > current
}

/// The workspace the app has open, or why it couldn't open it. Every call
/// of the page goes through [`Workspace::call`].
#[derive(Debug)]
pub(crate) struct Workspace {
    engine: Mutex<Option<Arc<Engine>>>,
    /// The workspace folder, for the stored files the app hands out.
    layout: Mutex<Option<WorkspaceLayout>>,
    /// Why the workspace isn't open; every call answers with it.
    failure: Mutex<Option<OpError>>,
    windows: Arc<Windows>,
    confirm: Confirmation,
    background: Mutex<Option<Background>>,
    /// What opening the workspace again needs.
    reopen: Mutex<Reopen>,
    /// Why the workspace stayed closed after a restore, if it did.
    lost: Mutex<Option<OpError>>,
    /// Whether opening it made it: it didn't exist before this start.
    created: bool,
}

/// How the workspace was opened, to open it the same way again.
#[derive(Default)]
struct Reopen {
    config: Option<EngineConfig>,
    notifier: Option<Arc<dyn reminders::Notifier>>,
}

impl std::fmt::Debug for Reopen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reopen")
            .field("config", &self.config.is_some())
            .field("notifier", &self.notifier.is_some())
            .finish()
    }
}

#[derive(Debug)]
struct Background {
    _watcher: Option<WatcherHandle>,
    stop: Arc<AtomicBool>,
    jobs: Option<JoinHandle<()>>,
    reminders: Option<reminders::Running>,
}

impl Workspace {
    /// Opens the workspace at `layout`, creating it on the first run, and
    /// starts forwarding its change events. `confirm` is how the person
    /// confirms a `permanent` operation: the native dialog in the app.
    pub(crate) fn open(
        layout: &WorkspaceLayout,
        config: EngineConfig,
        confirm: Confirmation,
    ) -> Self {
        let windows = Arc::new(Windows::default());
        let created = !layout.database().exists();
        let (engine, background) = match start(layout, config.clone(), &windows) {
            Ok(started) => started,
            Err(err) => return Self::failed_with(err, Some(config), confirm),
        };
        Self {
            engine: Mutex::new(Some(engine)),
            layout: Mutex::new(Some(layout.clone())),
            failure: Mutex::new(None),
            windows,
            confirm,
            background: Mutex::new(Some(background)),
            reopen: Mutex::new(Reopen {
                config: Some(config),
                notifier: None,
            }),
            lost: Mutex::new(None),
            created,
        }
    }

    /// No workspace: every call answers with `failure`, until the person
    /// opens another workspace, which opens with `config` and `confirm`.
    pub(crate) fn failed_with(
        failure: OpError,
        config: Option<EngineConfig>,
        confirm: Confirmation,
    ) -> Self {
        Self {
            engine: Mutex::new(None),
            layout: Mutex::new(None),
            failure: Mutex::new(Some(failure)),
            windows: Arc::new(Windows::default()),
            confirm,
            background: Mutex::new(None),
            reopen: Mutex::new(Reopen {
                config,
                notifier: None,
            }),
            lost: Mutex::new(None),
            created: false,
        }
    }

    /// Closes the engine and opens the workspace again, the way it was
    /// opened, in the time zone the person chose since; the reminders start
    /// again too. Every window then hears that everything changed. Calls
    /// still running on the old engine get a moment to end first.
    pub(crate) fn reopen(&self) -> Result<(), OpError> {
        let Some(layout) = self.layout() else {
            return Ok(());
        };
        self.open_at(&layout)
    }

    /// Closes the workspace and opens the one at `layout` in its place,
    /// creating it when it doesn't exist yet. When that fails, the one that
    /// was open opens again, and the error says why.
    pub(crate) fn switch(&self, layout: &WorkspaceLayout) -> Result<(), OpError> {
        self.closed_for(|_| Ok(layout.clone()))
    }

    /// Closes the workspace, runs `change` on the folder that was open
    /// (`None` when none was), and opens the folder it returns. When
    /// `change` or the opening fails, the folder that was open opens again,
    /// and the error is `change`'s or the opening's.
    pub(crate) fn closed_for(
        &self,
        change: impl FnOnce(Option<&WorkspaceLayout>) -> Result<WorkspaceLayout, OpError>,
    ) -> Result<(), OpError> {
        let before = self.layout();
        self.close_within(REOPEN_WAIT);
        let result = change(before.as_ref()).and_then(|layout| {
            self.open_at(&layout)?;
            *lock(&self.layout) = Some(layout);
            *lock(&self.failure) = None;
            Ok(())
        });
        if let Err(err) = result {
            if let Some(before) = before
                && lock(&self.engine).is_none()
            {
                let _ = self.open_at(&before);
            }
            return Err(err);
        }
        Ok(())
    }

    /// Opens the engine at `layout`, the way the workspace was first
    /// opened, in the time zone the person chose since, and tells every
    /// window that everything changed.
    fn open_at(&self, layout: &WorkspaceLayout) -> Result<(), OpError> {
        let (config, notifier) = {
            let reopen = lock(&self.reopen);
            (reopen.config.clone(), reopen.notifier.clone())
        };
        let Some(mut config) = config else {
            return Err(self
                .failure()
                .unwrap_or_else(|| OpError::new(ErrorCode::Storage, "The workspace is closed.")));
        };
        if let Some(zone) = self.time_zone() {
            config.time_zone = Some(zone);
        }
        self.close_within(REOPEN_WAIT);
        match start(layout, config, &self.windows) {
            Ok((engine, background)) => {
                *lock(&self.engine) = Some(engine);
                *lock(&self.background) = Some(background);
                *lock(&self.lost) = None;
                if let Some(notifier) = notifier {
                    self.start_reminders(notifier);
                }
                self.windows.send(&reopened());
                Ok(())
            }
            Err(err) => {
                *lock(&self.lost) = Some(err.clone());
                Err(err)
            }
        }
    }

    /// Why the workspace isn't open, if it isn't.
    pub(crate) fn failure(&self) -> Option<OpError> {
        lock(&self.failure)
            .clone()
            .or_else(|| lock(&self.lost).clone())
    }

    /// Whether this start made the workspace, so it is the first run on it.
    pub(crate) fn created(&self) -> bool {
        self.created
    }

    /// The workspace folder, while it is open.
    pub(crate) fn layout(&self) -> Option<WorkspaceLayout> {
        lock(&self.layout).clone()
    }

    /// Runs `op` for the person through the app. Blocks: call it from a
    /// blocking thread.
    pub(crate) fn call(
        &self,
        op: &str,
        input: Value,
        request_id: Option<String>,
    ) -> Result<Value, OpError> {
        let result = {
            let engine = self.engine()?;
            if op == POLICY_OP {
                self.confirm_raise(&engine, &input)?;
            }
            if op == LIMITS_OP {
                self.confirm_limits(&engine, &input)?;
            }
            let mut ctx = Context::user(Via::App).with_confirmation(self.confirm.clone());
            ctx.request_id = request_id;
            engine.call(&ctx, op, input)
        };
        // The engine that restored a backup refuses every later call: the
        // workspace opens again, on the restored data.
        if op == RESTORE_OP
            && result.is_ok()
            && let Err(err) = self.reopen()
        {
            eprintln!(
                "glyphkeel-app: the restored workspace could not be opened again: {}",
                err.message
            );
        }
        result
    }

    /// Raising the most an agent may do needs the person's native
    /// confirmation, so that script in the page can't widen what agents
    /// may do. Lowering it, or setting it again, doesn't ask.
    fn confirm_raise(&self, engine: &Engine, input: &Value) -> Result<(), OpError> {
        let Some(wanted) = input
            .get("max_profile")
            .and_then(Value::as_str)
            .and_then(|name| name.parse::<Profile>().ok())
        else {
            // The engine refuses the input with its own message.
            return Ok(());
        };
        let settings = engine.call(
            &Context::user(Via::App),
            "settings.get",
            Value::Object(serde_json::Map::new()),
        )?;
        let current = settings
            .get("agent_max_profile")
            .and_then(Value::as_str)
            .and_then(|name| name.parse::<Profile>().ok())
            .unwrap_or(Profile::ReadOnly);
        if !raises(current, wanted) {
            return Ok(());
        }
        self.ask(POLICY_OP, input, "confirm-agent-raise-refused")
    }

    /// Raising a limit on agents, or lifting one, needs the person's native
    /// confirmation too, for the same reason. Lowering doesn't ask.
    fn confirm_limits(&self, engine: &Engine, input: &Value) -> Result<(), OpError> {
        let Ok(patch) = serde_json::from_value::<AgentLimitsPatch>(input.clone()) else {
            // The engine refuses the input with its own message.
            return Ok(());
        };
        let settings = engine.call(
            &Context::user(Via::App),
            "settings.get",
            Value::Object(serde_json::Map::new()),
        )?;
        let current = settings
            .get("agent_limits")
            .and_then(|limits| serde_json::from_value::<AgentLimits>(limits.clone()).ok())
            .unwrap_or_default();
        if !patch.apply(current).allow_more_than(&current) {
            return Ok(());
        }
        self.ask(LIMITS_OP, input, "confirm-agent-limits-refused")
    }

    /// Asks the person natively about `op`; `refused` is the message when
    /// they don't confirm.
    fn ask(&self, op: &str, input: &Value, refused: &str) -> Result<(), OpError> {
        let confirmed = match (&self.confirm, gk_api::descriptor(op)) {
            (Confirmation::Given, _) => true,
            (Confirmation::Ask(ask), Some(op)) => ask(&ConfirmRequest {
                op,
                input: input.clone(),
            }),
            _ => false,
        };
        if confirmed {
            Ok(())
        } else {
            Err(OpError::forbidden(text(refused, &[])))
        }
    }

    /// The time zone the engine counts "today" and local times in, by its
    /// IANA name; `None` while no workspace is open or the system's zone
    /// has no name.
    pub(crate) fn time_zone(&self) -> Option<String> {
        self.engine().ok().and_then(|engine| engine.time_zone())
    }

    /// Switches the engine to the person's newly chosen zone (`"system"`
    /// or an IANA name), and wakes the reminders so that they count their
    /// times in it at once. Without an open workspace the name is only
    /// checked.
    pub(crate) fn set_time_zone(&self, name: &str) -> Result<(), OpError> {
        match self.engine() {
            Ok(engine) => {
                engine.set_time_zone(name)?;
                self.wake_reminders();
                Ok(())
            }
            Err(_) => gk_engine::check_time_zone(name).map(|_| ()),
        }
    }

    /// Makes the reminders look again now, if they are running.
    pub(crate) fn wake_reminders(&self) {
        if let Some(reminders) = lock(&self.background)
            .as_ref()
            .and_then(|background| background.reminders.as_ref())
        {
            reminders.wake();
        }
    }

    /// Sets the change-event channel of window `label`; see
    /// [`Windows::subscribe`].
    pub(crate) fn subscribe(&self, label: &str, channel: Channel<Changed>) -> bool {
        self.windows.subscribe(label, channel)
    }

    /// Forgets the channel of a closed window.
    pub(crate) fn unsubscribe(&self, label: &str) {
        self.windows.unsubscribe(label);
    }

    /// Starts showing reminders through `notifier`, until the workspace
    /// closes. Runs whether or not a window is open.
    pub(crate) fn start_reminders(&self, notifier: Arc<dyn Notifier>) {
        lock(&self.reopen).notifier = Some(Arc::clone(&notifier));
        let Ok(engine) = self.engine() else {
            return;
        };
        let mut background = lock(&self.background);
        let Some(background) = background.as_mut() else {
            return;
        };
        if background.reminders.is_some() {
            return;
        }
        background.reminders = reminders::start(
            Arc::downgrade(&engine),
            engine.subscribe(),
            system_clock(),
            notifier,
            reminders::LONGEST_WAIT,
        );
    }

    /// Stops the background work and closes the engine, checkpointing the
    /// log. Calls still running keep the engine alive; then it closes when
    /// the last one ends, without the checkpoint.
    pub(crate) fn close(&self) {
        self.close_within(Duration::ZERO);
    }

    /// Closes as [`Workspace::close`] does, waiting up to `wait` for calls
    /// still running to end, so that the engine closes cleanly and lets go
    /// of the workspace.
    fn close_within(&self, wait: Duration) {
        if let Some(mut background) = lock(&self.background).take() {
            if let Some(reminders) = background.reminders.take() {
                reminders.stop();
            }
            background.stop.store(true, Ordering::Release);
            if let Some(jobs) = background.jobs {
                jobs.thread().unpark();
                let _ = jobs.join();
            }
        }
        let Some(mut engine) = lock(&self.engine).take() else {
            return;
        };
        let deadline = std::time::Instant::now() + wait;
        loop {
            match Arc::try_unwrap(engine) {
                Ok(engine) => return engine.close(),
                Err(shared) if std::time::Instant::now() < deadline => {
                    engine = shared;
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => {
                    eprintln!(
                        "glyphkeel-app: calls were still running; the workspace closes without a checkpoint"
                    );
                    return;
                }
            }
        }
    }

    fn engine(&self) -> Result<Arc<Engine>, OpError> {
        if let Some(engine) = lock(&self.engine).as_ref() {
            return Ok(Arc::clone(engine));
        }
        Err(self.failure().unwrap_or_else(|| {
            OpError::new(ErrorCode::Storage, "The workspace is closed.")
                .with_hint("The app is quitting.")
        }))
    }
}

/// Opens the engine at `layout` and starts its background work: the
/// events forwarded to `windows`, the watcher for other programs' commits
/// and the jobs.
fn start(
    layout: &WorkspaceLayout,
    config: EngineConfig,
    windows: &Arc<Windows>,
) -> Result<(Arc<Engine>, Background), OpError> {
    let engine = Arc::new(Engine::open(layout, config)?);
    windows.forward(engine.subscribe());
    let watcher = engine
        .watch_other_processes(WATCH_INTERVAL)
        .inspect_err(|err| {
            eprintln!(
                "glyphkeel-app: other programs' changes won't show until a restart: {}",
                err.message
            );
        })
        .ok();
    let stop = Arc::new(AtomicBool::new(false));
    let jobs = run_jobs(Arc::downgrade(&engine), Arc::clone(&stop));
    Ok((
        engine,
        Background {
            _watcher: watcher,
            stop,
            jobs,
            reminders: None,
        },
    ))
}

/// The event that tells every window the whole workspace changed: it was
/// restored from a backup and opened again.
fn reopened() -> Changed {
    Changed {
        seq: 0,
        group: String::new(),
        origin: Origin {
            via: Via::App,
            actor: Actor {
                kind: ActorKind::User,
                name: String::new(),
            },
            session: None,
        },
        changes: vec![EntityChange {
            entity: "workspace".to_owned(),
            id: "restored".to_owned(),
            project: None,
            fields: vec!["*".to_owned()],
        }],
    }
}

/// Gives the background jobs (taking over the jobs lock, checkpoints) a turn
/// every few seconds until `stop` is set or the engine is gone.
fn run_jobs(engine: Weak<Engine>, stop: Arc<AtomicBool>) -> Option<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("gk-app-jobs".into())
        .spawn(move || {
            loop {
                std::thread::park_timeout(JOBS_INTERVAL);
                if stop.load(Ordering::Acquire) {
                    break;
                }
                let Some(engine) = engine.upgrade() else {
                    break;
                };
                if let Err(err) = engine.jobs_tick() {
                    eprintln!("glyphkeel-app: background jobs: {}", err.message);
                }
            }
        })
        .inspect_err(|err| eprintln!("glyphkeel-app: cannot start the background jobs: {err}"))
        .ok()
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
