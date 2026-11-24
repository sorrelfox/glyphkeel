// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Limits per agent session, so a runaway loop can't flood the workspace.
//!
//! Today: at most the workspace's limit of calls of tools that change
//! something, in any sliding minute ([`WRITES_PER_MINUTE`] unless the person
//! set another number or no limit). Going over returns a `limit` error with
//! `retry_after` in seconds.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use gk_api::OpError;

use crate::messages;

/// Calls of changing tools allowed in any minute, unless the person set
/// another limit.
pub const WRITES_PER_MINUTE: usize = gk_api::ops::agent::AGENT_WRITES_PER_MINUTE as usize;

const WINDOW: Duration = Duration::from_secs(60);

/// A sliding-window counter of calls. The most allowed is given with each
/// call, because the person can change it while the session runs; calls
/// are counted even without a limit, so one set later counts them too.
#[derive(Debug)]
pub(crate) struct RateLimit {
    window: Duration,
    calls: Mutex<VecDeque<Instant>>,
}

impl RateLimit {
    /// The writes-per-minute counter of an agent session.
    #[must_use]
    pub(crate) fn writes() -> Self {
        Self::new(WINDOW)
    }

    /// Counts calls in any `window`.
    #[must_use]
    pub(crate) fn new(window: Duration) -> Self {
        Self {
            window,
            calls: Mutex::new(VecDeque::new()),
        }
    }

    /// Counts a call at `now`, or refuses it when `max` calls were made in
    /// the window already. `None`: no limit.
    ///
    /// # Errors
    /// `limit`, with `details.max` and `details.retry_after` (whole seconds,
    /// at least 1).
    pub(crate) fn check(&self, now: Instant, max: Option<usize>) -> Result<(), OpError> {
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
        let Some(max) = max.filter(|max| calls.len() >= *max) else {
            calls.push_back(now);
            return Ok(());
        };
        let oldest = calls.front().copied().unwrap_or(now);
        let wait = self
            .window
            .saturating_sub(now.saturating_duration_since(oldest));
        let retry_after = wait.as_secs() + u64::from(wait.subsec_nanos() > 0);
        let retry_after = retry_after.max(1);
        Err(messages::hint(
            messages::error("mcp-rate-limit", &[("max", max.to_string())], |m| {
                OpError::limit(m, max as u64)
            })
            .with_detail("retry_after", retry_after),
            "mcp-rate-limit-hint",
            &[("seconds", retry_after.to_string())],
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_over_the_limit_and_recovers() {
        let limit = RateLimit::new(Duration::from_secs(60));
        let t0 = Instant::now();
        for i in 0..3 {
            assert!(limit.check(t0 + Duration::from_secs(i), Some(3)).is_ok());
        }
        let refused = limit
            .check(t0 + Duration::from_secs(10), Some(3))
            .expect_err("over the limit");
        assert_eq!(refused.code, gk_api::ErrorCode::Limit);
        assert_eq!(refused.details["retry_after"], 50);
        assert_eq!(refused.details["max"], 3);
        // The first call leaves the window after a minute.
        assert!(limit.check(t0 + Duration::from_secs(60), Some(3)).is_ok());
        assert!(limit.check(t0 + Duration::from_secs(60), Some(3)).is_err());
    }

    #[test]
    fn no_limit_allows_every_call_but_still_counts_them() {
        let limit = RateLimit::new(Duration::from_secs(60));
        let t0 = Instant::now();
        for _ in 0..1000 {
            assert!(limit.check(t0, None).is_ok());
        }
        // A limit set later counts the calls already made in the window.
        assert!(limit.check(t0, Some(1000)).is_err());
        assert!(
            limit
                .check(t0 + Duration::from_secs(60), Some(1000))
                .is_ok()
        );
    }

    #[test]
    fn the_session_limit() {
        let limit = RateLimit::writes();
        let t0 = Instant::now();
        for _ in 0..WRITES_PER_MINUTE {
            assert!(limit.check(t0, Some(WRITES_PER_MINUTE)).is_ok());
        }
        assert!(limit.check(t0, Some(WRITES_PER_MINUTE)).is_err());
    }
}
