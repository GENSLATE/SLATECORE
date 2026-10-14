//! Wrong-password throttle (research/vault.md 7).
//!
//! This is a **UX safeguard, not a security boundary**: it slows someone guessing through the
//! launcher. Anyone with a copy of the drive runs Argon2id offline and never meets it; deleting
//! `vault.guard` resets the persisted part. The real defence is the KDF cost and the password.
//!
//! The in-memory monotonic deadline is authoritative while the launcher runs. Across restarts
//! the deadline is restored from `vault.guard`; a wall clock moved back before the last attempt
//! keeps the full delay. An attempt is recorded on disk *before* the KDF runs, so killing the
//! process mid-unlock does not yield a free guess.

use std::fs;
use std::io::{Read as _, Write as _};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Added wait before the next attempt after `failures` consecutive wrong passwords:
/// none for 1 to 3, then 5 s doubling per failure, capped at 900 s from the 12th on.
pub fn delay_after(failures: u32) -> Duration {
    match failures {
        0..=3 => Duration::ZERO,
        4..=11 => Duration::from_secs(5 << (failures - 4)),
        _ => Duration::from_mins(15),
    }
}

#[derive(Serialize, Deserialize)]
struct GuardFile {
    v: u32,
    failures: u32,
    last_attempt_ms: u64,
    locked_until_ms: u64,
}

/// Throttle state for one vault.
pub(crate) struct Throttle {
    path: PathBuf,
    failures: u32,
    next_allowed: Option<Instant>,
}

/// An attempt that has been recorded but not yet resolved.
pub(crate) struct Attempt {
    previous: u32,
}

/// Longest `vault.guard` that is read; the file this module writes is under 100 bytes.
const MAX_GUARD_BYTES: u64 = 1024;

/// The guard file's bytes, or `None` when it is missing, unreadable or larger than the cap.
fn read_guard(path: &std::path::Path) -> Option<Vec<u8>> {
    let file = fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_GUARD_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= MAX_GUARD_BYTES).then_some(bytes)
}

fn wall_ms(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

impl Throttle {
    /// Restores the persisted state. A missing, unreadable, oversized or corrupt file means zero
    /// failures.
    pub(crate) fn load(path: PathBuf, now: Instant, now_wall: SystemTime) -> Self {
        let guard = read_guard(&path)
            .and_then(|bytes| serde_json::from_slice::<GuardFile>(&bytes).ok())
            .filter(|g| g.v == 1);
        let Some(guard) = guard else {
            return Self {
                path,
                failures: 0,
                next_allowed: None,
            };
        };
        let now_ms = wall_ms(now_wall);
        let remaining_ms = if now_ms < guard.last_attempt_ms {
            // Clock moved back: keep the full delay for this many failures.
            u64::try_from(delay_after(guard.failures).as_millis()).unwrap_or(u64::MAX)
        } else {
            guard.locked_until_ms.saturating_sub(now_ms)
        };
        let remaining = Duration::from_millis(remaining_ms).min(delay_after(guard.failures));
        let next_allowed = (!remaining.is_zero()).then(|| now + remaining);
        Self {
            path,
            failures: guard.failures,
            next_allowed,
        }
    }

    pub(crate) fn failures(&self) -> u32 {
        self.failures
    }

    /// Time left before the next attempt is allowed, if any.
    pub(crate) fn retry_after(&self, now: Instant) -> Option<Duration> {
        self.next_allowed
            .and_then(|t| t.checked_duration_since(now))
            .filter(|d| !d.is_zero())
    }

    /// Records an attempt before the KDF runs, or refuses it while throttled.
    pub(crate) fn begin(
        &mut self,
        now: Instant,
        now_wall: SystemTime,
    ) -> Result<Attempt, Duration> {
        if let Some(wait) = self.retry_after(now) {
            return Err(wait);
        }
        let previous = self.failures;
        let counted = previous.saturating_add(1);
        let now_ms = wall_ms(now_wall);
        let delay_ms = u64::try_from(delay_after(counted).as_millis()).unwrap_or(u64::MAX);
        self.persist(counted, now_ms, now_ms.saturating_add(delay_ms));
        Ok(Attempt { previous })
    }

    /// The password was wrong: the recorded attempt stands and the wait starts now.
    pub(crate) fn fail(&mut self, attempt: Attempt, now: Instant) {
        self.failures = attempt.previous.saturating_add(1);
        let wait = delay_after(self.failures);
        self.next_allowed = (!wait.is_zero()).then(|| now + wait);
    }

    /// The password was right: reset to zero on disk and in memory.
    pub(crate) fn succeed(&mut self, _attempt: Attempt, now_wall: SystemTime) {
        self.failures = 0;
        self.next_allowed = None;
        let now_ms = wall_ms(now_wall);
        self.persist(0, now_ms, now_ms);
    }

    /// The attempt never reached a verdict (for example the KDF could not get memory): restore
    /// the previous count.
    pub(crate) fn abandon(&mut self, attempt: Attempt, now_wall: SystemTime) {
        // `begin` only succeeds when no wait is pending, so none needs restoring.
        self.failures = attempt.previous;
        let now_ms = wall_ms(now_wall);
        self.persist(self.failures, now_ms, now_ms);
    }

    /// Best-effort write with `sync_data`; failures are ignored (the in-memory state holds).
    fn persist(&self, failures: u32, last_attempt_ms: u64, locked_until_ms: u64) {
        let guard = GuardFile {
            v: 1,
            failures,
            last_attempt_ms,
            locked_until_ms,
        };
        let Ok(bytes) = serde_json::to_vec(&guard) else {
            return;
        };
        let written = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)
            .and_then(|mut f| f.write_all(&bytes).and_then(|()| f.sync_data()));
        let _ = written;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_with_injected_clock() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("vault.guard");
        let start = Instant::now();
        let wall = SystemTime::now();
        let mut t = Throttle::load(path.clone(), start, wall);
        for n in 1..=12u32 {
            let now = start + Duration::from_secs(10_000 * u64::from(n));
            let attempt = t.begin(now, wall);
            assert!(attempt.is_ok(), "attempt {n} refused");
            if let Ok(attempt) = attempt {
                t.fail(attempt, now);
            }
            assert_eq!(
                t.retry_after(now),
                Some(delay_after(n)).filter(|d| !d.is_zero())
            );
            if n >= 4 {
                assert!(
                    t.begin(now + Duration::from_secs(1), wall).is_err(),
                    "n = {n}"
                );
            }
        }
    }
}
