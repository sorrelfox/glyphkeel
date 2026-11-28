// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The app lock, and the workspace's encryption as Preferences changes it.
//!
//! The app locks on demand (a command, its shortcut, Preferences) or after
//! the person was idle for the minutes they chose. While it is locked:
//!
//! - every call of the page is refused ([`AppLock::check`] in
//!   [`Workspace::call`](crate::ipc::Workspace::call)), so no data reaches
//!   a window, and stored images aren't served either, since they go
//!   through the same calls;
//! - the page shows the lock screen instead of the app and drops the data
//!   it held;
//! - reminders still come, but their notifications name no task
//!   ([`masked`]).
//!
//! It unlocks with the workspace's passphrase when the workspace is
//! encrypted, else with a passphrase kept for the lock alone, as a salted
//! PBKDF2 check in the app's settings ([`gk_store::PassphraseCheck`]). The
//! operating system's password is not used. Without either there is
//! nothing to unlock with, so the app can't lock.
//!
//! The workspace stays open while the app is locked: reminders and other
//! programs go on. The key stays in memory, as the open database needs it.
//! Passphrases are never kept in the system's keyring.
//!
//! An encrypted workspace that the app could not open at start for want of
//! its passphrase shows the same screen; its passphrase opens it
//! ([`unlock_app`]).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use gk_api::OpError;
use gk_engine::encryption::{self, Change};
use gk_store::{Passphrase, PassphraseCheck};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::app_settings::AppSettings;
use crate::files::blocking;
use crate::ipc::Workspace;
use crate::reminders::{Notice, Notifier};

/// The longest idle time that can be chosen, in minutes (one day).
const MAX_IDLE_MINUTES: u32 = 24 * 60;

/// Whether the app is locked, and when it locks by itself.
#[derive(Debug)]
pub(crate) struct AppLock {
    locked: AtomicBool,
    /// Whether there is a passphrase to unlock with; without one the app
    /// never locks.
    can_lock: AtomicBool,
    last_activity: Mutex<Instant>,
    idle: Mutex<Option<Duration>>,
}

impl Default for AppLock {
    fn default() -> Self {
        Self {
            locked: AtomicBool::new(false),
            can_lock: AtomicBool::new(false),
            last_activity: Mutex::new(Instant::now()),
            idle: Mutex::new(None),
        }
    }
}

impl AppLock {
    /// Sets how long the person may be idle before the app locks itself
    /// (`None`: never), and whether there is a passphrase to unlock with.
    pub(crate) fn configure(&self, idle: Option<Duration>, can_lock: bool) {
        *self.idle.lock().unwrap_or_else(PoisonError::into_inner) = idle;
        self.can_lock.store(can_lock, Ordering::SeqCst);
        if !can_lock {
            self.locked.store(false, Ordering::SeqCst);
        }
    }

    /// Whether the app can lock: there is a passphrase to unlock with.
    pub(crate) fn can_lock(&self) -> bool {
        self.can_lock.load(Ordering::SeqCst)
    }

    /// The person did something in a window.
    pub(crate) fn touch(&self) {
        if !self.is_locked() {
            *self
                .last_activity
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Instant::now();
        }
    }

    /// Locks the app; `false` when there is nothing to unlock it with.
    pub(crate) fn lock(&self) -> bool {
        if !self.can_lock() {
            return false;
        }
        self.locked.store(true, Ordering::SeqCst);
        crate::files::private_copies::clean();
        true
    }

    fn unlock(&self) {
        self.locked.store(false, Ordering::SeqCst);
        self.touch();
    }

    /// Whether the app is locked now; an idle time that ran out locks it.
    pub(crate) fn is_locked(&self) -> bool {
        if self.locked.load(Ordering::SeqCst) {
            return true;
        }
        let idle = *self.idle.lock().unwrap_or_else(PoisonError::into_inner);
        let since = self
            .last_activity
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .elapsed();
        if self.can_lock() && idle.is_some_and(|idle| since >= idle) {
            self.locked.store(true, Ordering::SeqCst);
            crate::files::private_copies::clean();
            return true;
        }
        false
    }

    /// Refuses while the app is locked.
    pub(crate) fn check(&self) -> Result<(), OpError> {
        if self.is_locked() {
            return Err(locked_error());
        }
        Ok(())
    }
}

/// The error of a call made while the app is locked.
fn locked_error() -> OpError {
    crate::messages::error("app-locked", &[], OpError::forbidden).with_detail("locked", true)
}

/// A notifier that names no task while the app is locked.
pub(crate) fn masked(lock: Arc<AppLock>, inner: Arc<dyn Notifier>) -> Arc<dyn Notifier> {
    Arc::new(Masked { lock, inner })
}

struct Masked {
    lock: Arc<AppLock>,
    inner: Arc<dyn Notifier>,
}

impl Notifier for Masked {
    fn show(&self, notice: &Notice) {
        if self.lock.is_locked() {
            self.inner.show(&masked_notice());
        } else {
            self.inner.show(notice);
        }
    }

    fn show_private(&self, notice: &Notice, locked: &Notice) {
        if self.lock.is_locked() {
            self.inner.show(locked);
        } else {
            self.inner.show(notice);
        }
    }
}

fn masked_notice() -> Notice {
    Notice {
        title: crate::messages::text("lock-notice-title", &[]),
        body: crate::messages::text("lock-notice-body", &[]),
        task: None,
    }
}

/// What the page needs to know about the lock and the encryption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct LockView {
    /// The app is locked, or the workspace waits for its passphrase.
    pub(crate) locked: bool,
    /// Why: `"app"` for the app lock, `"workspace"` for an encrypted
    /// workspace that didn't open yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reason: Option<&'static str>,
    /// There is a passphrase to unlock with, so the app can lock.
    pub(crate) can_lock: bool,
    /// Minutes of idle time before the app locks; absent: never.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) lock_after_minutes: Option<u32>,
    /// A passphrase is kept for the lock of a plain workspace.
    pub(crate) lock_passphrase: bool,
    /// The workspace's encryption; absent when no workspace is open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) encryption: Option<encryption::Status>,
}

/// The settings of the lock as stored, read by [`view`] and [`configure`].
fn stored(settings: &AppSettings) -> (Option<u32>, Option<PassphraseCheck>) {
    let stored = settings.stored();
    (stored.lock_after_minutes, stored.lock_passphrase)
}

/// The workspace's encryption: of the open one, else of the one waiting
/// for its passphrase.
fn encryption_status(workspace: &Workspace) -> Option<encryption::Status> {
    workspace
        .layout()
        .or_else(|| workspace.waiting_for_passphrase())
        .and_then(|layout| encryption::status(&layout).ok())
}

/// Brings the lock in line with the settings and the workspace.
pub(crate) fn configure(settings: &AppSettings, workspace: &Workspace) {
    let (minutes, check) = stored(settings);
    let encrypted = workspace
        .layout()
        .and_then(|layout| encryption::status(&layout).ok())
        .is_some_and(|status| status.encrypted);
    workspace.app_lock().configure(
        minutes.map(|m| Duration::from_secs(u64::from(m) * 60)),
        encrypted || check.is_some(),
    );
}

fn view(settings: &AppSettings, workspace: &Workspace) -> LockView {
    let (minutes, check) = stored(settings);
    let lock = workspace.app_lock();
    let waiting = workspace.waiting_for_passphrase().is_some();
    let app_locked = lock.is_locked();
    LockView {
        locked: waiting || app_locked,
        reason: if waiting {
            Some("workspace")
        } else if app_locked {
            Some("app")
        } else {
            None
        },
        can_lock: lock.can_lock(),
        lock_after_minutes: minutes,
        lock_passphrase: check.is_some(),
        encryption: encryption_status(workspace),
    }
}

fn passphrase(text: &str) -> Result<Passphrase, OpError> {
    Passphrase::new(text).ok_or_else(|| {
        crate::messages::error("lock-passphrase-empty", &[], |m| {
            OpError::invalid_at("passphrase", m)
        })
    })
}

fn wrong_passphrase() -> OpError {
    crate::messages::error("lock-passphrase-wrong", &[], |m| {
        OpError::invalid_at("passphrase", m)
    })
    .with_detail("passphrase", "wrong")
}

/// Whether `given` unlocks: the encrypted workspace's passphrase, else the
/// lock's own.
fn opens(
    settings: &AppSettings,
    workspace: &Workspace,
    given: &Passphrase,
) -> Result<bool, OpError> {
    if let Some(layout) = workspace.layout()
        && gk_store::encryption_of(&layout.database()).map_err(|e| gk_engine::store_error(&e))?
            == gk_store::Encryption::Encrypted
    {
        return gk_store::passphrase_opens(&layout.database(), given)
            .map_err(|e| gk_engine::store_error(&e));
    }
    Ok(stored(settings).1.is_some_and(|check| check.matches(given)))
}

/// The lock and the encryption, for the page.
#[tauri::command]
pub(crate) fn lock_state(
    settings: State<'_, AppSettings>,
    workspace: State<'_, Arc<Workspace>>,
) -> LockView {
    view(&settings, &workspace)
}

/// The person did something: the idle time starts again. Answers whether
/// the app is locked.
#[tauri::command]
pub(crate) fn app_activity(workspace: State<'_, Arc<Workspace>>) -> bool {
    let lock = workspace.app_lock();
    lock.touch();
    lock.is_locked()
}

/// Locks the app now. The other windows find out at their next call or
/// look at the lock.
#[tauri::command]
pub(crate) fn lock_app(
    settings: State<'_, AppSettings>,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<LockView, OpError> {
    if !workspace.app_lock().lock() {
        return Err(crate::messages::hint(
            crate::messages::error("lock-not-possible", &[], OpError::invalid),
            "lock-not-possible-hint",
            &[],
        ));
    }
    Ok(view(&settings, &workspace))
}

/// Unlocks the app with `passphrase`, or opens the encrypted workspace
/// that waited for it.
#[tauri::command]
pub(crate) async fn unlock_app(
    app: AppHandle,
    workspace: State<'_, Arc<Workspace>>,
    passphrase: String,
) -> Result<LockView, OpError> {
    let workspace = Arc::clone(&workspace);
    blocking(move || {
        let settings = app.state::<AppSettings>();
        let given = self::passphrase(&passphrase)?;
        if let Some(layout) = workspace.waiting_for_passphrase() {
            let database = layout.database();
            if !gk_store::passphrase_opens(&database, &given)
                .map_err(|e| gk_engine::store_error(&e))?
            {
                return Err(wrong_passphrase());
            }
            workspace.open_waiting()?;
            configure(&settings, &workspace);
            workspace.app_lock().unlock();
            return Ok(view(&settings, &workspace));
        }
        if !opens(&settings, &workspace, &given)? {
            return Err(wrong_passphrase());
        }
        workspace.app_lock().unlock();
        Ok(view(&settings, &workspace))
    })
    .await
}

/// A change to the lock from Preferences; fields left out stay.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LockPatch {
    /// Minutes of idle time before the app locks; 0 for never.
    #[serde(default)]
    lock_after_minutes: Option<u32>,
    /// A new passphrase for the lock of a plain workspace; empty removes
    /// it.
    #[serde(default)]
    lock_passphrase: Option<String>,
    /// The lock's passphrase now, needed to change or remove it.
    #[serde(default)]
    current: Option<String>,
}

/// Changes when the app locks, and the lock's own passphrase.
#[tauri::command]
pub(crate) async fn update_lock(
    app: AppHandle,
    workspace: State<'_, Arc<Workspace>>,
    patch: LockPatch,
) -> Result<LockView, OpError> {
    let workspace = Arc::clone(&workspace);
    blocking(move || {
        let settings = app.state::<AppSettings>();
        workspace.app_lock().check()?;
        let (_, check) = stored(&settings);
        let new_check = match patch.lock_passphrase.as_deref() {
            None => None,
            Some(text) => {
                if let Some(check) = &check {
                    let current = patch.current.as_deref().map(passphrase).transpose()?;
                    if !current.is_some_and(|c| check.matches(&c)) {
                        return Err(wrong_passphrase());
                    }
                }
                Some(match Passphrase::new(text) {
                    Some(p) => {
                        Some(PassphraseCheck::new(&p).map_err(|e| gk_engine::store_error(&e))?)
                    }
                    None => None,
                })
            }
        };
        let minutes = patch
            .lock_after_minutes
            .map(|m| m.min(MAX_IDLE_MINUTES))
            .map(|m| (m > 0).then_some(m));
        settings.change_lock(minutes, new_check);
        configure(&settings, &workspace);
        Ok(view(&settings, &workspace))
    })
    .await
}

/// What changing the encryption did, for Preferences.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct EncryptionChanged {
    #[serde(flatten)]
    view: LockView,
    /// The backup made before the change.
    backup: String,
    /// Backups that stayed unencrypted.
    backups_left_plain: u64,
    /// Attached files the change left as they were.
    files_failed: u64,
}

/// Encrypts the open workspace with `passphrase`, gives it that new
/// passphrase, or (`encrypt` false) stores it unencrypted again. An
/// encrypted workspace needs its `current` passphrase for either. The
/// workspace closes for the change and opens again; no other program may
/// have it open.
#[tauri::command]
pub(crate) async fn change_encryption(
    app: AppHandle,
    workspace: State<'_, Arc<Workspace>>,
    encrypt: bool,
    passphrase: Option<String>,
    current: Option<String>,
) -> Result<EncryptionChanged, OpError> {
    let workspace = Arc::clone(&workspace);
    blocking(move || {
        let settings = app.state::<AppSettings>();
        workspace.app_lock().check()?;
        let layout = workspace
            .layout()
            .ok_or_else(|| crate::messages::error("workspace-closed", &[], OpError::storage))?;
        let database = layout.database();
        let encrypted = gk_store::encryption_of(&database)
            .map_err(|e| gk_engine::store_error(&e))?
            == gk_store::Encryption::Encrypted;
        if encrypted {
            let current = self::passphrase(current.as_deref().unwrap_or_default())?;
            if !gk_store::passphrase_opens(&database, &current)
                .map_err(|e| gk_engine::store_error(&e))?
            {
                return Err(wrong_passphrase().with_detail("field", "current"));
            }
        }
        let change = if encrypt {
            Change::Encrypt(self::passphrase(passphrase.as_deref().unwrap_or_default())?)
        } else {
            Change::Decrypt
        };
        let now = gk_engine::system_clock()();
        let version = env!("CARGO_PKG_VERSION");
        let mut changed = None;
        workspace.closed_for(|before| {
            let layout = before
                .cloned()
                .ok_or_else(|| crate::messages::error("workspace-closed", &[], OpError::storage))?;
            changed = Some(encryption::change(&layout, None, &change, now, version)?);
            Ok(layout)
        })?;
        let Some(changed) = changed else {
            return Err(crate::messages::error(
                "workspace-closed",
                &[],
                OpError::storage,
            ));
        };
        configure(&settings, &workspace);
        Ok(EncryptionChanged {
            view: view(&settings, &workspace),
            backup: changed.backup.name,
            backups_left_plain: changed.backups_left_plain,
            files_failed: changed.files_failed,
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    #[test]
    fn without_a_passphrase_the_app_does_not_lock() {
        let lock = AppLock::default();
        assert!(!lock.lock());
        assert!(!lock.is_locked());
        assert!(lock.check().is_ok());
        lock.configure(Some(Duration::ZERO), false);
        assert!(!lock.is_locked(), "idle time alone doesn't lock");
    }

    #[test]
    fn a_locked_app_refuses_calls_until_unlocked() {
        let lock = AppLock::default();
        lock.configure(None, true);
        assert!(lock.lock());
        let err = lock.check().unwrap_err();
        assert_eq!(err.code, gk_api::ErrorCode::Forbidden);
        assert_eq!(err.details["locked"], true);
        assert_eq!(err.details["message_id"], "app-locked");
        // Activity doesn't unlock.
        lock.touch();
        assert!(lock.is_locked());
        lock.unlock();
        assert!(lock.check().is_ok());
    }

    #[test]
    fn idle_time_locks_and_activity_keeps_it_open() {
        let lock = AppLock::default();
        lock.configure(Some(Duration::from_secs(3600)), true);
        lock.touch();
        assert!(!lock.is_locked());
        lock.configure(Some(Duration::ZERO), true);
        assert!(lock.is_locked());
        // Removing the passphrase unlocks: nothing could unlock it.
        lock.configure(Some(Duration::ZERO), false);
        assert!(!lock.is_locked());
    }

    struct Seen(StdMutex<Vec<Notice>>);

    impl Notifier for Seen {
        fn show(&self, notice: &Notice) {
            self.0.lock().unwrap().push(Notice {
                title: notice.title.clone(),
                body: notice.body.clone(),
                task: notice.task.clone(),
            });
        }
    }

    #[test]
    fn notifications_name_no_task_while_locked() {
        let lock = Arc::new(AppLock::default());
        lock.configure(None, true);
        let seen = Arc::new(Seen(StdMutex::new(Vec::new())));
        let notifier = masked(Arc::clone(&lock), Arc::clone(&seen) as Arc<dyn Notifier>);
        let notice = Notice {
            title: "Call the bank".into(),
            body: "TM-7 is due".into(),
            task: Some("TM-7".into()),
        };
        notifier.show(&notice);
        lock.lock();
        notifier.show(&notice);
        let seen = seen.0.lock().unwrap();
        assert_eq!(seen[0].title, "Call the bank");
        assert!(!seen[1].title.contains("bank") && !seen[1].body.contains("TM-7"));
        assert_eq!(seen[1].task, None);
        assert_eq!(
            seen[1].title,
            crate::messages::text("lock-notice-title", &[])
        );
    }
}
