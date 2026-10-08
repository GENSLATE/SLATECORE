//! The app catalog: GENSLATE apps plus user-installed PortableApps.com and portapps.io apps,
//! with the user's overrides (favorites, hidden, names, args) applied.
//!
//! Scanning never fails and never panics. A folder that is not an app is skipped; an app
//! whose manifest cannot be read or parsed is listed as [`AppStatus::BrokenManifest`], one
//! whose executable is missing as [`AppStatus::MissingExe`]. The UI shows both in its
//! collapsed "Unavailable" group.

mod genslate;
mod icon;
mod model;
mod portableapps;
mod portapps;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use genslate_paths::{AppPaths, Mode};
use serde::Serialize;

pub use genslate::GenslateRoots;
pub use icon::{IconData, IconSource, load_icon};
pub use model::{AppEntry, AppId, AppStatus, Source, title_case};

use crate::config::issue_text;
use crate::metadata::{TabSettings, metadata_dir};

/// Everything the catalog scans.
#[derive(Debug, Clone)]
pub struct CatalogRoots {
    /// `programs/`. Below it: `genslate/`, `portableapps.com/` and `portapps.io/`, see
    /// [`Source::folder`]. `None` scans nothing.
    pub programs: Option<PathBuf>,
    /// `other/launcher/configs/metadata/`: GENSLATE app descriptions and the per-tab settings
    /// files that hold favorites and other overrides.
    pub metadata: PathBuf,
    /// `other/launcher/resources/icons/`: `<app>.svg` for GENSLATE apps.
    pub icons: PathBuf,
    /// Dev only: `<repo>/target/debug`, where apps that are not staged into `programs/` yet
    /// are built.
    pub dev_target: Option<PathBuf>,
}

impl CatalogRoots {
    /// The roots of a resolved install: nothing here is stored, so a different drive letter
    /// next time is fine.
    pub fn from_paths(paths: &AppPaths) -> Self {
        Self {
            programs: paths.layout.programs.clone(),
            metadata: metadata_dir(paths),
            icons: paths.resources_dir().join("icons"),
            dev_target: (paths.mode() == Mode::Dev)
                .then(|| paths.layout.root.join("target").join("debug")),
        }
    }

    /// The folder `source`'s apps live in: `programs/<`[`Source::folder`]`>`.
    pub fn source_dir(&self, source: Source) -> Option<PathBuf> {
        self.programs
            .as_ref()
            .map(|programs| programs.join(source.folder()))
    }

    /// The folders a program may be started from (see [`crate::launch::LaunchSpec::resolve`]):
    /// `programs/`, plus the dev target dir.
    pub fn launch_roots(&self) -> Vec<PathBuf> {
        self.programs
            .iter()
            .chain(self.dev_target.iter())
            .cloned()
            .collect()
    }
}

/// One launcher tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabInfo {
    /// The source the tab lists.
    pub source: Source,
    /// The tab's label.
    pub label: &'static str,
    /// Visible apps in the tab (zero for a source with no apps).
    pub count: usize,
}

/// All scanned apps plus the tab settings they were scanned with.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    apps: Vec<AppEntry>,
    tabs: BTreeMap<Source, TabSettings>,
    issues: Vec<String>,
}

/// The launcher's own key (`programs/genslate/launcher/`): never listed.
const SELF_KEY: &str = crate::APP_NAME;

impl Catalog {
    /// Scans every source. Missing folders simply yield no apps.
    pub fn scan(roots: &CatalogRoots) -> Self {
        let mut issues = Vec::new();
        let tabs: BTreeMap<Source, TabSettings> = Source::ALL
            .into_iter()
            .map(|source| {
                let path = roots.metadata.join(source.settings_file());
                let settings = TabSettings::load(&path).unwrap_or_else(|error| {
                    log::warn!("{error}; using default tab settings");
                    issues.push(issue_text(&error));
                    TabSettings::default()
                });
                (source, settings)
            })
            .collect();

        // Without a `programs/` folder there is nothing to scan; a relative path must never be
        // resolved against the working directory instead.
        let mut apps = roots
            .source_dir(Source::Genslate)
            .map(|programs| {
                genslate::scan(&GenslateRoots {
                    programs,
                    metadata: roots.metadata.clone(),
                    icons: roots.icons.clone(),
                    dev_target: roots.dev_target.clone(),
                    exclude: vec![SELF_KEY.to_owned()],
                })
            })
            .unwrap_or_default();
        if let Some(dir) = roots.source_dir(Source::Portapps) {
            apps.extend(portapps::scan(&dir));
        }
        if let Some(dir) = roots.source_dir(Source::PortableApps) {
            apps.extend(portableapps::scan(&dir));
        }
        for app in &mut apps {
            if let Some(settings) = tabs.get(&app.id.source) {
                apply_override(app, settings);
            }
        }
        apps.sort_by(|a, b| {
            (a.id.source, a.name.to_lowercase()).cmp(&(b.id.source, b.name.to_lowercase()))
        });
        Self { apps, tabs, issues }
    }

    /// Why a tab settings file (`genslate.toml`, `portapps.toml`, `portableapps.toml`) was
    /// ignored, one line each, naming the file and never its folder. An ignored file means its
    /// favorites and overrides are not applied until it is fixed. Empty when all are fine.
    pub fn issues(&self) -> &[String] {
        &self.issues
    }

    /// Every app (hidden ones included; the UI filters them).
    pub fn apps(&self) -> &[AppEntry] {
        &self.apps
    }

    /// Apps of one source.
    pub fn by_source(&self, source: Source) -> impl Iterator<Item = &AppEntry> {
        self.apps.iter().filter(move |app| app.id.source == source)
    }

    /// Looks an app up by id.
    pub fn find(&self, id: &AppId) -> Option<&AppEntry> {
        self.apps.iter().find(|app| &app.id == id)
    }

    /// The tabs to show, in order: GENSLATE, portapps.io, PortableApps.com. A source with no
    /// apps still has its tab, with a count of zero, so the UI can draw an empty state instead
    /// of a blank panel. Only a tab the user turned off (`enabled = false` in its settings
    /// file) is left out, and GENSLATE cannot be turned off.
    pub fn tabs(&self) -> Vec<TabInfo> {
        let mut tabs: Vec<(u32, TabInfo)> = Source::ALL
            .into_iter()
            .enumerate()
            .filter_map(|(index, source)| {
                let settings = self.tabs.get(&source).cloned().unwrap_or_default();
                let show = source == Source::Genslate || settings.enabled;
                let order = settings
                    .order
                    .unwrap_or(u32::try_from(index).unwrap_or(u32::MAX));
                show.then(|| {
                    (
                        order,
                        TabInfo {
                            source,
                            label: source.label(),
                            count: self.by_source(source).filter(|app| !app.hidden).count(),
                        },
                    )
                })
            })
            .collect();
        tabs.sort_by_key(|(order, _)| *order);
        tabs.into_iter().map(|(_, tab)| tab).collect()
    }

    /// Marks apps whose program is among `running` (executable paths of live processes) as
    /// [`AppStatus::Running`], and running→ready for ones that stopped. Returns whether
    /// anything changed.
    pub fn update_running(&mut self, running: &[PathBuf]) -> bool {
        let running: Vec<String> = running.iter().map(|path| normalize(path)).collect();
        let mut changed = false;
        for app in &mut self.apps {
            let Some(program) = &app.program else {
                continue;
            };
            let program = normalize(program);
            let is_running = running.iter().any(|exe| exe == &program);
            let next = match (app.status, is_running) {
                (AppStatus::Ready, true) => AppStatus::Running,
                (AppStatus::Running, false) => AppStatus::Ready,
                (status, _) => status,
            };
            changed |= next != app.status;
            app.status = next;
        }
        changed
    }
}

fn apply_override(app: &mut AppEntry, settings: &TabSettings) {
    let Some(overrides) = settings.apps.get(&app.id.key) else {
        return;
    };
    app.favorite = overrides.favorite;
    app.hidden = overrides.hidden;
    if let Some(name) = overrides
        .name
        .as_ref()
        .filter(|name| !name.trim().is_empty())
    {
        name.trim().clone_into(&mut app.name);
    }
    if let Some(category) = overrides.category.as_ref().filter(|c| !c.trim().is_empty()) {
        category.trim().clone_into(&mut app.category);
    }
    if let Some(args) = &overrides.args {
        app.args.clone_from(args);
    }
}

/// Comparable path text: forward slashes; case-insensitive on Windows.
fn normalize(path: &Path) -> String {
    let text = dunce::simplified(path).to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_paths::{AppPaths, Environment, resolve_with};
    use genslate_testing::{TempTree, fake_repo, fake_suite};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn roots(tree: &TempTree) -> CatalogRoots {
        CatalogRoots {
            programs: Some(tree.join("programs")),
            metadata: tree.join("metadata"),
            icons: tree.join("icons"),
            dev_target: None,
        }
    }

    fn suite() -> std::io::Result<TempTree> {
        TempTree::new()?
            .file("metadata/explorer.toml", "[app]\nname = \"Explorer\"\n")?
            .file("metadata/launcher.toml", "[app]\nname = \"Launcher\"\n")?
            .file(
                "metadata/genslate.toml",
                "[apps.explorer]\nfavorite = true\nname = \"Files\"\nargs = [\"--new\"]\n",
            )?
            .file("metadata/portapps.toml", "enabled = false\n")?
            .file("programs/genslate/explorer/slatecore-explorer.exe", "")?
            .file("programs/genslate/launcher/slatecore-launcher.exe", "")?
            .file(
                "programs/portableapps.com/ToolPortable/ToolPortable.exe",
                "",
            )?
            .file(
                "programs/portapps.io/x-portable/portapp.json",
                r#"{"id":"x-portable"}"#,
            )?
            .file("programs/portapps.io/x-portable/x-portable.exe", "")
    }

    #[test]
    fn applies_overrides_and_skips_the_launcher() -> TestResult {
        let tree = suite()?;
        let catalog = Catalog::scan(&roots(&tree));
        let explorer = catalog
            .find(&AppId::new(Source::Genslate, "explorer"))
            .ok_or("explorer")?;
        assert!(explorer.favorite);
        assert_eq!(explorer.name, "Files");
        assert_eq!(explorer.args, ["--new"]);
        assert!(
            catalog
                .find(&AppId::new(Source::Genslate, "launcher"))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn a_disabled_tab_is_hidden_and_the_rest_keep_the_launcher_order() -> TestResult {
        let tree = suite()?;
        let tabs = Catalog::scan(&roots(&tree)).tabs();
        let sources: Vec<Source> = tabs.iter().map(|tab| tab.source).collect();
        assert_eq!(sources, [Source::Genslate, Source::PortableApps]);
        assert_eq!(tabs[1].count, 1);

        // Without the `enabled = false` file all three show: GENSLATE, portapps.io,
        // PortableApps.com.
        let enabled = TempTree::new()?
            .file("programs/genslate/explorer/slatecore-explorer.exe", "")?
            .dir("programs/portapps.io")?
            .dir("programs/portableapps.com")?;
        let tabs = Catalog::scan(&roots(&enabled)).tabs();
        assert_eq!(
            tabs.iter().map(|tab| tab.label).collect::<Vec<_>>(),
            ["GENSLATE", "portapps.io", "PortableApps.com"]
        );
        Ok(())
    }

    #[test]
    fn tab_settings_can_reorder_tabs() -> TestResult {
        let tree = TempTree::new()?
            .file("metadata/portableapps.toml", "order = 0\n")?
            .file("metadata/genslate.toml", "order = 5\n")?;
        let sources: Vec<Source> = Catalog::scan(&roots(&tree))
            .tabs()
            .iter()
            .map(|tab| tab.source)
            .collect();
        assert_eq!(
            sources,
            [Source::PortableApps, Source::Portapps, Source::Genslate]
        );
        Ok(())
    }

    #[test]
    fn tracks_running_programs() -> TestResult {
        let tree = suite()?;
        let mut catalog = Catalog::scan(&roots(&tree));
        let exe = tree.join("programs/genslate/explorer/slatecore-explorer.exe");
        let id = AppId::new(Source::Genslate, "explorer");
        assert!(catalog.update_running(&[exe]));
        assert_eq!(
            catalog.find(&id).map(|a| a.status),
            Some(AppStatus::Running)
        );
        assert!(catalog.update_running(&[]));
        assert_eq!(catalog.find(&id).map(|a| a.status), Some(AppStatus::Ready));
        assert!(!catalog.update_running(&[]));
        Ok(())
    }

    #[test]
    fn empty_source_yields_zero_count_tab() -> TestResult {
        let tree = TempTree::new()?
            .file("metadata/explorer.toml", "[app]\nname = \"Explorer\"\n")?
            .file("programs/genslate/explorer/slatecore-explorer.exe", "")?
            .dir("programs/portableapps.com")?
            .dir("programs/portapps.io")?;
        let tabs = Catalog::scan(&roots(&tree)).tabs();
        let summary: Vec<(Source, usize)> = tabs.iter().map(|t| (t.source, t.count)).collect();
        assert_eq!(
            summary,
            [
                (Source::Genslate, 1),
                (Source::Portapps, 0),
                (Source::PortableApps, 0)
            ]
        );
        Ok(())
    }

    #[test]
    fn no_programs_folder_at_all_is_three_empty_tabs() {
        let catalog = Catalog::scan(&CatalogRoots {
            programs: None,
            metadata: PathBuf::from("no-such-metadata"),
            icons: PathBuf::from("no-such-icons"),
            dev_target: None,
        });
        assert_eq!(catalog.apps(), []);
        let counts: Vec<usize> = catalog.tabs().iter().map(|tab| tab.count).collect();
        assert_eq!(counts, [0, 0, 0]);
    }

    #[test]
    fn hidden_apps_are_not_counted() -> TestResult {
        let tree = TempTree::new()?
            .file(
                "metadata/portapps.toml",
                "[apps.x-portable]\nhidden = true\n",
            )?
            .file(
                "programs/portapps.io/x-portable/portapp.json",
                r#"{"id":"x-portable"}"#,
            )?
            .file("programs/portapps.io/x-portable/x-portable.exe", "")?;
        let catalog = Catalog::scan(&roots(&tree));
        assert_eq!(catalog.apps().len(), 1, "hidden apps stay in the list");
        let portapps = catalog
            .tabs()
            .into_iter()
            .find(|tab| tab.source == Source::Portapps)
            .ok_or("portapps tab")?;
        assert_eq!(portapps.count, 0);
        Ok(())
    }

    fn suite_paths(tree: &TempTree) -> Result<(PathBuf, AppPaths), Box<dyn std::error::Error>> {
        let root = fake_suite(tree.path())?;
        let env = Environment {
            exe: Some(root.join("programs/genslate/launcher/slatecore-launcher.exe")),
            ..Environment::default()
        };
        let paths = resolve_with(crate::APP_NAME, &env)?;
        Ok((root, paths))
    }

    #[test]
    fn scan_roots_are_programs_genslate_portableapps_com_portapps_io() -> TestResult {
        let tree = TempTree::new()?;
        let (root, paths) = suite_paths(&tree)?;
        let roots = CatalogRoots::from_paths(&paths);
        assert_eq!(roots.programs, Some(root.join("programs")));
        assert_eq!(
            roots.source_dir(Source::Genslate),
            Some(root.join("programs/genslate"))
        );
        assert_eq!(
            roots.source_dir(Source::PortableApps),
            Some(root.join("programs/portableapps.com"))
        );
        assert_eq!(
            roots.source_dir(Source::Portapps),
            Some(root.join("programs/portapps.io"))
        );
        assert_eq!(roots.metadata, root.join("other/launcher/configs/metadata"));
        assert_eq!(roots.icons, root.join("other/launcher/resources/icons"));
        assert_eq!(roots.dev_target, None, "release layouts have no target dir");
        assert_eq!(roots.launch_roots(), [root.join("programs")]);

        // Only those three folders are scanned.
        for (file, contents) in [
            ("programs/genslate/explorer/slatecore-explorer.exe", ""),
            ("programs/portapps.io/brave-portable/brave-portable.exe", ""),
            (
                "programs/portapps.io/brave-portable/portapp.json",
                r#"{"id":"brave-portable"}"#,
            ),
            (
                "programs/portableapps.com/FirefoxPortable/FirefoxPortable.exe",
                "",
            ),
            ("programs/portableapps/WrongPortable/WrongPortable.exe", ""),
            ("programs/elsewhere/ToolPortable/ToolPortable.exe", ""),
            ("programs/portapps/brave-portable/brave-portable.exe", ""),
        ] {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().ok_or("parent")?)?;
            std::fs::write(path, contents)?;
        }
        let catalog = Catalog::scan(&roots);
        let ids: Vec<String> = catalog
            .apps()
            .iter()
            .map(|app| app.id.to_string())
            .collect();
        assert_eq!(
            ids,
            [
                "genslate/explorer",
                "portapps/brave-portable",
                "portableapps/FirefoxPortable"
            ]
        );
        for app in catalog.apps() {
            let program = app.program.as_ref().ok_or("program")?;
            assert!(program.starts_with(root.join("programs")), "{program:?}");
        }
        Ok(())
    }

    #[test]
    fn dev_mode_scans_the_mockup_and_finds_builds_in_target_debug() -> TestResult {
        let tree = TempTree::new()?;
        let repo = fake_repo(tree.path())?;
        let env = Environment {
            debug: true,
            repo_root: Some(repo.clone()),
            exe: Some(repo.join("target/debug/slatecore-launcher.exe")),
            ..Environment::default()
        };
        let paths = resolve_with(crate::APP_NAME, &env)?;
        let roots = CatalogRoots::from_paths(&paths);
        let launcher = repo.join("programs/desktop/launcher");
        assert_eq!(roots.programs, Some(launcher.join("installDir/programs")));
        assert_eq!(
            roots.metadata,
            launcher.join("other/launcher/configs/metadata")
        );
        assert_eq!(roots.dev_target, Some(repo.join("target/debug")));
        assert_eq!(
            roots.launch_roots(),
            [
                launcher.join("installDir/programs"),
                repo.join("target/debug")
            ]
        );
        Ok(())
    }

    #[test]
    fn without_a_programs_folder_nothing_is_resolved_against_the_working_directory() -> TestResult {
        // `cargo test` runs in the crate folder, which has a `src` directory full of files.
        let tree = TempTree::new()?.file("metadata/src.toml", "[app]\nname = \"Src\"\n")?;
        let catalog = Catalog::scan(&CatalogRoots {
            programs: None,
            metadata: tree.join("metadata"),
            icons: tree.join("icons"),
            dev_target: None,
        });
        assert_eq!(catalog.apps(), []);
        Ok(())
    }

    #[test]
    fn invalid_tab_settings_files_are_reported_not_just_logged() -> TestResult {
        let tree = TempTree::new()?
            .file("metadata/genslate.toml", "[apps.explorer]\nfavorite = \n")?
            .file("metadata/portapps.toml", "enabled = \"yes\"\n")?
            .file("metadata/portableapps.toml", "enabled = true\n")?
            .file("programs/genslate/explorer/slatecore-explorer.exe", "")?;
        let catalog = Catalog::scan(&roots(&tree));
        let issues = catalog.issues();
        assert_eq!(issues.len(), 2, "{issues:?}");
        assert!(issues[0].starts_with("genslate.toml: line 2"), "{issues:?}");
        assert!(issues[1].starts_with("portapps.toml:"), "{issues:?}");
        let folder = tree.path().to_string_lossy().into_owned();
        assert!(issues.iter().all(|issue| !issue.contains(&folder)));
        // The scan still works with defaults for the broken files.
        assert_eq!(catalog.apps().len(), 1);
        assert!(!catalog.apps()[0].favorite);

        // Once fixed, the next scan reports nothing.
        tree.write(
            "metadata/genslate.toml",
            "[apps.explorer]\nfavorite = true\n",
        )?;
        tree.write("metadata/portapps.toml", "enabled = true\n")?;
        let catalog = Catalog::scan(&roots(&tree));
        assert_eq!(catalog.issues(), [] as [String; 0]);
        assert!(catalog.apps()[0].favorite);
        Ok(())
    }
}
