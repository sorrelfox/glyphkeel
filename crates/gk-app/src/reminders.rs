// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The reminder scheduler: shows what comes due as desktop notifications,
//! whether or not a window is open.
//!
//! It runs in the app's process, which alone opens the workspace in the app's
//! role (the app lock), so reminders show once even with agents and the
//! command line at work. Each round asks the engine which reminders are due,
//! shows them, acknowledges them, and sleeps until the next one comes due.
//! Due dates, reminders, settings and the timer move that moment, in this
//! process or another, so every change event starts a new round.
//!
//! The first round passes the time the app started as `since`: what came
//! due while the app was closed shows as missed.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Weak};
use std::thread::JoinHandle;
use std::time::Duration;

use gk_api::OpError;
use gk_api::ops::common::Via;
use gk_api::ops::time::{
    DueReminder, ReminderAckInput, ReminderAcknowledge, ReminderDue, ReminderDueInput, ReminderKind,
};
use gk_engine::{Clock, Context, Engine, rfc3339};

/// One desktop notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Notice {
    pub(crate) title: String,
    pub(crate) body: String,
    /// The task "Open" brings forward.
    pub(crate) task: Option<String>,
}

/// Where notices go: the desktop's notification server, or a test's list.
pub(crate) trait Notifier: Send + Sync {
    /// Shows `notice`. A failure is the notifier's own to report; the
    /// reminder counts as shown either way, or it would come back forever.
    fn show(&self, notice: &Notice);
}

/// At most this many missed reminders show one by one; more show as one
/// summary.
const MISSED_ONE_BY_ONE: usize = 3;
/// How many due reminders one round reads.
const ROUND_LIMIT: u32 = 100;

// The texts are English until the Rust side reads the message files the
// page uses.
fn notice_for(item: &DueReminder) -> Notice {
    let key = &item.task.key;
    let what = match item.kind {
        ReminderKind::Reminder => format!("Reminder: {key}"),
        ReminderKind::Deadline => match &item.due_date {
            Some(day) => format!("{key} is due {day}"),
            None => format!("{key} is due soon"),
        },
        ReminderKind::Timer => format!("The timer is still running on {key}"),
    };
    let title = if item.missed {
        format!("Missed: {what}")
    } else {
        what
    };
    let body = if item.note.is_empty() {
        item.task.title.clone()
    } else {
        format!("{}\n{}", item.task.title, item.note)
    };
    Notice {
        title,
        body,
        task: Some(key.clone()),
    }
}

fn missed_summary(items: &[&DueReminder]) -> Notice {
    let keys: Vec<&str> = items
        .iter()
        .take(6)
        .map(|item| item.task.key.as_str())
        .collect();
    let more = if items.len() > keys.len() {
        ", …"
    } else {
        ""
    };
    Notice {
        title: format!("{} missed reminders", items.len()),
        body: format!(
            "They came due while Glyphkeel was closed: {}{more}",
            keys.join(", ")
        ),
        task: items.first().map(|item| item.task.key.clone()),
    }
}

/// The scheduler's state between rounds.
#[derive(Debug)]
pub(crate) struct Scheduler {
    /// When the app started: reminders due before it were missed.
    started_at: i64,
}

impl Scheduler {
    pub(crate) fn new(started_at: i64) -> Self {
        Self { started_at }
    }

    /// One round at `now` (milliseconds since 1970): shows the reminders that
    /// are due and acknowledges them. Answers when the next one comes due.
    pub(crate) fn round(
        &mut self,
        engine: &Engine,
        now: i64,
        notifier: &dyn Notifier,
    ) -> Result<Option<i64>, OpError> {
        let ctx = Context::user(Via::App);
        loop {
            let due = engine.run::<ReminderDue>(
                &ctx,
                &ReminderDueInput {
                    until: Some(rfc3339(now)),
                    since: Some(rfc3339(self.started_at)),
                    limit: Some(ROUND_LIMIT),
                },
            )?;
            let next = due.next_at.as_deref().and_then(millis_of);
            if due.items.is_empty() {
                return Ok(next);
            }
            let (missed, current): (Vec<&DueReminder>, Vec<&DueReminder>) =
                due.items.iter().partition(|item| item.missed);
            if missed.len() > MISSED_ONE_BY_ONE {
                notifier.show(&missed_summary(&missed));
            } else {
                for item in &missed {
                    notifier.show(&notice_for(item));
                }
            }
            for item in &current {
                notifier.show(&notice_for(item));
            }
            let ids = due.items.iter().map(|item| item.id.clone()).collect();
            engine.run::<ReminderAcknowledge>(&ctx, &ReminderAckInput { ids })?;
            // More than one round's worth: read on.
            if due.total <= u64::from(ROUND_LIMIT) {
                return Ok(next);
            }
        }
    }
}

/// An RFC 3339 moment in milliseconds since 1970.
fn millis_of(moment: &str) -> Option<i64> {
    moment
        .parse::<jiff::Timestamp>()
        .ok()
        .map(jiff::Timestamp::as_millisecond)
}

/// What wakes the scheduler before its next moment.
enum Wake {
    Changed,
    Stop,
}

/// How long the scheduler waits at most between rounds. The clock it
/// sleeps on stops while the computer sleeps; the wall clock doesn't, so it
/// looks again at least this often.
pub(crate) const LONGEST_WAIT: Duration = Duration::from_secs(30);
/// Change events come in bursts; one round follows a burst.
const SETTLE: Duration = Duration::from_millis(200);
/// After a failed round, the next try.
const RETRY: Duration = Duration::from_secs(60);

/// The running scheduler; dropping it doesn't stop it, [`Running::stop`]
/// does.
#[derive(Debug)]
pub(crate) struct Running {
    wake: Sender<Wake>,
    thread: Option<JoinHandle<()>>,
    stopped: Arc<AtomicBool>,
}

impl Running {
    /// Stops the scheduler and waits for its round to end.
    pub(crate) fn stop(mut self) {
        self.stopped.store(true, Ordering::Release);
        let _ = self.wake.send(Wake::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Starts the scheduler on its own thread. `events` are the engine's change
/// events; `clock` gives the time (the system's, or a test's); `longest_wait`
/// bounds each sleep.
pub(crate) fn start(
    engine: Weak<Engine>,
    events: Receiver<gk_api::Changed>,
    clock: Clock,
    notifier: Arc<dyn Notifier>,
    longest_wait: Duration,
) -> Option<Running> {
    let (wake, woken) = mpsc::channel();
    let forward = wake.clone();
    // Ends when the engine's event bus goes, with the engine.
    let forwarder = std::thread::Builder::new()
        .name("gk-app-reminder-events".into())
        .spawn(move || {
            for _ in events {
                if forward.send(Wake::Changed).is_err() {
                    break;
                }
            }
        });
    if let Err(err) = forwarder {
        eprintln!("glyphkeel-app: reminders won't follow changes until a restart: {err}");
    }
    let stopped = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stopped);
    let thread = std::thread::Builder::new()
        .name("gk-app-reminders".into())
        .spawn(move || {
            run(
                &engine,
                &woken,
                &clock,
                &*notifier,
                &stop_flag,
                longest_wait,
            )
        })
        .inspect_err(|err| eprintln!("glyphkeel-app: cannot start the reminders: {err}"))
        .ok()?;
    Some(Running {
        wake,
        thread: Some(thread),
        stopped,
    })
}

fn run(
    engine: &Weak<Engine>,
    woken: &Receiver<Wake>,
    clock: &Clock,
    notifier: &dyn Notifier,
    stopped: &AtomicBool,
    longest_wait: Duration,
) {
    let mut scheduler = Scheduler::new(clock());
    let longest = i64::try_from(longest_wait.as_millis()).unwrap_or(i64::MAX);
    loop {
        if stopped.load(Ordering::Acquire) {
            return;
        }
        let Some(strong) = engine.upgrade() else {
            return;
        };
        let now = clock();
        let next = match scheduler.round(&strong, now, notifier) {
            Ok(next) => next,
            Err(err) => {
                eprintln!("glyphkeel-app: reminders: {}", err.message);
                Some(now.saturating_add(i64::try_from(RETRY.as_millis()).unwrap_or(i64::MAX)))
            }
        };
        // Only the rounds hold the engine, so that closing it isn't held up.
        drop(strong);
        let until = next.unwrap_or(i64::MAX);
        loop {
            let left = until.saturating_sub(clock());
            if left <= 0 {
                break;
            }
            let wait = Duration::from_millis(u64::try_from(left.min(longest)).unwrap_or(0));
            match woken.recv_timeout(wait) {
                Ok(Wake::Changed) => {
                    // Let the burst finish, then look again.
                    std::thread::sleep(SETTLE);
                    while let Ok(wake) = woken.try_recv() {
                        if matches!(wake, Wake::Stop) {
                            return;
                        }
                    }
                    break;
                }
                Ok(Wake::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {
                    if stopped.load(Ordering::Acquire) {
                        return;
                    }
                    if left > longest {
                        // A long sleep: look again, in case the computer slept.
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::sync::Mutex;
    use std::sync::atomic::AtomicI64;

    use gk_engine::{EngineConfig, Role};
    use gk_store::WorkspaceLayout;
    use serde_json::{Value, json};
    use tempfile::TempDir;

    use super::*;

    /// 2026-11-23T06:00:00Z, a Monday: 07:00 in Berlin.
    const START: i64 = 1_795_413_600_000;
    const MINUTE: i64 = 60_000;
    const HOUR: i64 = 60 * MINUTE;

    #[derive(Default)]
    struct Shown(Mutex<Vec<Notice>>);

    impl Notifier for Shown {
        fn show(&self, notice: &Notice) {
            self.0.lock().unwrap().push(notice.clone());
        }
    }

    impl Shown {
        fn titles(&self) -> Vec<String> {
            self.0
                .lock()
                .unwrap()
                .iter()
                .map(|n| n.title.clone())
                .collect()
        }
    }

    fn open(dir: &TempDir, clock: &Arc<AtomicI64>) -> Engine {
        let tick = Arc::clone(clock);
        let mut config = EngineConfig::new(Role::App);
        config.clock = Arc::new(move || tick.load(Ordering::SeqCst));
        config.time_zone = Some("Europe/Berlin".into());
        Engine::open(&WorkspaceLayout::new(dir.path().join("Personal")), config).unwrap()
    }

    fn call(engine: &Engine, op: &str, input: Value) -> Value {
        engine
            .call(&Context::user(Via::App), op, input)
            .unwrap_or_else(|err| panic!("{op}: {err:?}"))
    }

    fn new_task(engine: &Engine, title: &str) -> String {
        call(
            engine,
            "task.create",
            json!({ "project": "TM", "title": title }),
        )["task"]["key"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn a_round_shows_what_is_due_acknowledges_it_and_says_when_to_look_next() {
        let dir = TempDir::new().unwrap();
        let clock = Arc::new(AtomicI64::new(START));
        let engine = open(&dir, &clock);
        call(
            &engine,
            "project.create",
            json!({ "name": "T", "key": "TM" }),
        );
        let key = new_task(&engine, "Call the bank");
        call(
            &engine,
            "reminder.set",
            json!({ "key": key, "at": "2026-11-23 12:00", "note": "Ask about the fee" }),
        );
        let shown = Shown::default();
        let mut scheduler = Scheduler::new(START);

        // Before it: nothing shows; the next look is at 12:00 Berlin.
        let next = scheduler.round(&engine, START, &shown).unwrap();
        assert_eq!(next, Some(START + 5 * HOUR));
        assert!(shown.titles().is_empty());

        // At it: it shows once, with its task to open.
        clock.store(START + 5 * HOUR, Ordering::SeqCst);
        let next = scheduler.round(&engine, START + 5 * HOUR, &shown).unwrap();
        assert_eq!(next, None);
        let notices = shown.0.lock().unwrap().clone();
        assert_eq!(
            notices,
            [Notice {
                title: format!("Reminder: {key}"),
                body: "Call the bank\nAsk about the fee".into(),
                task: Some(key.clone()),
            }]
        );
        scheduler.round(&engine, START + 6 * HOUR, &shown).unwrap();
        assert_eq!(shown.titles().len(), 1, "acknowledged reminders stay quiet");
    }

    #[test]
    fn reminders_due_while_the_app_was_closed_show_as_missed() {
        let dir = TempDir::new().unwrap();
        let clock = Arc::new(AtomicI64::new(START));
        let engine = open(&dir, &clock);
        call(
            &engine,
            "project.create",
            json!({ "name": "T", "key": "TM" }),
        );
        let keys: Vec<String> = (1..=5)
            .map(|n| new_task(&engine, &format!("Task {n}")))
            .collect();
        call(
            &engine,
            "reminder.set",
            json!({ "key": keys[0], "at": "2026-11-23 08:00" }),
        );
        drop(engine);

        // Started at 10:00: the 08:00 reminder was missed.
        clock.store(START + 3 * HOUR, Ordering::SeqCst);
        let engine = open(&dir, &clock);
        let shown = Shown::default();
        Scheduler::new(START + 3 * HOUR)
            .round(&engine, START + 3 * HOUR, &shown)
            .unwrap();
        assert_eq!(shown.titles(), [format!("Missed: Reminder: {}", keys[0])]);

        // Many missed at once show as one summary that opens one of them.
        for key in &keys {
            call(
                &engine,
                "reminder.set",
                json!({ "key": key, "at": "2026-11-23 09:00" }),
            );
        }
        let shown = Shown::default();
        Scheduler::new(START + 3 * HOUR)
            .round(&engine, START + 3 * HOUR, &shown)
            .unwrap();
        let notices = shown.0.lock().unwrap().clone();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].title, "5 missed reminders");
        assert!(keys.contains(notices[0].task.as_ref().unwrap()));
        assert!(notices[0].body.contains(&keys[4]), "{}", notices[0].body);
    }

    #[test]
    fn the_timer_reminds_after_the_workspace_hours() {
        let dir = TempDir::new().unwrap();
        let clock = Arc::new(AtomicI64::new(START));
        let engine = open(&dir, &clock);
        call(
            &engine,
            "project.create",
            json!({ "name": "T", "key": "TM" }),
        );
        let key = new_task(&engine, "Write the report");
        call(&engine, "timer.start", json!({ "key": key }));
        let shown = Shown::default();
        let mut scheduler = Scheduler::new(START);
        assert_eq!(
            scheduler.round(&engine, START, &shown).unwrap(),
            Some(START + 8 * HOUR)
        );
        clock.store(START + 8 * HOUR, Ordering::SeqCst);
        scheduler.round(&engine, START + 8 * HOUR, &shown).unwrap();
        assert_eq!(
            shown.titles(),
            [format!("The timer is still running on {key}")]
        );
    }

    #[test]
    fn the_running_scheduler_follows_the_clock_and_changes_and_stops() {
        let dir = TempDir::new().unwrap();
        let clock = Arc::new(AtomicI64::new(START));
        let engine = Arc::new(open(&dir, &clock));
        call(
            &engine,
            "project.create",
            json!({ "name": "T", "key": "TM" }),
        );
        let key = new_task(&engine, "Water the plants");
        let shown = Arc::new(Shown::default());
        let tick = Arc::clone(&clock);
        let running = start(
            Arc::downgrade(&engine),
            engine.subscribe(),
            Arc::new(move || tick.load(Ordering::SeqCst)),
            Arc::clone(&shown) as Arc<dyn Notifier>,
            Duration::from_millis(20),
        )
        .unwrap();
        let wait_for = |count: usize| {
            for _ in 0..200 {
                if shown.titles().len() >= count {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("expected {count} notices, got {:?}", shown.titles());
        };

        // A reminder set while it sleeps moves its next look; it fires when
        // the clock gets there.
        call(
            &engine,
            "reminder.set",
            json!({ "key": key, "at": "2026-11-23 07:30" }),
        );
        std::thread::sleep(Duration::from_millis(400));
        assert!(shown.titles().is_empty());
        clock.store(START + 30 * MINUTE, Ordering::SeqCst);
        wait_for(1);
        assert_eq!(shown.titles(), [format!("Reminder: {key}")]);

        // After stopping, nothing more shows.
        running.stop();
        call(
            &engine,
            "reminder.set",
            json!({ "key": key, "at": "2026-11-23 07:45" }),
        );
        clock.store(START + HOUR, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(shown.titles().len(), 1);
    }

    #[test]
    fn the_scheduler_ends_with_the_engine() {
        let dir = TempDir::new().unwrap();
        let clock = Arc::new(AtomicI64::new(START));
        let engine = Arc::new(open(&dir, &clock));
        let tick = Arc::clone(&clock);
        let running = start(
            Arc::downgrade(&engine),
            engine.subscribe(),
            Arc::new(move || tick.load(Ordering::SeqCst)),
            Arc::new(Shown::default()),
            Duration::from_millis(20),
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(100));
        // Nothing else holds the engine between rounds.
        let mut shared = engine;
        let engine = loop {
            match Arc::try_unwrap(shared) {
                Ok(engine) => break engine,
                Err(back) => {
                    shared = back;
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        };
        engine.close();
        running.stop();
    }
}
