//! Tier 1 of the portability test: the launcher's start-up steps, run without Tauri against an
//! install folder in a temp dir, must leave `%TEMP%`, `%APPDATA%` and `%LOCALAPPDATA%` as they
//! were (`genslate_testing::NoTraceGuard`). Tier 2 (`scripts/tests/no-trace.test.ts`) does the
//! same with the packaged exe, and also covers the registry.
//!
//! What this runs: the same I/O the shell performs before and just after its first window, in the
//! same order, through the public crates (`genslate-paths`, `genslate-launcher-core`,
//! `genslate-vault`). The shell's own `env_bootstrap` and `web_data` modules are private to the app
//! crate, so an integration test cannot call them; their start-up steps run under the same
//! `NoTraceGuard` in unit tests inside `src/` (`env_bootstrap::tests::startup_steps_leave_the_os_folders_untouched`
//! and `web_data::tests::resetting_the_web_data_never_touches_the_os_folders`). `env_bootstrap::apply`
//! itself changes the environment of the whole process, so only its plan and its stale-variable
//! lists are tested, never a call.
//!
//! Building: an integration test links the app's library, which needs the Windows Tauri
//! toolchain, so on Linux it can only be compile-checked
//! (`cargo check -p genslate-launcher --tests --locked --target x86_64-pc-windows-msvc`).
//! Run it on Windows with `cargo test -p genslate-launcher --locked --test no_trace`.

use std::error::Error;
use std::fs;
use std::path::Path;

use genslate_launcher_core::APP_NAME;
use genslate_launcher_core::catalog::{AppId, Catalog, CatalogRoots, Source};
use genslate_launcher_core::config::{SETTINGS_FILE, Settings};
use genslate_launcher_core::metadata::write_value;
use genslate_launcher_core::recent::{RecentLaunches, recent_path};
use genslate_paths::{AppPaths, Environment, resolve_with};
use genslate_testing::{NoTraceGuard, TempTree, fake_suite};
use genslate_vault::{KdfParams, LockPolicy, SecretString, Vault, VaultConfig};

type TestResult = Result<(), Box<dyn Error>>;

/// The paths of an unpacked package: found from the exe's place, like the shipped exe does.
fn packaged_paths(tmp: &TempTree) -> Result<AppPaths, Box<dyn Error>> {
    let root = fake_suite(tmp.path())?;
    let env = Environment {
        exe: Some(root.join("programs/genslate/launcher/slatecore-launcher.exe")),
        ..Environment::default()
    };
    Ok(resolve_with(APP_NAME, &env)?)
}

/// Every folder the launcher writes to is inside installDir, and none is an OS folder.
fn assert_inside_install_dir(paths: &AppPaths) {
    let root = &paths.layout.root;
    for dir in [
        &paths.config_dir,
        &paths.log_dir,
        &paths.cache_dir,
        &paths.database_dir,
        &paths.webview2_dir,
        &paths.temp_dir,
        &paths.vault_dir(),
        &paths.vault_session_dir(),
        &paths.bundled_webview2_root(),
    ] {
        assert!(
            dir.starts_with(root),
            "{} is outside installDir {}",
            dir.display(),
            root.display()
        );
    }
}

#[test]
fn startup_dry_run_leaves_the_os_folders_untouched() -> TestResult {
    let tmp = TempTree::new()?;
    let guard = NoTraceGuard::new()?;

    // 1. Folders: found from the exe, created inside installDir (no fallback anywhere else).
    let paths = packaged_paths(&tmp)?;
    assert_inside_install_dir(&paths);
    paths.create_dirs()?;

    // 2. Settings and the app lists: read-only at start-up; a missing file is the defaults.
    let settings = Settings::load(&paths.config_dir);
    assert_eq!(settings.issue, None);
    let catalog = Catalog::scan(&CatalogRoots::from_paths(&paths));
    assert!(catalog.issues().is_empty(), "{:?}", catalog.issues());

    // 3. The vault: opened before any window, then created, locked and unlocked like Settings
    //    does. Its decrypted session folder is inside installDir too.
    let mut config = VaultConfig::new(paths.vault_dir(), paths.vault_session_dir());
    config.kdf = KdfParams::MINIMUM;
    let (vault, _report) = Vault::open(config)?;
    let password = SecretString::from("a password for the no trace test");
    vault.create(&password)?;
    vault.lock(LockPolicy::SyncThenWipe)?;
    vault.unlock(&password)?;
    vault.lock(LockPolicy::Force)?;
    drop(vault);

    // 4. What the launcher persists while it runs: a setting and the recent list.
    let settings_file = paths.config_dir.join(SETTINGS_FILE);
    write_value(&settings_file, "appearance", "theme", "snow-storm".into())?;
    let recent_file = recent_path(&paths);
    let mut recent = RecentLaunches::load(&recent_file);
    recent.record(AppId::new(Source::Genslate, "explorer"), 1, &recent_file)?;

    // Something really happened inside installDir...
    assert!(paths.vault_dir().join("files").is_dir());
    assert!(settings_file.is_file());
    assert!(recent_file.is_file());
    // ...and nothing outside it.
    guard.assert_unchanged();
    Ok(())
}

#[test]
fn the_guard_would_notice_a_write_to_an_os_folder() -> TestResult {
    // The test above only means something if the guard sees writes: leave a file in a watched
    // folder on purpose and expect the report.
    let tmp = TempTree::new()?;
    let watched = tmp.join("AppData");
    fs::create_dir_all(&watched)?;
    let guard = NoTraceGuard::watching([watched.as_path()])?;
    guard.assert_unchanged();
    fs::create_dir_all(watched.join("xyz.genslate.slatecore.launcher"))?;
    let changes = guard.changes()?;
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert!(changes[0].contains("xyz.genslate.slatecore.launcher"));
    Ok(())
}

#[test]
fn the_shipped_settings_seed_loads_as_the_defaults() {
    // `configs/settings.toml` goes into the package as it is: no issue, and it changes nothing.
    let seed = Path::new(env!("CARGO_MANIFEST_DIR")).join("../other/launcher/configs");
    let settings = Settings::load(&seed);
    assert_eq!(settings.issue, None);
    assert_eq!(settings, Settings::default());
}
