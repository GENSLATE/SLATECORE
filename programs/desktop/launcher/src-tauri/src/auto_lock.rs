//! When the vault locks itself: after a period without use, and after the launcher has been
//! hidden for a while. A pure state machine over injected `Instant`s; the shell polls it from a
//! timer thread and calls the vault's `lock`.
//!
//! The minutes come from `[vault]` in `settings.toml` (`idle-lock-minutes`, default 10, and
//! `hide-lock-minutes`, default 5); `0` turns a timer off.

use std::time::{Duration, Instant};

/// Why the vault should lock now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockReason {
    /// Nothing used the vault for the idle period.
    Idle,
    /// The launcher stayed hidden for the hide period.
    Hidden,
}

/// The two timeouts; `None` = never.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    pub idle: Option<Duration>,
    pub hidden: Option<Duration>,
}

impl Timeouts {
    /// From the settings' minutes (`0` = never).
    pub fn from_minutes(idle: u32, hidden: u32) -> Self {
        let minutes = |m: u32| (m > 0).then(|| Duration::from_secs(u64::from(m) * 60));
        Self {
            idle: minutes(idle),
            hidden: minutes(hidden),
        }
    }
}

/// Tracks the last use of the vault and since when the launcher is hidden.
#[derive(Debug, Clone)]
pub struct AutoLock {
    last_use: Instant,
    hidden_since: Option<Instant>,
}

impl AutoLock {
    /// Starts counting at `now` with the launcher hidden (it is shown a moment later).
    pub fn new(now: Instant) -> Self {
        Self {
            last_use: now,
            hidden_since: Some(now),
        }
    }

    /// The vault was used (a command ran, the launcher was shown): restart the idle period.
    pub fn touch(&mut self, now: Instant) {
        self.last_use = now;
    }

    /// The launcher became visible or hidden. Showing it counts as use.
    pub fn set_visible(&mut self, visible: bool, now: Instant) {
        if visible {
            self.hidden_since = None;
            self.last_use = now;
        } else if self.hidden_since.is_none() {
            self.hidden_since = Some(now);
        }
    }

    /// The reason to lock at `now`, if any. A hidden launcher is judged by the hide period; the
    /// idle period applies to a visible one that nobody uses.
    pub fn due(&self, now: Instant, timeouts: Timeouts) -> Option<LockReason> {
        let elapsed = |since: Instant| now.saturating_duration_since(since);
        if let (Some(since), Some(limit)) = (self.hidden_since, timeouts.hidden)
            && elapsed(since) >= limit
        {
            return Some(LockReason::Hidden);
        }
        match timeouts.idle {
            Some(limit) if elapsed(self.last_use) >= limit => Some(LockReason::Idle),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minutes(m: u64) -> Duration {
        Duration::from_secs(m * 60)
    }

    #[test]
    fn the_defaults_are_ten_idle_minutes_and_five_hidden() {
        let timeouts = Timeouts::from_minutes(10, 5);
        assert_eq!(timeouts.idle, Some(minutes(10)));
        assert_eq!(timeouts.hidden, Some(minutes(5)));
        assert_eq!(
            Timeouts::from_minutes(0, 0),
            Timeouts {
                idle: None,
                hidden: None
            }
        );
    }

    #[test]
    fn a_visible_launcher_locks_after_ten_idle_minutes() {
        let start = Instant::now();
        let timeouts = Timeouts::from_minutes(10, 5);
        let mut lock = AutoLock::new(start);
        lock.set_visible(true, start);
        assert_eq!(lock.due(start + minutes(9), timeouts), None);
        assert_eq!(
            lock.due(start + minutes(10), timeouts),
            Some(LockReason::Idle)
        );
        // Using the vault restarts the period.
        lock.touch(start + minutes(9));
        assert_eq!(lock.due(start + minutes(10), timeouts), None);
        assert_eq!(
            lock.due(start + minutes(19), timeouts),
            Some(LockReason::Idle)
        );
    }

    #[test]
    fn a_hidden_launcher_locks_after_five_minutes() {
        let start = Instant::now();
        let timeouts = Timeouts::from_minutes(10, 5);
        let mut lock = AutoLock::new(start);
        lock.set_visible(true, start);
        lock.set_visible(false, start + minutes(1));
        assert_eq!(lock.due(start + minutes(5), timeouts), None);
        assert_eq!(
            lock.due(start + minutes(6), timeouts),
            Some(LockReason::Hidden)
        );
    }

    #[test]
    fn hiding_again_does_not_restart_the_hide_period() {
        let start = Instant::now();
        let timeouts = Timeouts::from_minutes(10, 5);
        let mut lock = AutoLock::new(start);
        lock.set_visible(true, start);
        lock.set_visible(false, start);
        lock.set_visible(false, start + minutes(4));
        assert_eq!(
            lock.due(start + minutes(5), timeouts),
            Some(LockReason::Hidden)
        );
    }

    #[test]
    fn showing_the_launcher_cancels_the_hide_timer_and_counts_as_use() {
        let start = Instant::now();
        let timeouts = Timeouts::from_minutes(10, 5);
        let mut lock = AutoLock::new(start);
        lock.set_visible(false, start);
        lock.set_visible(true, start + minutes(4));
        assert_eq!(lock.due(start + minutes(8), timeouts), None);
        assert_eq!(
            lock.due(start + minutes(14), timeouts),
            Some(LockReason::Idle)
        );
    }

    #[test]
    fn zero_minutes_turn_a_timer_off() {
        let start = Instant::now();
        let mut lock = AutoLock::new(start);
        lock.set_visible(false, start);
        let never = Timeouts::from_minutes(0, 0);
        assert_eq!(lock.due(start + minutes(100_000), never), None);
        let no_hide = Timeouts::from_minutes(10, 0);
        assert_eq!(
            lock.due(start + minutes(11), no_hide),
            Some(LockReason::Idle)
        );
    }
}
