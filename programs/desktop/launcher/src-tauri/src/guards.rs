//! Guard tests: rules of the leave-no-trace contract that no type can express, checked against
//! the source and the JSON files. They read files, not Tauri, so they need no Windows API.

// cspell:ignore nosniff

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use genslate_launcher_core::actions::{Effect, REGISTRY};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

/// The shell crate's folder (`.../src-tauri`).
fn shell_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn manifest(relative: &str) -> PathBuf {
    shell_dir().join(relative)
}

fn read(relative: &str) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(manifest(relative))?)
}

fn json(relative: &str) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_str(&read(relative)?)?)
}

/// The SLATECORE checkout: `programs/desktop/launcher/src-tauri` is four folders below it.
fn workspace_root() -> PathBuf {
    manifest("../../../..")
}

/// Every `.rs` file below `dir`.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// This file names the patterns it looks for, so it is never scanned.
fn is_this_file(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "guards.rs")
}

fn shell_sources() -> Vec<PathBuf> {
    let mut files = Vec::new();
    rust_files(&manifest("src"), &mut files);
    files.retain(|path| !is_this_file(path));
    files
}

// ── autostart ────────────────────────────────────────────────────────────────

#[test]
fn autostart_module_does_not_exist() -> TestResult {
    assert!(
        !manifest("src/autostart.rs").exists(),
        "the launcher never writes the PC's startup entries"
    );
    for file in shell_sources() {
        let text = fs::read_to_string(&file)?.to_lowercase();
        assert!(
            !text.contains("autostart"),
            "{} mentions autostart",
            file.display()
        );
    }
    for relative in [
        "Cargo.toml",
        "tauri.conf.json",
        "capabilities/main.capability.json",
        "capabilities/tray-menu.capability.json",
    ] {
        assert!(
            !read(relative)?.to_lowercase().contains("autostart"),
            "{relative} mentions autostart"
        );
    }
    Ok(())
}

// ── unsafe ───────────────────────────────────────────────────────────────────

/// Whether `text` holds `word` as a whole identifier (`unsafe_code` is not `unsafe`).
fn has_word(text: &str, word: &str) -> bool {
    let is_name = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(is_name) && !after.is_some_and(is_name)
    })
}

/// Whether a line of code uses `unsafe` in any form, or switches the lint that forbids it off:
/// `unsafe {`, `unsafe fn`, `unsafe impl`, `unsafe extern`, `#[unsafe(no_mangle)]`,
/// `#[allow(unsafe_code)]`, `#[expect(unsafe_code)]` and the lint name on a line of its own in a
/// multi-line attribute. Comment lines are skipped; a line may still mention the word in a string.
fn uses_unsafe(line: &str) -> bool {
    let line = line.trim_start();
    if line.starts_with("//") {
        return false;
    }
    let lowers_the_lint =
        line.contains("unsafe_code") && !line.contains("deny") && !line.contains("forbid");
    has_word(line, "unsafe") || lowers_the_lint
}

#[test]
fn the_unsafe_check_sees_every_spelling() {
    for line in [
        "unsafe { std::env::set_var(a, b) };",
        "    pub unsafe fn f() {}",
        "unsafe impl Send for X {}",
        "unsafe extern \"C\" {}",
        "#[unsafe(no_mangle)]",
        "#[allow(unsafe_code)]",
        "#[expect(unsafe_code, reason = \"x\")]",
        "#![allow(unsafe_code)]",
        "    unsafe_code,",
        "let x = 1; unsafe {}",
    ] {
        assert!(uses_unsafe(line), "{line}");
    }
    for line in [
        "// unsafe { not code }",
        "/// unsafe is described here",
        "#![deny(unsafe_code)]",
        "#![forbid(unsafe_code)]",
        "let unsafe_count = 1;",
        "fn is_unsafely_named() {}",
    ] {
        assert!(!uses_unsafe(line), "{line}");
    }
}

#[test]
fn only_env_bootstrap_contains_unsafe() -> TestResult {
    let root = workspace_root();
    let mut folders = vec![manifest("src"), manifest("tests"), manifest("build.rs")];
    let crates = fs::read_dir(root.join("crates"))?;
    for entry in crates.flatten() {
        for part in ["src", "tests", "benches", "examples", "build.rs"] {
            folders.push(entry.path().join(part));
        }
    }
    let mut files = Vec::new();
    for folder in &folders {
        if folder.is_file() {
            files.push(folder.clone());
        } else {
            rust_files(folder, &mut files);
        }
    }
    files.retain(|path| !is_this_file(path));
    assert!(
        files.len() > 20,
        "the scan found only {} files",
        files.len()
    );
    assert!(
        files.contains(&manifest("build.rs")),
        "the shell's build.rs is scanned"
    );

    let mut users = BTreeSet::new();
    for file in &files {
        if fs::read_to_string(file)?.lines().any(uses_unsafe)
            && let Some(name) = file.file_name()
        {
            users.insert(name.to_string_lossy().into_owned());
        }
    }
    assert_eq!(
        users,
        BTreeSet::from(["env_bootstrap.rs".to_owned()]),
        "only env_bootstrap.rs may contain unsafe code"
    );
    Ok(())
}

// ── vault commands ───────────────────────────────────────────────────────────

#[test]
fn vault_commands_are_all_async() -> TestResult {
    let source = read("src/vault_commands.rs")?;
    let attributes = source.matches("#[tauri::command]").count();
    let async_commands = source.matches("pub async fn vault_").count();
    assert_eq!(attributes, 10, "the vault has ten commands");
    assert_eq!(async_commands, attributes, "every vault command is async");
    assert!(
        !source.contains("pub fn vault_"),
        "a vault command is not async"
    );
    assert!(
        source.contains("spawn_blocking"),
        "vault work runs on a blocking thread"
    );
    Ok(())
}

// ── commands, handler, capabilities ──────────────────────────────────────────

/// The string literals of `build.rs`'s `COMMANDS`.
fn build_rs_commands() -> Result<BTreeSet<String>, Box<dyn Error>> {
    let source = read("build.rs")?;
    let start = source.find("const COMMANDS").ok_or("no COMMANDS")?;
    let body = &source[start..];
    let body = &body[body.find('[').ok_or("no [")? + 1..];
    let body = &body[..body.find("];").ok_or("no ];")?];
    Ok(body
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect())
}

/// The command names inside `generate_handler![...]` in `lib.rs`.
fn handler_commands() -> Result<BTreeSet<String>, Box<dyn Error>> {
    let source = read("src/lib.rs")?;
    let start = source.find("generate_handler![").ok_or("no handler")?;
    let body = &source[start + "generate_handler![".len()..];
    let body = &body[..body.find(']').ok_or("no ]")?];
    Ok(body
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.split("::").last().unwrap_or(entry).to_owned())
        .collect())
}

fn permissions(relative: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
    json(relative)?["permissions"]
        .as_array()
        .ok_or("no permissions array")?
        .iter()
        .map(|permission| {
            permission
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "a permission is not a string".into())
        })
        .collect()
}

fn permission_for(command: &str) -> String {
    format!("allow-{}", command.replace('_', "-"))
}

#[test]
fn build_rs_handler_and_capabilities_name_the_same_commands() -> TestResult {
    let declared = build_rs_commands()?;
    let handled = handler_commands()?;
    assert_eq!(declared.len(), 29, "{declared:?}");
    assert_eq!(
        declared, handled,
        "build.rs COMMANDS and generate_handler! differ"
    );

    let main = permissions("capabilities/main.capability.json")?;
    let tray = permissions("capabilities/tray-menu.capability.json")?;
    for command in &declared {
        let permission = permission_for(command);
        assert!(
            main.contains(&permission) || tray.contains(&permission),
            "{command} is granted to no window"
        );
    }
    let allowed: BTreeSet<String> = declared
        .iter()
        .map(|command| permission_for(command))
        .collect();
    for permission in main.iter().chain(&tray) {
        assert!(
            allowed.contains(permission) || permission.starts_with("core:"),
            "{permission} is not a command of this app"
        );
    }
    Ok(())
}

#[test]
fn the_tray_menu_window_cannot_reach_the_vault_or_the_registry() -> TestResult {
    let tray = permissions("capabilities/tray-menu.capability.json")?;
    for forbidden in [
        "allow-run-action",
        "allow-set-app-override",
        "allow-open-app-folder",
        "allow-set-telemetry-active",
        "allow-get-volume-info",
    ] {
        assert!(
            !tray.contains(forbidden),
            "tray menu may not use {forbidden}"
        );
    }
    assert!(
        tray.iter()
            .all(|permission| !permission.starts_with("allow-vault-")),
        "the tray menu has no vault access"
    );
    Ok(())
}

#[test]
fn no_window_may_be_dragged_or_moved_by_the_webview() -> TestResult {
    for relative in [
        "capabilities/main.capability.json",
        "capabilities/tray-menu.capability.json",
    ] {
        let granted = permissions(relative)?;
        for permission in &granted {
            for banned in [
                "start-dragging",
                "start-resize-dragging",
                "set-position",
                "set-size",
                "set-focus",
                "core:default",
                "core:window:default",
                "opener:",
                "shell:",
                "fs:",
                "dialog:",
                "os:",
                "store:",
                "updater:",
                "autostart:",
                "window-state:",
            ] {
                assert!(
                    !permission.contains(banned),
                    "{relative} grants {permission}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn the_capabilities_name_their_windows() -> TestResult {
    let main = json("capabilities/main.capability.json")?;
    let tray = json("capabilities/tray-menu.capability.json")?;
    assert_eq!(main["identifier"], "main");
    assert_eq!(main["windows"], serde_json::json!(["main"]));
    assert_eq!(tray["identifier"], "tray-menu");
    assert_eq!(tray["windows"], serde_json::json!(["tray-menu"]));
    let conf = json("tauri.conf.json")?;
    assert_eq!(
        conf["app"]["security"]["capabilities"],
        serde_json::json!(["main", "tray-menu"])
    );
    Ok(())
}

#[test]
fn run_action_handles_every_registry_id_that_is_not_a_ui_action() -> TestResult {
    let source = read("src/actions.rs")?;
    for spec in REGISTRY {
        let ui_only = matches!(spec.effect, Effect::Ui | Effect::Ai);
        let handled = source.contains(&format!("\"{}\" =>", spec.id));
        assert_eq!(
            handled, !ui_only,
            "/{} ({:?}) must be handled by run_action unless the UI does it",
            spec.id, spec.effect
        );
    }
    Ok(())
}

// ── plugins, windows, start-up ───────────────────────────────────────────────

#[test]
fn only_the_allowed_plugins_are_registered() -> TestResult {
    let allowed: BTreeSet<&str> = ["single_instance", "log", "global_shortcut"].into();
    let mut seen = BTreeSet::new();
    for file in shell_sources() {
        let text = fs::read_to_string(&file)?;
        for part in text.split("tauri_plugin_").skip(1) {
            let name: String = part
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || *c == '_')
                .collect();
            let name = name.trim_end_matches('_').to_owned();
            // `tauri_plugin_log::Builder` and friends: the name ends at the `::`.
            seen.insert(name);
        }
    }
    let seen: BTreeSet<&str> = seen.iter().map(String::as_str).collect();
    assert!(seen.is_subset(&allowed), "unexpected plugins: {seen:?}");

    let manifest_text = read("Cargo.toml")?;
    for line in manifest_text
        .lines()
        .filter(|line| line.starts_with("tauri-plugin-"))
    {
        let name = line.split(['.', ' ', '=']).next().unwrap_or_default();
        assert!(
            [
                "tauri-plugin-single-instance",
                "tauri-plugin-log",
                "tauri-plugin-global-shortcut"
            ]
            .contains(&name),
            "{name} is not an allowed plugin"
        );
    }
    Ok(())
}

#[test]
fn windows_are_built_only_by_the_portable_builder() -> TestResult {
    for file in shell_sources() {
        let text = fs::read_to_string(&file)?;
        let builds_windows = text.contains("WebviewWindowBuilder::new(")
            || text.contains("WindowBuilder::new(")
            || text.contains("WebviewBuilder::new(");
        let is_window_rs = file.file_name().is_some_and(|name| name == "window.rs");
        assert_eq!(
            builds_windows,
            is_window_rs,
            "{} builds a window outside window.rs",
            file.display()
        );
    }
    let window = read("src/window.rs")?;
    for required in [
        ".data_directory(paths.webview2_dir",
        ".incognito(true)",
        ".shadow(false)",
        ".skip_taskbar(true)",
        ".decorations(false)",
        ".transparent(true)",
        ".resizable(false)",
        ".visible(false)",
        ".on_navigation(",
        ".on_new_window(",
        "NewWindowResponse::Deny",
    ] {
        assert!(
            window.contains(required),
            "portable_builder lacks {required}"
        );
    }
    assert!(
        read("src/tray_menu.rs")?.contains("portable_builder("),
        "the tray menu window is built by portable_builder"
    );
    Ok(())
}

#[test]
fn no_window_is_created_by_the_config_so_none_gets_the_default_data_folder() -> TestResult {
    let conf = json("tauri.conf.json")?;
    assert_eq!(conf["app"]["windows"], serde_json::json!([]));
    assert_eq!(conf["bundle"]["active"], false);
    assert_eq!(
        conf["bundle"]["windows"]["webviewInstallMode"]["type"],
        "skip"
    );
    assert!(
        conf["app"]["security"]["csp"]
            .as_str()
            .is_some_and(|csp| csp.contains("launcher-icon:") && !csp.contains("unsafe-eval")),
        "the CSP serves the icon scheme and allows no eval"
    );
    Ok(())
}

#[test]
fn the_csp_shuts_the_doors_the_ui_does_not_use() -> TestResult {
    let conf = json("tauri.conf.json")?;
    for key in ["csp", "devCsp"] {
        let csp = conf["app"]["security"][key]
            .as_str()
            .ok_or_else(|| format!("no {key}"))?;
        for directive in [
            "default-src 'self'",
            "base-uri 'none'",
            "form-action 'none'",
            "frame-ancestors 'none'",
            "object-src 'none'",
        ] {
            assert!(csp.contains(directive), "{key} lacks {directive}");
        }
        assert!(!csp.contains("unsafe-eval"), "{key} allows eval");
        assert!(!csp.contains('*'), "{key} has a wildcard");
    }
    Ok(())
}

#[test]
fn vault_events_go_to_the_launcher_window_only_and_logs_name_no_vault_entries() -> TestResult {
    let commands = read("src/vault_commands.rs")?;
    assert!(
        !commands.contains("reload::emit;") && !commands.contains(" emit(app"),
        "vault events are sent with emit_main, never broadcast"
    );
    assert!(commands.contains("emit_main(app, events::VAULT_LOCK_BLOCKED"));
    // A vault error's text can name an entry (NotFound, Exists): logs carry its code only.
    for relative in ["src/vault_commands.rs", "src/vault_service.rs"] {
        for line in read(relative)?
            .lines()
            .filter(|line| line.contains("log::"))
        {
            assert!(
                !line.contains("{error}") && !line.contains(".message"),
                "{relative} logs a vault error text: {line}"
            );
        }
    }
    let lib = read("src/lib.rs")?;
    assert!(!lib.contains("the vault is unavailable: {error}"));
    assert!(!lib.contains("lock on exit: {error}"));
    Ok(())
}

#[test]
fn the_icon_scheme_answers_nosniff() -> TestResult {
    assert!(read("src/icons.rs")?.contains("X_CONTENT_TYPE_OPTIONS"));
    Ok(())
}

#[test]
fn vault_open_refuses_the_types_that_run_code_before_it_decrypts_anything() -> TestResult {
    let source = read("src/vault_commands.rs")?;
    let command = source
        .find("pub async fn vault_open")
        .ok_or("no vault_open")?;
    let body = &source[command..];
    let check = body.find("is_blocked_type(").ok_or("no policy check")?;
    let decrypt = body.find("open_entry(").ok_or("no decrypt")?;
    assert!(
        check < decrypt,
        "the policy runs after the file is decrypted"
    );
    assert!(body.contains("VaultErrorDto::blocked_type()"));
    Ok(())
}

#[test]
fn launch_app_knows_who_is_calling_and_set_app_override_caps_its_arguments() -> TestResult {
    let source = read("src/commands.rs")?;
    let launch = source.find("pub fn launch_app").ok_or("no launch_app")?;
    let launch = &source[launch..];
    let launch = &launch[..launch.find("\n}\n").ok_or("no end")?];
    assert!(
        launch.contains("window: WebviewWindow"),
        "the caller's window"
    );
    assert!(launch.contains("launch_args::check_launch(window.label()"));
    let set = source
        .find("pub fn set_app_override")
        .ok_or("no override")?;
    assert!(source[set..].contains("launch_args::check_list("));
    Ok(())
}

#[test]
fn the_shell_has_no_code_for_any_other_system() -> TestResult {
    assert!(
        !read("Cargo.toml")?.contains("[target."),
        "dependencies are not split by target: the shell is Windows-only"
    );
    assert!(
        !read("Cargo.toml")?.contains("dynamic-acl"),
        "dynamic-acl is for plugins that add permissions at run time; there are none"
    );
    for file in shell_sources() {
        let text = fs::read_to_string(&file)?;
        for banned in [
            "cfg(not(windows))",
            "cfg(unix)",
            "cfg(not(target_os",
            "target_os = \"linux\"",
            "target_os = \"macos\"",
            "std::os::unix",
        ] {
            assert!(
                !text.contains(banned),
                "{} has code for another system ({banned})",
                file.display()
            );
        }
    }
    Ok(())
}

#[test]
fn start_up_runs_in_the_documented_order() -> TestResult {
    let lib = read("src/lib.rs")?;
    let at = |needle: &str| {
        lib.find(needle)
            .ok_or_else(|| format!("lib.rs lacks {needle}"))
    };

    assert!(at("refuse_if_elevated")? < at("genslate_paths::resolve")?);
    assert!(at("genslate_paths::resolve")? < at("runtime::select(")?);
    assert!(at("runtime::select(")? < at("env_bootstrap::apply(")?);
    assert!(at("env_bootstrap::apply(")? < at("generate_context!()")?);
    assert!(at("generate_context!()")? < at("tauri::Builder::default()")?);
    let after_builder = &lib[at("tauri::Builder::default()")?..];
    let first_plugin = after_builder.find(".plugin(").ok_or("no plugin")?;
    assert!(
        after_builder[first_plugin..].starts_with(".plugin(tauri_plugin_single_instance::init"),
        "the single-instance plugin is the first plugin"
    );
    assert!(at("tauri_plugin_single_instance::init")? < at("log_plugin(&paths")?);
    // Before the first window: the web data reset and the vault (which wipes stale sessions).
    // The scratch folder is emptied only once the vault is open: its instance lock is what keeps
    // a second launch under another path spelling from emptying the running launcher's TEMP.
    assert!(at("clear_stale_data(&paths, &stamp)")? < at("open_vault(&paths)")?);
    assert!(at("open_vault(&paths)")? < at("clear_scratch_folder(&paths, &vault)")?);
    assert!(at("clear_scratch_folder(&paths, &vault)")? < at("window::create(")?);
    let stale = &lib[at("fn clear_stale_data")?..at("fn clear_scratch_folder")?];
    assert!(stale.contains("reset_if_runtime_changed"));
    assert!(
        !stale.contains("empty_temp"),
        "TEMP is not emptied before the vault lock"
    );
    let scratch = &lib[at("fn clear_scratch_folder")?..at("fn try_run")?];
    assert!(scratch.contains("temp_clean::empty_temp(&paths.temp_dir, &paths.cache_dir)"));
    assert!(scratch.contains("VaultSlot::Ready"));
    assert!(at("window::create(")? < at("window::show(handle)")?);
    // The vault is locked when the app exits.
    assert!(lib.contains("RunEvent::Exit => lock_vault_for_exit"));
    Ok(())
}

/// Tauri turns an error out of `setup` into a panic inside the event loop, and the release build
/// aborts on panic with no console: the user would see nothing. So `setup` has no `?` and no
/// `Err(..)` return, every fallible step ends in `abort_start` (a box, then exit 1) and a panic
/// hook shows one generic box.
#[test]
fn a_failed_setup_step_shows_a_box_instead_of_a_silent_panic() -> TestResult {
    let lib = read("src/lib.rs")?;
    let start = lib.find(".setup(move |app|").ok_or("no setup")?;
    let end = lib[start..]
        .find(".on_window_event")
        .ok_or("no end of setup")?
        + start;
    let setup = &lib[start..end];
    assert!(
        !setup.contains(")?"),
        "a `?` inside setup becomes a silent panic"
    );
    assert!(
        !setup.contains("return Err"),
        "setup must not return an error"
    );
    for step in ["Window", "Tray", "Show"] {
        assert!(
            setup.contains(&format!("abort_start(SetupStep::{step}")),
            "setup lacks abort_start for {step}"
        );
    }
    assert!(setup.contains("Ok(())"));
    // The hook goes in before anything can panic, and the box it shows is the generic one.
    let run = lib.find("pub fn run()").ok_or("no run")?;
    let after_run = &lib[run..];
    assert!(
        after_run
            .find("install_panic_hook()")
            .ok_or("no hook call")?
            < after_run.find("try_run()").ok_or("no try_run call")?
    );
    // The box is shown from its own thread with a bound, then the process is aborted whatever
    // the box does: a panic on the vault's thread must not leave the launcher running.
    let hook = &lib[lib.find("fn install_panic_hook").ok_or("no hook")?
        ..lib.find("fn abort_start").ok_or("no abort_start")?];
    assert!(hook.contains("fatal::bounded(CRASH_BOX_BOUND, || dialog::show(&Fatal::Crashed))"));
    assert!(hook.contains("std::process::abort()"));
    assert!(
        hook.find("fatal::bounded").ok_or("no bounded")?
            < hook.find("std::process::abort()").ok_or("no abort")?
    );
    // Locations are logged as a file name, and abort_start logs no unfiltered error text.
    assert!(hook.contains("fatal::panic_place("));
    assert!(!hook.contains("location.file()}"));
    let abort_start = &lib[lib.find("fn abort_start").ok_or("no abort_start")?
        ..lib.find("fn clear_stale_data").ok_or("no clear")?];
    assert!(abort_start.contains("error_text::without_paths("));
    Ok(())
}

/// The KDF memory cap is set when the vault is opened, and the scratch folder cannot be skipped.
#[test]
fn the_vault_gets_a_memory_cap_and_the_scratch_folder_is_emptied() -> TestResult {
    let service = read("src/vault_service.rs")?;
    assert!(service.contains("set_kdf_memory_limit(Some(kdf_limit_bytes("));
    let lib = read("src/lib.rs")?;
    assert!(lib.contains("temp_clean::empty_temp("));
    // The background loop retries what a lock could not delete, and exit tries one last time.
    let commands = read("src/vault_commands.rs")?;
    assert!(commands.contains("service.retry_leftovers()"));
    assert!(lib.contains("service.final_wipe(EXIT_WIPE_BOUND)"));
    Ok(())
}

#[test]
fn dev_url_matches_launcher_vite_port_1420() -> TestResult {
    let conf = json("tauri.conf.json")?;
    assert_eq!(conf["build"]["devUrl"], "http://localhost:1420");
    let vite = fs::read_to_string(manifest("../vite.config.ts"))?;
    assert!(vite.contains("1420"), "vite.config.ts serves port 1420");
    assert!(vite.contains("1421"), "vite.config.ts keeps HMR on 1421");
    assert!(
        conf["app"]["security"]["devCsp"]
            .as_str()
            .is_some_and(|csp| csp.contains("ws://localhost:1421")),
        "the dev CSP allows the HMR socket"
    );
    Ok(())
}
