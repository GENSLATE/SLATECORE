//! The wrong-password throttle (R1 to R8). It is a UX safeguard, not a security boundary.

mod common;

use std::fs;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use common::{Fixture, OTHER_PASSWORD, PASSWORD, TestResult, pw};
use genslate_vault::{LockPolicy, VaultError, throttle};

fn now_ms() -> Result<u64, Box<dyn std::error::Error>> {
    Ok(u64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}

fn write_guard(
    fixture: &Fixture,
    failures: u32,
    last_attempt_ms: u64,
    locked_until_ms: u64,
) -> std::io::Result<()> {
    fs::write(
        fixture.root().join("vault.guard"),
        format!(
            r#"{{"v":1,"failures":{failures},"last_attempt_ms":{last_attempt_ms},"locked_until_ms":{locked_until_ms}}}"#
        ),
    )
}

#[test]
fn throttle_schedule_increases_and_survives_restart() -> TestResult {
    // R1: the documented schedule.
    let expected = [0, 0, 0, 0, 5, 10, 20, 40, 80, 160, 320, 640, 900, 900, 900];
    for (failures, secs) in expected.iter().enumerate() {
        assert_eq!(
            throttle::delay_after(u32::try_from(failures)?),
            Duration::from_secs(*secs),
            "n = {failures}"
        );
    }
    assert_eq!(throttle::delay_after(u32::MAX), Duration::from_mins(15));

    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    vault.lock(LockPolicy::SyncThenWipe)?;
    for attempt in 1..=3 {
        assert!(matches!(
            vault.unlock(&pw(OTHER_PASSWORD)),
            Err(VaultError::WrongPassword)
        ));
        let status = vault.status();
        assert_eq!(status.failed_attempts, attempt);
        assert_eq!(
            status.retry_after, None,
            "no wait for the first three failures"
        );
    }
    assert!(matches!(
        vault.unlock(&pw(OTHER_PASSWORD)),
        Err(VaultError::WrongPassword)
    ));
    let status = vault.status();
    assert_eq!(status.failed_attempts, 4);
    let wait = status
        .retry_after
        .ok_or("expected a wait after 4 failures")?;
    assert!(
        wait > Duration::from_secs(4) && wait <= Duration::from_secs(5),
        "{wait:?}"
    );

    // R2: while throttled, even the right password is refused without running Argon2.
    let started = Instant::now();
    let result = vault.unlock(&pw(PASSWORD));
    assert!(
        matches!(result, Err(VaultError::Throttled { .. })),
        "{result:?}"
    );
    assert!(
        started.elapsed() < Duration::from_millis(50),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(
        vault.status().failed_attempts,
        4,
        "a refused attempt is not counted"
    );

    // R3: the delay survives a restart.
    drop(vault);
    let vault = fixture.open()?;
    let status = vault.status();
    assert_eq!(status.failed_attempts, 4);
    let wait = status.retry_after.ok_or("throttle lost on restart")?;
    assert!(wait > Duration::from_secs(3), "{wait:?}");
    assert!(matches!(
        vault.unlock(&pw(PASSWORD)),
        Err(VaultError::Throttled { .. })
    ));
    drop(vault);

    // After the window it allows again, and the next failure doubles the wait.
    let now = now_ms()?;
    write_guard(&fixture, 4, now - 60_000, now - 55_000)?;
    let vault = fixture.open()?;
    assert_eq!(vault.status().retry_after, None);
    assert!(matches!(
        vault.unlock(&pw(OTHER_PASSWORD)),
        Err(VaultError::WrongPassword)
    ));
    let wait = vault
        .status()
        .retry_after
        .ok_or("expected a wait after 5 failures")?;
    assert!(
        wait > Duration::from_secs(9) && wait <= Duration::from_secs(10),
        "{wait:?}"
    );
    drop(vault);

    // R7: a clock moved back before the last attempt does not shorten the delay.
    write_guard(&fixture, 4, now + 86_400_000, now + 86_400_000)?;
    let vault = fixture.open()?;
    let wait = vault
        .status()
        .retry_after
        .ok_or("rewound clock must keep the full delay")?;
    assert!(
        wait > Duration::from_secs(4) && wait <= Duration::from_secs(5),
        "{wait:?}"
    );
    drop(vault);

    // R5/P2: success resets the counter on disk.
    write_guard(&fixture, 4, now - 60_000, now - 55_000)?;
    let vault = fixture.open()?;
    vault.unlock(&pw(PASSWORD))?;
    assert_eq!(vault.status().failed_attempts, 0);
    let guard = fs::read_to_string(fixture.root().join("vault.guard"))?;
    assert!(guard.contains(r#""failures":0"#), "{guard}");
    Ok(())
}

#[test]
fn attempt_is_counted_before_the_kdf_runs() -> TestResult {
    // R4: the counter is written before Argon2 starts, so killing the process mid-KDF cannot
    // yield a free guess. Watch vault.guard while a deliberately slow KDF runs.
    let mut fixture = Fixture::new()?;
    fixture.config.kdf = genslate_vault::KdfParams {
        m_cost_kib: 65_536,
        t_cost: 3,
        p_cost: 1,
    };
    let vault = fixture.create()?;
    vault.lock(LockPolicy::SyncThenWipe)?;
    let guard_path = fixture.root().join("vault.guard");
    let seen_during_kdf = std::thread::scope(|scope| {
        let unlocking = scope.spawn(|| vault.unlock(&pw(PASSWORD)));
        let mut seen = false;
        while !unlocking.is_finished() {
            if fs::read_to_string(&guard_path).is_ok_and(|g| g.contains(r#""failures":1"#)) {
                seen = true;
            }
        }
        (seen, unlocking.join())
    });
    let (seen, joined) = seen_during_kdf;
    joined.map_err(|_| "unlock panicked")??;
    assert!(seen, "the attempt was not recorded before the KDF finished");
    let guard = fs::read_to_string(&guard_path)?;
    assert!(
        guard.contains(r#""failures":0"#),
        "reset after success: {guard}"
    );
    Ok(())
}

#[test]
fn missing_or_corrupt_guard_means_zero_failures() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    vault.lock(LockPolicy::SyncThenWipe)?;
    for _ in 0..4 {
        let _ = vault.unlock(&pw(OTHER_PASSWORD));
    }
    drop(vault);

    // R8: garbage is treated as zero failures and does not panic.
    fs::write(fixture.root().join("vault.guard"), b"\x00\xff not json{{")?;
    let vault = fixture.open()?;
    assert_eq!(vault.status().failed_attempts, 0);
    assert_eq!(vault.status().retry_after, None);
    drop(vault);

    // An oversized guard file is not read past its cap: treated as corrupt, zero failures.
    let padding = " ".repeat(64 * 1024);
    fs::write(
        fixture.root().join("vault.guard"),
        format!(
            r#"{padding}{{"v":1,"failures":9,"last_attempt_ms":0,"locked_until_ms":{}}}"#,
            now_ms()? + 160_000
        ),
    )?;
    let vault = fixture.open()?;
    assert_eq!(vault.status().failed_attempts, 0);
    drop(vault);

    // R6 (documented limit): deleting the file resets the persisted state.
    write_guard(&fixture, 9, now_ms()?, now_ms()? + 160_000)?;
    fs::remove_file(fixture.root().join("vault.guard"))?;
    let vault = fixture.open()?;
    assert_eq!(vault.status().failed_attempts, 0);
    vault.unlock(&pw(PASSWORD))?;
    Ok(())
}
