// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The open workspace: the engine, the change events it forwards to the
//! windows, the watcher for other programs' commits and the background
//! jobs.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::thread::JoinHandle;
use std::time::Duration;

use gk_api::ops::common::Via;
use gk_api::{Changed, ErrorCode, OpError};
use gk_engine::{Confirmation, Context, Engine, EngineConfig, system_clock};
use gk_store::{WatcherHandle, WorkspaceLayout};
use serde_json::Value;
use tauri::ipc::Channel;

use super::windows::Windows;
use crate::reminders::{self, Notifier};

/// How often other programs' commits are looked for.
const WATCH_INTERVAL: Duration = Duration::from_millis(250);
/// How often the background jobs get a turn.
const JOBS_INTERVAL: Duration = Duration::from_secs(5);

/// The workspace the app has open, or why it couldn't open it. Every call
/// of the page goes through [`Workspace::call`].
#[derive(Debug)]
pub(crate) struct Workspace {
    engine: Mutex<Option<Arc<Engine>>>,
    /// The workspace folder, for the stored files the app hands out.
    layout: Option<WorkspaceLayout>,
    /// Why the workspace isn't open; every call answers with it.
    failure: Option<OpError>,
    windows: Arc<Windows>,
    confirm: Confirmation,
    background: Mutex<Option<Background>>,
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
        let engine = match Engine::open(layout, config) {
            Ok(engine) => Arc::new(engine),
            Err(err) => return Self::failed(err),
        };
        let windows = Arc::new(Windows::default());
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
        Self {
            engine: Mutex::new(Some(engine)),
            layout: Some(layout.clone()),
            failure: None,
            windows,
            confirm,
            background: Mutex::new(Some(Background {
                _watcher: watcher,
                stop,
                jobs,
                reminders: None,
            })),
        }
    }

    /// No workspace: every call answers with `failure`.
    pub(crate) fn failed(failure: OpError) -> Self {
        Self {
            engine: Mutex::new(None),
            layout: None,
            failure: Some(failure),
            windows: Arc::new(Windows::default()),
            confirm: Confirmation::Refuse,
            background: Mutex::new(None),
        }
    }

    /// Why the workspace isn't open, if it isn't.
    pub(crate) fn failure(&self) -> Option<&OpError> {
        self.failure.as_ref()
    }

    /// The workspace folder, while it is open.
    pub(crate) fn layout(&self) -> Option<&WorkspaceLayout> {
        self.layout.as_ref()
    }

    /// Runs `op` for the person through the app. Blocks: call it from a
    /// blocking thread.
    pub(crate) fn call(
        &self,
        op: &str,
        input: Value,
        request_id: Option<String>,
    ) -> Result<Value, OpError> {
        let engine = self.engine()?;
        let mut ctx = Context::user(Via::App).with_confirmation(self.confirm.clone());
        ctx.request_id = request_id;
        engine.call(&ctx, op, input)
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
        let Some(engine) = lock(&self.engine).take() else {
            return;
        };
        match Arc::try_unwrap(engine) {
            Ok(engine) => engine.close(),
            Err(_) => eprintln!(
                "glyphkeel-app: calls were still running; the workspace closes without a checkpoint"
            ),
        }
    }

    fn engine(&self) -> Result<Arc<Engine>, OpError> {
        if let Some(engine) = lock(&self.engine).as_ref() {
            return Ok(Arc::clone(engine));
        }
        Err(self.failure.clone().unwrap_or_else(|| {
            OpError::new(ErrorCode::Storage, "The workspace is closed.")
                .with_hint("The app is quitting.")
        }))
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
