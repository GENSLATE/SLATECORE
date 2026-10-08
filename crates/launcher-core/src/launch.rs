//! Starting apps safely.
//!
//! The UI only ever sends an [`crate::catalog::AppId`]; the program path comes from the catalog
//! and must resolve (after following links) inside an allowed root: `programs/`, or `target/`
//! in dev. Apps are started detached, in their own folder, with no inherited stdio, and
//! without the `WebView2` variables the launcher sets for itself.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::LauncherError;
use crate::catalog::{AppEntry, AppStatus};

/// Variables with this prefix point the launcher's own `WebView2` at the install folder. A
/// started app that embeds `WebView2` must not inherit them: it would share (and lock) the
/// launcher's data folder.
const WEBVIEW2_PREFIX: &str = "WEBVIEW2_";

/// A validated launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    /// Canonical program path.
    pub program: PathBuf,
    /// Arguments.
    pub args: Vec<String>,
    /// Working directory: the app's own folder.
    pub cwd: PathBuf,
}

impl LaunchSpec {
    /// Validates `app` for launching. `args` replaces the app's configured arguments.
    pub fn resolve(
        app: &AppEntry,
        args: Option<Vec<String>>,
        allowed_roots: &[PathBuf],
    ) -> Result<Self, LauncherError> {
        let unavailable = |reason| LauncherError::Unavailable {
            app: app.name.clone(),
            reason,
        };
        match app.status {
            AppStatus::NotInstalled => return Err(LauncherError::NotInstalled(app.name.clone())),
            AppStatus::MissingExe => return Err(unavailable("its program is missing")),
            AppStatus::BrokenManifest => return Err(unavailable("its manifest is damaged")),
            AppStatus::Ready | AppStatus::Running => {}
        }
        let program = app
            .program
            .as_deref()
            .ok_or_else(|| unavailable("it has no program to start"))?;
        let program = dunce::canonicalize(program).map_err(|source| LauncherError::Spawn {
            path: program.to_path_buf(),
            source,
        })?;
        let inside = allowed_roots
            .iter()
            .filter_map(|root| dunce::canonicalize(root).ok())
            .any(|root| program.starts_with(&root) && program != root);
        if !inside {
            return Err(LauncherError::OutsideRoot(program));
        }
        let cwd = program
            .parent()
            .map_or_else(|| program.clone(), Path::to_path_buf);
        Ok(Self {
            args: args.unwrap_or_else(|| app.args.clone()),
            program,
            cwd,
        })
    }

    /// The command that starts the app.
    pub fn command(&self) -> Command {
        self.command_without_webview2(std::env::vars_os().map(|(name, _)| name))
    }

    /// [`LaunchSpec::command`] for an environment that has the variables `names`.
    fn command_without_webview2(&self, names: impl Iterator<Item = OsString>) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for name in names.filter(|name| is_webview2_variable(name)) {
            command.env_remove(name);
        }
        detach(&mut command);
        command
    }

    /// Starts the app and returns its process id.
    pub fn spawn(&self) -> Result<u32, LauncherError> {
        self.command()
            .spawn()
            .map(|child| child.id())
            .map_err(|source| LauncherError::Spawn {
                path: self.program.clone(),
                source,
            })
    }
}

/// Environment variable names are case-insensitive on Windows.
fn is_webview2_variable(name: &OsStr) -> bool {
    name.to_str()
        .is_some_and(|name| name.to_ascii_uppercase().starts_with(WEBVIEW2_PREFIX))
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    /// Own process group: Ctrl+C / console close of the launcher never reach the app.
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

/// Only Windows ships; this keeps the crate building (and its tests running) on other hosts.
#[cfg(not(windows))]
fn detach(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{AppId, AppStatus, Source};
    use genslate_testing::TempTree;

    fn app(program: PathBuf) -> AppEntry {
        let mut app = AppEntry::new(AppId::new(Source::Genslate, "x"), "X");
        app.program = Some(program);
        app.args = vec!["--default".to_owned()];
        app
    }

    #[test]
    fn accepts_programs_inside_the_roots() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?.file("programs/genslate/x/x.exe", "")?;
        let spec = LaunchSpec::resolve(
            &app(tree.join("programs/genslate/x/x.exe")),
            None,
            &[tree.join("programs")],
        )?;
        assert!(spec.program.ends_with("x.exe"));
        assert!(spec.cwd.ends_with("x"));
        assert_eq!(spec.args, ["--default"]);
        let custom = LaunchSpec::resolve(
            &app(tree.join("programs/genslate/x/x.exe")),
            Some(vec!["--a".to_owned()]),
            &[tree.join("programs")],
        )?;
        assert_eq!(custom.args, ["--a"]);
        Ok(())
    }

    #[test]
    fn rejects_programs_outside_the_roots() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?
            .file("outside/evil.exe", "")?
            .dir("programs")?;
        let sneaky = tree.join("programs/../outside/evil.exe");
        let error = LaunchSpec::resolve(&app(sneaky), None, &[tree.join("programs")])
            .err()
            .ok_or("expected an error")?;
        assert_eq!(error.kind(), "outside-root");
        Ok(())
    }

    #[test]
    fn refuses_apps_that_are_unavailable() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?.file("programs/x.exe", "")?;
        for (status, kind, wording) in [
            (AppStatus::NotInstalled, "not-installed", "is not installed"),
            (
                AppStatus::MissingExe,
                "unavailable",
                "its program is missing",
            ),
            (
                AppStatus::BrokenManifest,
                "unavailable",
                "its manifest is damaged",
            ),
        ] {
            let mut unavailable = app(tree.join("programs/x.exe"));
            unavailable.status = status;
            let error = LaunchSpec::resolve(&unavailable, None, &[tree.join("programs")])
                .err()
                .ok_or("expected an error")?;
            assert_eq!(error.kind(), kind, "{status:?}");
            assert!(error.to_string().contains(wording), "{error}");
            if kind == "unavailable" {
                assert!(
                    !error.to_string().contains("not installed"),
                    "a damaged app is not 'not installed': {error}"
                );
            }
        }
        // Ready but without a program cannot happen from a scan; it is still refused, not a panic.
        let mut no_program = app(tree.join("programs/x.exe"));
        no_program.program = None;
        let error = LaunchSpec::resolve(&no_program, None, &[tree.join("programs")])
            .err()
            .ok_or("expected an error")?;
        assert_eq!(error.kind(), "unavailable");
        Ok(())
    }

    #[test]
    fn started_apps_do_not_inherit_the_launchers_webview2_variables()
    -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?.file("programs/x/x.exe", "")?;
        let spec = LaunchSpec::resolve(
            &app(tree.join("programs/x/x.exe")),
            None,
            &[tree.join("programs")],
        )?;
        let names = [
            "WEBVIEW2_USER_DATA_FOLDER",
            "webview2_browser_executable_folder",
            "PATH",
            "TEMP",
        ]
        .map(OsString::from);
        let command = spec.command_without_webview2(names.into_iter());
        let mut removed: Vec<String> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect();
        removed.sort();
        assert_eq!(
            removed,
            [
                "WEBVIEW2_USER_DATA_FOLDER",
                "webview2_browser_executable_folder"
            ],
            "TEMP and PATH are left as they are"
        );
        assert_eq!(command.get_current_dir(), Some(spec.cwd.as_path()));
        assert_eq!(command.get_program(), spec.program.as_os_str());
        Ok(())
    }
}
