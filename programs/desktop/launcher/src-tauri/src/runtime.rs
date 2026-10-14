//! Which `WebView2` runtime the launcher uses: the PC's own (Evergreen) one if it is recent
//! enough, otherwise a fixed-version runtime bundled in `other/shared/webview2/<version>-x64/`.
//!
//! Everything here is plain std so it is tested on any host. The registry read that finds the
//! PC's runtime is in `webview_host`.

// cspell:ignore msedgewebview
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// File that marks a folder as a `WebView2` runtime.
const RUNTIME_EXE: &str = "msedgewebview2.exe";
/// Suffix of a bundled runtime folder: `154.0.4258.62-x64`.
const FOLDER_SUFFIX: &str = "-x64";

/// The oldest host runtime the launcher accepts. The value is a starting point that has not
/// been checked against the oldest runtime Tauri 2.12 and wry 0.57 really work with; raise it
/// when a feature needs it. A host runtime below it is treated as missing.
pub const MIN_HOST_VERSION: Version = Version::new(120, 0, 0, 0);

/// A runtime version, `major.minor.build.patch` (`154.0.4258.62`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Version([u32; 4]);

impl Version {
    /// A version from its four parts.
    pub const fn new(major: u32, minor: u32, build: u32, patch: u32) -> Self {
        Self([major, minor, build, patch])
    }

    /// `true` for `0.0.0.0`, which the registry uses for "not installed".
    pub fn is_zero(self) -> bool {
        self.0 == [0; 4]
    }
}

impl FromStr for Version {
    type Err = RuntimeError;

    /// One to four dot-separated numbers; missing parts are zero.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let bad = || RuntimeError::BadVersion(text.trim().to_owned());
        let mut parts = [0_u32; 4];
        for (count, piece) in text.trim().split('.').enumerate() {
            let slot = parts.get_mut(count).ok_or_else(bad)?;
            *slot = piece.parse().map_err(|_| bad())?;
        }
        Ok(Self(parts))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [major, minor, build, patch] = self.0;
        write!(f, "{major}.{minor}.{build}.{patch}")
    }
}

/// The runtime the launcher will use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Runtime {
    /// The PC's Evergreen runtime; `WebView2` finds it by itself.
    Host,
    /// A fixed-version runtime folder inside installDir, selected through
    /// `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER`.
    Bundled(PathBuf),
}

/// Why no runtime could be chosen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeError {
    /// Neither the PC nor the drive has a usable runtime.
    #[error("no usable WebView2 runtime (PC runtime: {})", describe(.host.as_ref()))]
    NoneAvailable {
        /// The PC's runtime when one exists but is older than the floor.
        host: Option<Version>,
    },
    /// A version string that is not `a.b.c.d`.
    #[error("{0:?} is not a runtime version")]
    BadVersion(String),
}

fn describe(host: Option<&Version>) -> String {
    host.map_or_else(
        || "not installed".to_owned(),
        |version| format!("{version}, too old"),
    )
}

/// Chooses the runtime: the PC's if it is at least `floor`, else the bundled one, else an error.
///
/// `host` is the PC runtime's version (`None` if it has none) and `bundled` the folder of the
/// bundled runtime (`None` if the drive has none).
pub fn select(
    host: Option<Version>,
    bundled: Option<PathBuf>,
    floor: Version,
) -> Result<Runtime, RuntimeError> {
    match (host, bundled) {
        (Some(version), _) if version >= floor => Ok(Runtime::Host),
        (_, Some(dir)) => Ok(Runtime::Bundled(dir)),
        (host, None) => Err(RuntimeError::NoneAvailable { host }),
    }
}

/// The newest `<version>-x64` folder inside `root` (`other/shared/webview2/`) that holds
/// `msedgewebview2.exe`. Folders that are not runtimes are skipped, never an error.
///
/// A folder under an `Edge\Application` path is refused: `WebView2` rejects it for a fixed
/// version runtime.
pub fn find_bundled(root: &Path) -> Option<PathBuf> {
    let mut found: Vec<(Version, PathBuf)> = fs::read_dir(root)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let version = name.strip_suffix(FOLDER_SUFFIX)?.parse().ok()?;
            let dir = entry.path();
            (dir.join(RUNTIME_EXE).is_file() && !is_edge_install(&dir)).then_some((version, dir))
        })
        .collect();
    found.sort_by_key(|(version, _)| *version);
    found.pop().map(|(_, dir)| dir)
}

fn is_edge_install(dir: &Path) -> bool {
    let text = dir
        .to_string_lossy()
        .to_ascii_lowercase()
        .replace('/', "\\");
    text.contains("\\edge\\application\\")
}

/// The newest of the `pv` values found in the registry, ignoring empty and `0.0.0.0` ones.
pub fn newest_installed(values: impl IntoIterator<Item = String>) -> Option<Version> {
    values
        .into_iter()
        .filter_map(|value| value.parse::<Version>().ok())
        .filter(|version| !version.is_zero())
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    type TestResult = Result<(), Box<dyn Error>>;

    fn v(text: &str) -> Result<Version, RuntimeError> {
        text.parse()
    }

    #[test]
    fn runtime_prefers_host_then_bundled_then_error() -> TestResult {
        let floor = v("120")?;
        let bundled = Some(PathBuf::from("x/154.0.1.2-x64"));
        // Both present: the PC's runtime wins.
        assert_eq!(
            select(Some(v("154.0.4258.62")?), bundled.clone(), floor),
            Ok(Runtime::Host)
        );
        // Only the bundled one.
        assert_eq!(
            select(None, bundled.clone(), floor),
            Ok(Runtime::Bundled(PathBuf::from("x/154.0.1.2-x64")))
        );
        // Only the host.
        assert_eq!(select(Some(floor), None, floor), Ok(Runtime::Host));
        // Neither.
        assert_eq!(
            select(None, None, floor),
            Err(RuntimeError::NoneAvailable { host: None })
        );
        Ok(())
    }

    #[test]
    fn runtime_below_floor_falls_back_to_bundled() -> TestResult {
        let floor = v("120.0.0.0")?;
        let old = v("119.0.9999.9")?;
        let bundled = PathBuf::from("b/154.0.1.2-x64");
        assert_eq!(
            select(Some(old), Some(bundled.clone()), floor),
            Ok(Runtime::Bundled(bundled))
        );
        let error = select(Some(old), None, floor);
        assert_eq!(error, Err(RuntimeError::NoneAvailable { host: Some(old) }));
        assert!(
            error
                .err()
                .is_some_and(|e| e.to_string().contains("too old"))
        );
        Ok(())
    }

    #[test]
    fn versions_parse_compare_and_print() -> TestResult {
        assert!(v("154.0.4258.62")? > v("154.0.4258.9")?);
        assert!(v("120")? < v("120.0.0.1")?);
        assert_eq!(v("154.0.4258.62")?.to_string(), "154.0.4258.62");
        assert!(v("1.2.3.4.5").is_err());
        assert!(v("").is_err());
        assert!(v("a.b").is_err());
        assert!(v("0.0.0.0")?.is_zero());
        Ok(())
    }

    #[test]
    fn registry_values_pick_the_newest_real_version() {
        let values = ["", "0.0.0.0", "junk", "152.0.1.1", "154.0.0.5"].map(String::from);
        assert_eq!(newest_installed(values), Some(Version::new(154, 0, 0, 5)));
        assert_eq!(newest_installed(["0.0.0.0".to_owned()]), None);
        assert_eq!(newest_installed([]), None);
    }

    #[test]
    fn bundled_runtime_is_the_newest_folder_with_the_exe() -> TestResult {
        let tmp = genslate_testing::TempTree::new()?
            .file("153.0.1.1-x64/msedgewebview2.exe", "")?
            .file("154.0.2.3-x64/msedgewebview2.exe", "")?
            .file("999.0.0.0-x64/readme.txt", "no exe")?
            .file("notes/msedgewebview2.exe", "not a version")?
            .file("200.0.0.0-arm64/msedgewebview2.exe", "wrong arch")?;
        assert_eq!(find_bundled(tmp.path()), Some(tmp.join("154.0.2.3-x64")));
        assert_eq!(find_bundled(&tmp.join("missing")), None);
        Ok(())
    }

    #[test]
    fn an_edge_install_folder_is_not_a_fixed_runtime() {
        assert!(is_edge_install(Path::new(
            r"C:\Program Files (x86)\Microsoft\Edge\Application\154.0.1.1-x64"
        )));
        assert!(!is_edge_install(Path::new(
            r"E:\SLATECORE\other\shared\webview2\154.0.1.1-x64"
        )));
    }
}
