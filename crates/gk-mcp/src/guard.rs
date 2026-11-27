// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Limits per agent session, so a runaway loop can't flood the workspace.
//!
//! Changes: at most the workspace's limit of calls of tools that change
//! something, in any sliding minute ([`WRITES_PER_MINUTE`] unless the person
//! set another number or no limit). Going over returns a `limit` error with
//! `retry_after` in seconds.
//!
//! Reads: a token bucket of reads (a burst, then a sustained rate) and one
//! of bytes returned a minute, so an agent can't copy out the workspace at
//! full speed. Going over returns a `rate_limited` error with
//! `retry_after`. The limits hold per session: each process, and each
//! session of the HTTP endpoint, counts its own.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use gk_api::ops::agent::AgentLimits;
use gk_api::{OpError, Profile};

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

/// What a session may read: the workspace's limits, scaled for the
/// agent's profile. A read-only agent may read twice as much, as reading
/// is all it does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ReadLimits {
    /// Reads at once, and the reads a second after that; `None`: no limit
    /// on reads.
    calls: Option<(f64, f64)>,
    /// Bytes in a minute; `None`: no limit.
    bytes_per_minute: Option<f64>,
    /// The workspace's numbers, for the error's text.
    burst: u32,
    per_minute: u32,
    mb: u32,
}

/// What one subscription notification costs, in reads.
pub(crate) const NOTIFICATION_COST: f64 = 0.1;

const BYTES_PER_MB: f64 = 1_048_576.0;

impl ReadLimits {
    /// The read limits for `profile` under the workspace's `limits`.
    #[must_use]
    pub(crate) fn of(limits: &AgentLimits, profile: Profile) -> Self {
        let scale = profile_scale(profile);
        let calls = limits.reads_per_minute.map(|per_minute| {
            let burst = limits.read_burst.unwrap_or(per_minute);
            (
                f64::from(burst) * scale,
                f64::from(per_minute) * scale / 60.0,
            )
        });
        Self {
            calls,
            bytes_per_minute: limits
                .read_mb_per_minute
                .map(|mb| f64::from(mb) * BYTES_PER_MB * scale),
            burst: limits
                .read_burst
                .or(limits.reads_per_minute)
                .map_or(0, |n| scale_u32(n, scale)),
            per_minute: limits.reads_per_minute.map_or(0, |n| scale_u32(n, scale)),
            mb: limits.read_mb_per_minute.map_or(0, |n| scale_u32(n, scale)),
        }
    }

    /// No limit at all.
    #[cfg(test)]
    pub(crate) const NONE: Self = Self {
        calls: None,
        bytes_per_minute: None,
        burst: 0,
        per_minute: 0,
        mb: 0,
    };
}

/// How much more than the workspace's limits a profile may read.
#[must_use]
pub(crate) const fn profile_scale(profile: Profile) -> f64 {
    match profile {
        Profile::ReadOnly => 2.0,
        Profile::Standard | Profile::Full => 1.0,
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "limits are at most a million, times two"
)]
fn scale_u32(n: u32, scale: f64) -> u32 {
    (f64::from(n) * scale).round() as u32
}

/// Two token buckets per session: one of reads, refilled at the sustained
/// rate up to the burst, and one of bytes, refilled at a minute's worth a
/// minute. A read needs a whole read in the first and anything left in the
/// second; what it returned is taken from the second afterwards, which may
/// go into debt of up to a minute's worth. The limits are given with each
/// read, because the person can change them while the session runs.
#[derive(Debug, Default)]
pub(crate) struct ReadLimit {
    state: Mutex<Buckets>,
}

#[derive(Debug, Default)]
struct Buckets {
    /// Reads left; `None`: full.
    calls: Option<f64>,
    /// Bytes left; `None`: full.
    bytes: Option<f64>,
    /// When they were last refilled.
    at: Option<Instant>,
}

/// Which limit refused a read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refused {
    Calls,
    Bytes,
}

impl Buckets {
    fn refill(&mut self, now: Instant, limits: &ReadLimits) {
        let elapsed = self
            .at
            .map_or(0.0, |at| now.saturating_duration_since(at).as_secs_f64());
        self.at = Some(now);
        self.calls = match (limits.calls, self.calls) {
            (Some((burst, rate)), Some(left)) => Some((left + elapsed * rate).min(burst)),
            _ => None,
        };
        self.bytes = match (limits.bytes_per_minute, self.bytes) {
            (Some(cap), Some(left)) => Some((left + elapsed * cap / 60.0).min(cap)),
            _ => None,
        };
    }

    fn calls_left(&self, limits: &ReadLimits) -> f64 {
        match limits.calls {
            Some((burst, _)) => self.calls.unwrap_or(burst),
            None => f64::INFINITY,
        }
    }

    fn bytes_left(&self, limits: &ReadLimits) -> f64 {
        match limits.bytes_per_minute {
            Some(cap) => self.bytes.unwrap_or(cap),
            None => f64::INFINITY,
        }
    }

    /// The seconds until a read costing `cost` may go ahead; `None`: now.
    fn wait(&self, limits: &ReadLimits, cost: f64) -> Option<(Refused, f64)> {
        let calls = self.calls_left(limits);
        // A little slack, so that ten notifications make one read exactly.
        if calls + 1e-9 < cost {
            let rate = limits.calls.map_or(1.0, |(_, rate)| rate);
            return Some((Refused::Calls, (cost - calls) / rate));
        }
        let bytes = self.bytes_left(limits);
        if bytes <= 0.0 {
            let rate = limits.bytes_per_minute.map_or(1.0, |cap| cap / 60.0);
            return Some((Refused::Bytes, (1.0 - bytes) / rate));
        }
        None
    }

    fn take(&mut self, limits: &ReadLimits, cost: f64) {
        if let Some((burst, _)) = limits.calls {
            self.calls = Some(self.calls.unwrap_or(burst) - cost);
        }
    }
}

impl ReadLimit {
    fn lock(&self) -> std::sync::MutexGuard<'_, Buckets> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Takes one read at `now`, or refuses it when the session read
    /// faster than `limits` allow.
    ///
    /// # Errors
    /// `rate_limited`, with `details.retry_after` (whole seconds, at least
    /// 1) and `details.limit`, the limit that refused it.
    pub(crate) fn check(&self, now: Instant, limits: &ReadLimits) -> Result<(), OpError> {
        let mut buckets = self.lock();
        buckets.refill(now, limits);
        if let Some((refused, wait)) = buckets.wait(limits, 1.0) {
            return Err(refusal(refused, wait, limits));
        }
        buckets.take(limits, 1.0);
        Ok(())
    }

    /// Takes what a read returned, `bytes`, from the session's bytes.
    #[allow(clippy::cast_precision_loss, reason = "an estimate is enough")]
    pub(crate) fn returned(&self, now: Instant, limits: &ReadLimits, bytes: usize) {
        let mut buckets = self.lock();
        buckets.refill(now, limits);
        if let Some(cap) = limits.bytes_per_minute {
            let left = buckets.bytes.unwrap_or(cap) - bytes as f64;
            buckets.bytes = Some(left.max(-cap));
        }
    }

    /// Takes a notification's light cost at `now`; `false` when the session
    /// must wait first, and the notification waits too.
    pub(crate) fn notify(&self, now: Instant, limits: &ReadLimits) -> bool {
        let mut buckets = self.lock();
        buckets.refill(now, limits);
        if buckets.wait(limits, NOTIFICATION_COST).is_some() {
            return false;
        }
        buckets.take(limits, NOTIFICATION_COST);
        true
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the wait is short and positive"
)]
fn refusal(refused: Refused, wait: f64, limits: &ReadLimits) -> OpError {
    let retry_after = (wait.ceil() as u64).max(1);
    let (id, args, limit) = match refused {
        Refused::Calls => (
            "mcp-read-limit-calls",
            vec![
                ("burst", limits.burst.to_string()),
                ("per_minute", limits.per_minute.to_string()),
            ],
            "reads_per_minute",
        ),
        Refused::Bytes => (
            "mcp-read-limit-bytes",
            vec![("mb", limits.mb.to_string())],
            "read_mb_per_minute",
        ),
    };
    messages::hint(
        messages::error(id, &args, |m| OpError::rate_limited(m, retry_after))
            .with_detail("limit", limit),
        "mcp-read-limit-hint",
        &[("seconds", retry_after.to_string())],
    )
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

    fn limits(burst: u32, per_minute: u32, mb: Option<u32>, profile: Profile) -> ReadLimits {
        let workspace = AgentLimits {
            read_burst: Some(burst),
            reads_per_minute: Some(per_minute),
            read_mb_per_minute: mb,
            ..AgentLimits::DEFAULT
        };
        ReadLimits::of(&workspace, profile)
    }

    #[test]
    fn a_burst_then_the_sustained_rate() {
        let read = ReadLimit::default();
        // 5 at once, then one every 2 s.
        let l = limits(5, 30, None, Profile::Standard);
        let t0 = Instant::now();
        for _ in 0..5 {
            assert!(read.check(t0, &l).is_ok());
        }
        let refused = read.check(t0, &l).expect_err("over the burst");
        assert_eq!(refused.code, gk_api::ErrorCode::RateLimited);
        assert_eq!(refused.details["retry_after"], 2);
        assert_eq!(refused.details["limit"], "reads_per_minute");
        assert!(read.check(t0 + Duration::from_secs(2), &l).is_ok());
        assert!(read.check(t0 + Duration::from_secs(2), &l).is_err());
        // Idle for a while, the burst comes back, but no more.
        let later = t0 + Duration::from_secs(600);
        for _ in 0..5 {
            assert!(read.check(later, &l).is_ok());
        }
        assert!(read.check(later, &l).is_err());
    }

    #[test]
    fn the_defaults_never_hinder_ten_reads_a_second() {
        let read = ReadLimit::default();
        let l = ReadLimits::of(&AgentLimits::DEFAULT, Profile::Standard);
        let t0 = Instant::now();
        for i in 0..6000_u64 {
            let at = t0 + Duration::from_millis(i * 100);
            assert!(read.check(at, &l).is_ok(), "read {i}");
            read.returned(at, &l, 10_000);
        }
    }

    #[test]
    fn bytes_a_minute_slow_down_bulk_reads() {
        let read = ReadLimit::default();
        let l = limits(1000, 100_000, Some(1), Profile::Standard);
        let t0 = Instant::now();
        assert!(read.check(t0, &l).is_ok());
        read.returned(t0, &l, 3 * 1_048_576);
        let refused = read.check(t0, &l).expect_err("over the bytes");
        assert_eq!(refused.details["limit"], "read_mb_per_minute");
        // In debt of at most a minute's worth: a minute and a bit.
        assert_eq!(refused.details["retry_after"], 61);
        assert!(read.check(t0 + Duration::from_secs(61), &l).is_ok());
    }

    #[test]
    fn no_limit_and_the_profiles() {
        let read = ReadLimit::default();
        let t0 = Instant::now();
        for _ in 0..10_000 {
            assert!(read.check(t0, &ReadLimits::NONE).is_ok());
        }
        let lifted = AgentLimits {
            reads_per_minute: None,
            read_mb_per_minute: None,
            ..AgentLimits::DEFAULT
        };
        assert_eq!(ReadLimits::of(&lifted, Profile::Full).calls, None);

        // A read-only agent may read twice as much.
        let read_only = limits(3, 60, None, Profile::ReadOnly);
        let read = ReadLimit::default();
        for _ in 0..6 {
            assert!(read.check(t0, &read_only).is_ok());
        }
        assert!(read.check(t0, &read_only).is_err());
    }

    #[test]
    fn notifications_cost_little_and_wait_when_the_reads_are_used_up() {
        let read = ReadLimit::default();
        let l = limits(1, 1, None, Profile::Standard);
        let t0 = Instant::now();
        for _ in 0..9 {
            assert!(read.notify(t0, &l));
        }
        assert!(read.check(t0, &l).is_err());
        assert!(read.notify(t0, &l));
        assert!(!read.notify(t0, &l));
    }
}
