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

impl LockReason {
    /// The word the UI gets (`idle` or `hidden`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Hidden => "hidden",
        }
    }
}

/// How many ticks (5 s each) pass before a lock that stays blocked is announced again.
pub const REPEAT_BLOCKED_AFTER_TICKS: u32 = 12;

/// Decides when the UI is told that an automatic lock is blocked (an edited file in the session
/// folder cannot be saved back, so the vault stays unlocked and its plaintext stays on disk).
///
/// The lock is retried every tick, but the UI hears of it when the block *starts*, when its
/// cause changes (another file, another reason) and then once every
/// [`REPEAT_BLOCKED_AFTER_TICKS`] ticks, so a window opened later learns of it without the
/// event channel being flooded every five seconds.
#[derive(Debug, Clone, Default)]
pub struct BlockedNotifier {
    last: Option<(LockReason, String)>,
    quiet_ticks: u32,
}

impl BlockedNotifier {
    /// A tick found the lock blocked for `reason` because of `cause` (a stable description of
    /// it). `true` when the UI should be told now.
    pub fn blocked(&mut self, reason: LockReason, cause: &str) -> bool {
        let same = self
            .last
            .as_ref()
            .is_some_and(|(last, text)| *last == reason && text == cause);
        if !same {
            self.last = Some((reason, cause.to_owned()));
            self.quiet_ticks = 0;
            return true;
        }
        self.quiet_ticks += 1;
        if self.quiet_ticks >= REPEAT_BLOCKED_AFTER_TICKS {
            self.quiet_ticks = 0;
            return true;
        }
        false
    }

    /// A tick that was not blocked (the vault locked, or there was nothing to do): the next block
    /// is a new one.
    pub fn clear(&mut self) {
        self.last = None;
        self.quiet_ticks = 0;
    }
}

/// How long after a lock the shell keeps trying to delete decrypted files a program still holds.
pub const LEFTOVER_RETRY_WINDOW: Duration = Duration::from_secs(120);

/// What to do about decrypted files that a lock could not delete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftoverStep {
    /// Nothing is left, the vault is unlocked, or the window already ended.
    Nothing,
    /// Try to delete them again now.
    Retry,
    /// The window just ended with files still there: tell the UI one last time.
    GaveUp,
}

/// Retries the deletion of session files a lock could not remove (`research/vault.md` 6.6: every
/// tick for two minutes). The window starts when leftovers are first seen after a lock and ends
/// for good when they are gone or the vault is unlocked again, so every lock gets its own.
#[derive(Debug, Clone, Default)]
pub struct LeftoverRetry {
    since: Option<Instant>,
    gave_up: bool,
}

impl LeftoverRetry {
    /// One tick: the vault is `unlocked` or not and `leftovers` session files are still on disk.
    pub fn step(&mut self, now: Instant, unlocked: bool, leftovers: usize) -> LeftoverStep {
        if unlocked || leftovers == 0 {
            *self = Self::default();
            return LeftoverStep::Nothing;
        }
        let since = *self.since.get_or_insert(now);
        if now.saturating_duration_since(since) < LEFTOVER_RETRY_WINDOW {
            LeftoverStep::Retry
        } else if self.gave_up {
            LeftoverStep::Nothing
        } else {
            self.gave_up = true;
            LeftoverStep::GaveUp
        }
    }
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

    #[test]
    fn a_blocked_lock_is_announced_at_once_then_once_a_minute() {
        let mut notifier = BlockedNotifier::default();
        assert!(
            notifier.blocked(LockReason::Idle, "a.txt"),
            "the first tick"
        );
        for tick in 1..REPEAT_BLOCKED_AFTER_TICKS {
            assert!(
                !notifier.blocked(LockReason::Idle, "a.txt"),
                "tick {tick} is quiet"
            );
        }
        assert!(
            notifier.blocked(LockReason::Idle, "a.txt"),
            "announced again after a minute"
        );
        assert!(!notifier.blocked(LockReason::Idle, "a.txt"));
    }

    #[test]
    fn a_new_cause_or_reason_is_announced_at_once() {
        let mut notifier = BlockedNotifier::default();
        assert!(notifier.blocked(LockReason::Idle, "a.txt"));
        assert!(!notifier.blocked(LockReason::Idle, "a.txt"));
        assert!(
            notifier.blocked(LockReason::Idle, "a.txt,b.txt"),
            "another file"
        );
        assert!(
            notifier.blocked(LockReason::Hidden, "a.txt,b.txt"),
            "another reason"
        );
        assert!(!notifier.blocked(LockReason::Hidden, "a.txt,b.txt"));
    }

    #[test]
    fn once_the_vault_locks_the_next_block_is_a_new_one() {
        let mut notifier = BlockedNotifier::default();
        assert!(notifier.blocked(LockReason::Idle, "a.txt"));
        assert!(!notifier.blocked(LockReason::Idle, "a.txt"));
        notifier.clear();
        assert!(notifier.blocked(LockReason::Idle, "a.txt"));
    }

    #[test]
    fn the_reasons_use_the_words_the_ui_gets() {
        assert_eq!(LockReason::Idle.as_str(), "idle");
        assert_eq!(LockReason::Hidden.as_str(), "hidden");
    }

    #[test]
    fn leftovers_are_retried_every_tick_for_two_minutes_then_given_up_once() {
        let start = Instant::now();
        let mut retry = LeftoverRetry::default();
        assert_eq!(
            retry.step(start, false, 1),
            LeftoverStep::Retry,
            "first seen"
        );
        for seconds in (5..120).step_by(5) {
            let now = start + Duration::from_secs(seconds);
            assert_eq!(retry.step(now, false, 1), LeftoverStep::Retry, "{seconds}s");
        }
        let end = start + LEFTOVER_RETRY_WINDOW;
        assert_eq!(retry.step(end, false, 1), LeftoverStep::GaveUp);
        assert_eq!(
            retry.step(end + Duration::from_secs(5), false, 1),
            LeftoverStep::Nothing
        );
        assert_eq!(
            retry.step(end + Duration::from_secs(600), false, 1),
            LeftoverStep::Nothing
        );
    }

    #[test]
    fn no_leftovers_or_an_unlocked_vault_need_no_retry() {
        let start = Instant::now();
        let mut retry = LeftoverRetry::default();
        assert_eq!(retry.step(start, false, 0), LeftoverStep::Nothing);
        assert_eq!(
            retry.step(start, true, 3),
            LeftoverStep::Nothing,
            "while unlocked the session files are in use, not left over"
        );
    }

    #[test]
    fn a_new_lock_gets_a_new_window() {
        let start = Instant::now();
        let mut retry = LeftoverRetry::default();
        assert_eq!(retry.step(start, false, 2), LeftoverStep::Retry);
        // The program let go: nothing left.
        assert_eq!(
            retry.step(start + Duration::from_secs(30), false, 0),
            LeftoverStep::Nothing
        );
        // Much later, another lock leaves a file behind: a fresh two minutes.
        let later = start + Duration::from_secs(1000);
        assert_eq!(retry.step(later, false, 1), LeftoverStep::Retry);
        assert_eq!(
            retry.step(later + Duration::from_secs(119), false, 1),
            LeftoverStep::Retry
        );
        // Given up, then unlocked and locked again: also fresh.
        assert_eq!(
            retry.step(later + Duration::from_secs(121), false, 1),
            LeftoverStep::GaveUp
        );
        assert_eq!(
            retry.step(later + Duration::from_secs(130), true, 1),
            LeftoverStep::Nothing
        );
        let again = later + Duration::from_mins(15);
        assert_eq!(retry.step(again, false, 1), LeftoverStep::Retry);
    }
}
