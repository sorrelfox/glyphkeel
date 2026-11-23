// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Limits per agent session, so a runaway loop can't flood the workspace.
//!
//! Today: at most [`WRITES_PER_MINUTE`] calls of tools that change
//! something, in any sliding minute. Going over returns a `limit` error
//! with `retry_after` in seconds.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use gk_api::OpError;

/// Calls of changing tools allowed in any minute.
pub const WRITES_PER_MINUTE: usize = 120;

const WINDOW: Duration = Duration::from_secs(60);

/// A sliding-window counter of calls.
#[derive(Debug)]
pub(crate) struct RateLimit {
    max: usize,
    window: Duration,
    calls: Mutex<VecDeque<Instant>>,
}

impl RateLimit {
    /// The writes-per-minute limit of an agent session.
    #[must_use]
    pub(crate) fn writes() -> Self {
        Self::new(WRITES_PER_MINUTE, WINDOW)
    }

    /// At most `max` calls in any `window`.
    #[must_use]
    pub(crate) fn new(max: usize, window: Duration) -> Self {
        Self {
            max,
            window,
            calls: Mutex::new(VecDeque::new()),
        }
    }

    /// Counts a call at `now`, or refuses it when the window is full.
    ///
    /// # Errors
    /// `limit`, with `details.max` and `details.retry_after` (whole seconds,
    /// at least 1).
    pub(crate) fn check(&self, now: Instant) -> Result<(), OpError> {
        let mut calls = self
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while calls
            .front()
            .is_some_and(|t| now.saturating_duration_since(*t) >= self.window)
        {
            calls.pop_front();
        }
        if calls.len() < self.max {
            calls.push_back(now);
            return Ok(());
        }
        let oldest = calls.front().copied().unwrap_or(now);
        let wait = self
            .window
            .saturating_sub(now.saturating_duration_since(oldest));
        let retry_after = wait.as_secs() + u64::from(wait.subsec_nanos() > 0);
        let retry_after = retry_after.max(1);
        Err(OpError::limit(
            format!(
                "This session made {} changes in the last minute, the most allowed.",
                self.max
            ),
            self.max as u64,
        )
        .with_detail("retry_after", retry_after)
        .with_hint(format!("Wait {retry_after} s before the next change.")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_over_the_limit_and_recovers() {
        let limit = RateLimit::new(3, Duration::from_secs(60));
        let t0 = Instant::now();
        for i in 0..3 {
            assert!(limit.check(t0 + Duration::from_secs(i)).is_ok());
        }
        let refused = limit
            .check(t0 + Duration::from_secs(10))
            .expect_err("over the limit");
        assert_eq!(refused.code, gk_api::ErrorCode::Limit);
        assert_eq!(refused.details["retry_after"], 50);
        assert_eq!(refused.details["max"], 3);
        // The first call leaves the window after a minute.
        assert!(limit.check(t0 + Duration::from_secs(60)).is_ok());
        assert!(limit.check(t0 + Duration::from_secs(60)).is_err());
    }

    #[test]
    fn the_session_limit() {
        let limit = RateLimit::writes();
        let t0 = Instant::now();
        for _ in 0..WRITES_PER_MINUTE {
            assert!(limit.check(t0).is_ok());
        }
        assert!(limit.check(t0).is_err());
    }
}
