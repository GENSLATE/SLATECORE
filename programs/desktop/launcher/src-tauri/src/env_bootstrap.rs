//! The process environment the launcher runs with: `TEMP`, `TMP` and the `WebView2` variables
//! all point into installDir, and nothing stale from the PC leaks in.
//!
//! [`plan`] decides the values (pure), [`apply`] sets them. `apply` is the only place in the
//! workspace that uses `unsafe` (`std::env::set_var` is `unsafe` in edition 2024); a test
//! checks that no other file does. [`webview2_variables`] names what a started program must not
//! inherit: the launcher's own `WEBVIEW2_*` variables are removed so a `WebView2` app does not
//! share (and lock) the launcher's data folder.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use genslate_paths::AppPaths;

use crate::runtime::Runtime;

/// Names (compared without regard to letter case, as Windows does) with this prefix belong to
/// `WebView2` and are never handed to a started program.
pub const WEBVIEW2_PREFIX: &str = "WEBVIEW2_";
/// The data folder variable; every window also gets the same folder from code.
pub const USER_DATA_FOLDER: &str = "WEBVIEW2_USER_DATA_FOLDER";
/// Selects a fixed-version runtime folder (set only when the bundled runtime is chosen).
pub const BROWSER_EXECUTABLE_FOLDER: &str = "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER";

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The variables to set, in order: `TEMP`, `TMP`, `WEBVIEW2_USER_DATA_FOLDER` and, for the
/// bundled runtime, `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER`. Every value is inside installDir.
///
/// `TEMP` and `TMP` come first so the `WebView2` variables are set in an environment whose
/// scratch folder is already ours. `APPDATA`, `LOCALAPPDATA` and `USERPROFILE` stay as they are:
/// Windows' own folder APIs ignore them and programs expect the real profile.
pub fn plan(paths: &AppPaths, runtime: &Runtime) -> Vec<(String, String)> {
    let mut plan = vec![
        ("TEMP".to_owned(), text(&paths.temp_dir)),
        ("TMP".to_owned(), text(&paths.temp_dir)),
        (USER_DATA_FOLDER.to_owned(), text(&paths.webview2_dir)),
    ];
    if let Runtime::Bundled(dir) = runtime {
        plan.push((BROWSER_EXECUTABLE_FOLDER.to_owned(), text(dir)));
    }
    plan
}

/// Whether `name` is a `WebView2` variable. Names are compared without regard to letter case, as
/// Windows does, and by their lossy text so a name that is not valid Unicode but starts with the
/// prefix is still caught.
///
/// `launch.rs` in `genslate-launcher-core` has the same rule as a private function
/// (`is_webview2_variable`); it is not public, so this is a second copy until the crate exports it.
fn is_webview2(name: &OsStr) -> bool {
    name.to_string_lossy()
        .to_ascii_uppercase()
        .starts_with(WEBVIEW2_PREFIX)
}

/// The `WEBVIEW2_*` variables among `current` that the plan does not set: a PC may define
/// `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER`, `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` or
/// `WEBVIEW2_RELEASE_CHANNELS`, which would silently change which runtime runs or how.
pub fn stale_vars(
    current: impl IntoIterator<Item = OsString>,
    plan: &[(String, String)],
) -> Vec<OsString> {
    webview2_variables(current)
        .into_iter()
        .filter(|name| {
            let name = name.to_string_lossy();
            !plan.iter().any(|(set, _)| set.eq_ignore_ascii_case(&name))
        })
        .collect()
}

/// The names among `names` that a started program must not inherit (every `WEBVIEW2_*`
/// variable): the caller removes them from the child's environment, like `launch.rs` does for
/// started apps. Takes `OsString` names (from `std::env::vars_os`), so no variable can panic it.
pub fn webview2_variables(names: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
    names.into_iter().filter(|name| is_webview2(name)).collect()
}

/// Removes the stale `WEBVIEW2_*` variables, then sets `plan`.
///
/// Call it once, at the top of `run`, before the Tauri builder or any thread of ours exists.
#[allow(
    unsafe_code,
    reason = "std::env::set_var is unsafe in edition 2024; this is the one place that sets variables"
)]
pub fn apply(plan: &[(String, String)]) {
    let current = std::env::vars_os().map(|(name, _)| name);
    for name in stale_vars(current, plan) {
        // SAFETY: `apply` runs once, at the start of `run`, before the Tauri builder and before
        // any thread of ours is started, so nothing reads or writes the environment while it
        // changes. On Windows the call is a plain `SetEnvironmentVariableW`.
        unsafe { std::env::remove_var(&name) };
    }
    for (name, value) in plan {
        // SAFETY: see above; the same single-threaded start-up window.
        unsafe { std::env::set_var(name, value) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_paths::{Environment, resolve_with};
    use genslate_testing::{TempTree, fake_suite};
    use std::error::Error;
    use std::path::PathBuf;

    type TestResult = Result<(), Box<dyn Error>>;

    fn paths_in(tmp: &TempTree) -> Result<AppPaths, Box<dyn Error>> {
        let root = fake_suite(tmp.path())?;
        let env = Environment {
            install_dir_override: Some(root),
            ..Environment::default()
        };
        Ok(resolve_with("launcher", &env)?)
    }

    #[test]
    fn env_plan_orders_temp_before_webview_vars_and_all_inside_install_dir() -> TestResult {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        let plan = plan(&paths, &Runtime::Host);
        let names: Vec<&str> = plan.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["TEMP", "TMP", "WEBVIEW2_USER_DATA_FOLDER"]);
        for (name, value) in &plan {
            assert!(
                PathBuf::from(value).starts_with(&paths.layout.root),
                "{name} must be inside installDir"
            );
        }

        let bundled = paths.bundled_webview2_root().join("154.0.1.2-x64");
        let plan = super::plan(&paths, &Runtime::Bundled(bundled.clone()));
        let names: Vec<&str> = plan.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            [
                "TEMP",
                "TMP",
                "WEBVIEW2_USER_DATA_FOLDER",
                "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER"
            ]
        );
        let last = plan.last().map(|(_, value)| value.as_str());
        assert_eq!(last, Some(bundled.to_string_lossy().as_ref()));
        assert!(
            plan.iter()
                .all(|(_, value)| PathBuf::from(value).starts_with(&paths.layout.root))
        );
        Ok(())
    }

    #[test]
    fn the_host_runtime_plan_sets_no_browser_folder() -> TestResult {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        let plan = plan(&paths, &Runtime::Host);
        assert!(
            plan.iter()
                .all(|(name, _)| name != BROWSER_EXECUTABLE_FOLDER)
        );
        Ok(())
    }

    #[test]
    fn child_env_scrubs_webview2_vars() {
        let names = [
            "PATH",
            "TEMP",
            "WEBVIEW2_USER_DATA_FOLDER",
            "webview2_browser_executable_folder",
            "WebView2_Release_Channels",
            "WEBVIEW",
        ]
        .map(OsString::from);
        let scrubbed = webview2_variables(names);
        assert_eq!(
            scrubbed,
            [
                "WEBVIEW2_USER_DATA_FOLDER",
                "webview2_browser_executable_folder",
                "WebView2_Release_Channels"
            ]
            .map(OsString::from)
        );
    }

    /// A variable whose name is not valid Unicode (an unpaired surrogate) must neither panic the
    /// launcher (`std::env::vars` would) nor slip through when it starts with `WEBVIEW2_`.
    /// Only Windows can build such a name, which is also the only place the shell runs.
    #[cfg(windows)]
    #[test]
    fn a_name_that_is_not_unicode_is_neither_a_panic_nor_a_free_pass() {
        use std::os::windows::ffi::OsStringExt;
        let name = |prefix: &str| {
            let mut units: Vec<u16> = prefix.encode_utf16().collect();
            units.push(0xD800);
            OsString::from_wide(&units)
        };
        let odd = name("WEBVIEW2_");
        let scrubbed = webview2_variables([odd.clone(), name("OTHER_")]);
        assert_eq!(scrubbed, [odd]);
    }

    #[test]
    fn stale_webview_vars_are_the_ones_the_plan_does_not_set() {
        let plan = vec![
            ("TEMP".to_owned(), "t".to_owned()),
            (USER_DATA_FOLDER.to_owned(), "u".to_owned()),
        ];
        let current = [
            "PATH",
            "webview2_user_data_folder",
            "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER",
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
        ]
        .map(OsString::from);
        assert_eq!(
            stale_vars(current, &plan),
            [
                "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER",
                "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"
            ]
            .map(OsString::from)
        );
    }
}
