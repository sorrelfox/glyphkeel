// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Keeps the app's memory small: the page's script heap, and memory the
//! program has freed.
//!
//! The web view's script engine lets its heap grow in proportion to the
//! machine's memory before it collects, so on a large machine one scroll
//! through a long list leaves hundreds of megabytes of garbage behind. The
//! app tells it to size its heap as if the machine had 256 MB, unless the
//! user has set `JSC_forceRAMSize` already. The web view's processes inherit
//! the setting from the app's environment.
//!
//! glibc's allocator keeps freed memory for reuse: in one arena per busy
//! thread, and at the top of each arena's heap. After a large answer (a board
//! of thousands of cards, a long list) is serialized and sent to the page,
//! tens of megabytes stay with the process although nothing uses them. Two
//! things keep that small:
//!
//! - at start, the allocator is limited to two arenas, so the engine's
//!   blocking threads share their free memory instead of each keeping its own;
//! - a background thread returns the free memory to the system once the
//!   operations have been quiet for a moment.
//!
//! Setting the environment, and both calls into the C library, are `unsafe`,
//! which is why this one module may use it. The allocator calls are no-ops
//! with other C libraries.

#![allow(unsafe_code)]

use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// How often the background thread looks whether to give memory back.
const CHECK_EVERY: Duration = Duration::from_secs(2);

/// How long the operations must have been quiet before it does.
const QUIET_FOR: Duration = Duration::from_secs(1);

/// Some operation ran since memory was last given back.
static WORKED: AtomicBool = AtomicBool::new(true);

/// When the last operation finished, in milliseconds since [`START`].
static LAST_WORK_MS: AtomicU64 = AtomicU64::new(0);

static START: LazyLock<Instant> = LazyLock::new(Instant::now);

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod glibc {
    use std::ffi::c_int;

    /// `mallopt`'s parameter for the most arenas the allocator makes.
    const M_ARENA_MAX: c_int = -8;

    unsafe extern "C" {
        fn mallopt(param: c_int, value: c_int) -> c_int;
        fn malloc_trim(pad: usize) -> c_int;
    }

    pub(super) fn limit_arenas(arenas: c_int) {
        // SAFETY: `mallopt` only changes the allocator's tuning; glibc
        // documents it as safe to call at any time, from any thread.
        unsafe {
            mallopt(M_ARENA_MAX, arenas);
        }
    }

    pub(super) fn trim() {
        // SAFETY: `malloc_trim` takes the arenas' locks itself and only
        // releases pages that hold no allocation.
        unsafe {
            malloc_trim(0);
        }
    }
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
mod glibc {
    pub(super) fn limit_arenas(_arenas: i32) {}
    pub(super) fn trim() {}
}

/// The memory the page's script engine sizes its heap for, in bytes.
const SCRIPT_HEAP_RAM: &str = "268435456";

/// Sets the page's script heap and limits the allocator to two arenas.
/// Called first thing in `main`, before any other thread starts.
pub(crate) fn limit() {
    LazyLock::force(&START);
    #[cfg(target_os = "linux")]
    if std::env::var_os("JSC_forceRAMSize").is_none() {
        // SAFETY: called at the start of `main`, while no other thread
        // exists to read the environment at the same time.
        unsafe {
            std::env::set_var("JSC_forceRAMSize", SCRIPT_HEAP_RAM);
        }
    }
    glibc::limit_arenas(2);
}

/// Notes that an operation has just finished.
pub(crate) fn worked() {
    LAST_WORK_MS.store(elapsed_ms(), Ordering::Relaxed);
    WORKED.store(true, Ordering::Relaxed);
}

/// Starts the thread that gives free memory back after work.
pub(crate) fn start_trimming() {
    let spawned = thread::Builder::new().name("heap-trim".into()).spawn(|| {
        loop {
            thread::sleep(CHECK_EVERY);
            let worked = WORKED.load(Ordering::Relaxed);
            let last = LAST_WORK_MS.load(Ordering::Relaxed);
            if due(worked, last, elapsed_ms()) {
                WORKED.store(false, Ordering::Relaxed);
                glibc::trim();
            }
        }
    });
    if let Err(err) = spawned {
        eprintln!("glyphkeel-app: memory is not given back after work: {err}");
    }
}

/// Whether to give memory back now: something ran, and it has been quiet
/// since for a moment.
fn due(worked: bool, last_work_ms: u64, now_ms: u64) -> bool {
    worked && now_ms.saturating_sub(last_work_ms) >= QUIET_FOR.as_millis() as u64
}

fn elapsed_ms() -> u64 {
    u64::try_from(START.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_goes_back_only_after_work_and_a_quiet_moment() {
        assert!(!due(false, 0, 10_000));
        assert!(!due(true, 9_500, 10_000));
        assert!(due(true, 9_000, 10_000));
        assert!(due(true, 0, 10_000));
    }

    #[test]
    fn the_calls_into_the_c_library_run() {
        worked();
        glibc::trim();
        assert!(WORKED.load(Ordering::Relaxed));
    }
}
