//! [`NoTraceGuard`]: proves that code under test left nothing behind on the host PC.
//!
//! SLATECORE LAUNCHER must not write outside its install folder. A test takes a snapshot of the
//! folders Windows (and tools that follow its conventions) would use, runs the code, and asks
//! the guard whether the listings are still the same.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Entries below the watched folder are listed this many levels deep unless told otherwise.
///
/// Level 1 is the folder's own entries. That is enough to see a new `%APPDATA%\<identifier>`
/// or a stray file in `%TEMP%`, and it stays cheap and calm on a busy PC.
const DEFAULT_DEPTH: usize = 1;
/// Folders [`crate::TempTree`] creates in the system temp folder; the test itself makes these.
const DEFAULT_IGNORED_PREFIXES: [&str; 1] = ["genslate-test-"];

/// A snapshot of folder listings that can later be compared with the present.
///
/// ```
/// # fn main() -> std::io::Result<()> {
/// let tree = genslate_testing::TempTree::new()?;
/// let guard = genslate_testing::NoTraceGuard::watching([tree.path()])?;
/// // ... run the code that must not write there ...
/// guard.assert_unchanged();
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct NoTraceGuard {
    depth: usize,
    ignored_prefixes: Vec<String>,
    watched: Vec<Watched>,
}

/// One watched folder and its listing at snapshot time (`None`: it did not exist).
#[derive(Debug, Clone)]
struct Watched {
    dir: PathBuf,
    before: Option<BTreeSet<String>>,
}

impl NoTraceGuard {
    /// Snapshots `%TEMP%`, `%APPDATA%` and `%LOCALAPPDATA%` (the ones that are set).
    pub fn new() -> io::Result<Self> {
        let mut dirs = vec![env::temp_dir()];
        dirs.extend(
            ["APPDATA", "LOCALAPPDATA"]
                .iter()
                .filter_map(env::var_os)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
        );
        Self::watching(dirs)
    }

    /// Snapshots exactly `dirs`. A folder that does not exist yet is watched for appearing.
    pub fn watching<I, P>(dirs: I) -> io::Result<Self>
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        let dirs = dirs.into_iter().map(Into::into).collect();
        Self::snapshot(
            dirs,
            DEFAULT_DEPTH,
            DEFAULT_IGNORED_PREFIXES.map(str::to_owned).to_vec(),
        )
    }

    /// Lists `depth` levels instead of one and takes the snapshot again.
    ///
    /// Call it right after the constructor, before the code under test runs.
    pub fn depth(self, depth: usize) -> io::Result<Self> {
        let dirs = self.dirs();
        Self::snapshot(dirs, depth.max(1), self.ignored_prefixes)
    }

    /// Also ignores entries whose name starts with `prefix` and takes the snapshot again.
    ///
    /// Call it right after the constructor, before the code under test runs.
    pub fn ignoring(mut self, prefix: &str) -> io::Result<Self> {
        self.ignored_prefixes.push(prefix.to_owned());
        let dirs = self.dirs();
        Self::snapshot(dirs, self.depth, self.ignored_prefixes)
    }

    /// The folders being watched.
    pub fn watched(&self) -> Vec<PathBuf> {
        self.dirs()
    }

    /// What differs from the snapshot, one `+ added` or `- removed` line per entry, sorted.
    pub fn changes(&self) -> io::Result<Vec<String>> {
        let mut changes = Vec::new();
        for watched in &self.watched {
            let after = list(&watched.dir, self.depth, &self.ignored_prefixes)?;
            let shown = watched.dir.display();
            match (&watched.before, &after) {
                (None, None) => {}
                (None, Some(entries)) => {
                    changes.push(format!("+ {shown} (the folder itself)"));
                    changes.extend(entries.iter().map(|entry| format!("+ {shown}/{entry}")));
                }
                (Some(entries), None) => {
                    changes.push(format!("- {shown} (the folder itself)"));
                    changes.extend(entries.iter().map(|entry| format!("- {shown}/{entry}")));
                }
                (Some(before), Some(after)) => {
                    changes.extend(
                        after
                            .difference(before)
                            .map(|entry| format!("+ {shown}/{entry}")),
                    );
                    changes.extend(
                        before
                            .difference(after)
                            .map(|entry| format!("- {shown}/{entry}")),
                    );
                }
            }
        }
        Ok(changes)
    }

    /// Panics, listing every difference, if anything changed since the snapshot.
    ///
    /// # Panics
    ///
    /// When a watched folder differs from its snapshot, or can no longer be read.
    pub fn assert_unchanged(&self) {
        match self.changes() {
            Ok(changes) => assert!(
                changes.is_empty(),
                "the code under test left a trace outside its install folder. Changes:\n{}",
                changes.join("\n")
            ),
            Err(error) => panic!("could not re-read the watched folders: {error}"),
        }
    }

    fn dirs(&self) -> Vec<PathBuf> {
        self.watched.iter().map(|w| w.dir.clone()).collect()
    }

    fn snapshot(
        dirs: Vec<PathBuf>,
        depth: usize,
        ignored_prefixes: Vec<String>,
    ) -> io::Result<Self> {
        let watched = dirs
            .into_iter()
            .map(|dir| {
                let before = list(&dir, depth, &ignored_prefixes)?;
                Ok(Watched { dir, before })
            })
            .collect::<io::Result<_>>()?;
        Ok(Self {
            depth,
            ignored_prefixes,
            watched,
        })
    }
}

/// The relative listing of `dir` (`None` when it is not a folder); folders end in `/`.
fn list(dir: &Path, depth: usize, ignored: &[String]) -> io::Result<Option<BTreeSet<String>>> {
    match fs::metadata(dir) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    let mut entries = BTreeSet::new();
    // The top level must be readable; deeper levels are best effort (folders come and go).
    for entry in fs::read_dir(dir)?.flatten() {
        visit(dir, entry, depth, ignored, &mut entries);
    }
    Ok(Some(entries))
}

fn visit(
    root: &Path,
    entry: fs::DirEntry,
    depth: usize,
    ignored: &[String],
    out: &mut BTreeSet<String>,
) {
    let name = entry.file_name().to_string_lossy().into_owned();
    if ignored
        .iter()
        .any(|prefix| name.starts_with(prefix.as_str()))
    {
        return;
    }
    // `DirEntry::file_type` does not follow symlinks, so links never cause loops.
    let Ok(file_type) = entry.file_type() else {
        return;
    };
    let path = entry.path();
    let relative = path.strip_prefix(root).unwrap_or(&path);
    let mut text = relative.to_string_lossy().replace('\\', "/");
    if file_type.is_dir() {
        text.push('/');
    }
    out.insert(text);
    if file_type.is_dir()
        && depth > 1
        && let Ok(children) = fs::read_dir(&path)
    {
        for child in children.flatten() {
            visit(root, child, depth - 1, ignored, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TempTree;
    use std::fs;

    #[test]
    fn unchanged_folders_report_nothing() -> io::Result<()> {
        let tree = TempTree::new()?.file("keep.txt", "x")?;
        let guard = NoTraceGuard::watching([tree.path()])?;
        assert_eq!(guard.changes()?, Vec::<String>::new());
        guard.assert_unchanged();
        Ok(())
    }

    #[test]
    fn a_new_file_is_reported_with_its_folder() -> io::Result<()> {
        let tree = TempTree::new()?;
        let guard = NoTraceGuard::watching([tree.path()])?;
        tree.write("new.txt", "x")?;
        let changes = guard.changes()?;
        assert_eq!(changes.len(), 1, "{changes:?}");
        assert!(changes[0].starts_with('+'), "{changes:?}");
        assert!(changes[0].contains("new.txt"), "{changes:?}");
        Ok(())
    }

    #[test]
    fn a_removed_file_is_reported() -> io::Result<()> {
        let tree = TempTree::new()?.file("old.txt", "x")?;
        let guard = NoTraceGuard::watching([tree.path()])?;
        fs::remove_file(tree.join("old.txt"))?;
        let changes = guard.changes()?;
        assert_eq!(changes.len(), 1, "{changes:?}");
        assert!(changes[0].starts_with('-'), "{changes:?}");
        assert!(changes[0].contains("old.txt"), "{changes:?}");
        Ok(())
    }

    #[test]
    fn a_folder_created_after_the_snapshot_is_reported() -> io::Result<()> {
        let tree = TempTree::new()?;
        let appdata = tree.join("AppData");
        let guard = NoTraceGuard::watching([appdata.as_path()])?;
        guard.assert_unchanged();
        fs::create_dir_all(appdata.join("xyz.genslate"))?;
        let changes = guard.changes()?;
        assert!(
            changes.iter().any(|line| line.contains("AppData")),
            "{changes:?}"
        );
        Ok(())
    }

    #[test]
    fn a_folder_that_disappears_is_reported() -> io::Result<()> {
        let tree = TempTree::new()?.dir("Local")?;
        let guard = NoTraceGuard::watching([tree.join("Local")])?;
        fs::remove_dir(tree.join("Local"))?;
        let changes = guard.changes()?;
        assert_eq!(changes.len(), 1, "{changes:?}");
        assert!(changes[0].starts_with('-'), "{changes:?}");
        assert!(changes[0].contains("the folder itself"), "{changes:?}");
        Ok(())
    }

    #[test]
    fn default_depth_lists_only_the_top_level() -> io::Result<()> {
        let tree = TempTree::new()?.dir("app")?;
        let guard = NoTraceGuard::watching([tree.path()])?;
        tree.write("app/deep.txt", "x")?;
        assert_eq!(guard.changes()?, Vec::<String>::new());
        Ok(())
    }

    #[test]
    fn deeper_snapshots_see_nested_changes() -> io::Result<()> {
        let tree = TempTree::new()?.dir("app")?;
        let guard = NoTraceGuard::watching([tree.path()])?.depth(2)?;
        tree.write("app/deep.txt", "x")?;
        let changes = guard.changes()?;
        assert_eq!(changes.len(), 1, "{changes:?}");
        assert!(changes[0].contains("app/deep.txt"), "{changes:?}");
        Ok(())
    }

    #[test]
    fn test_temp_trees_are_ignored_by_default() -> io::Result<()> {
        let tree = TempTree::new()?;
        let guard = NoTraceGuard::watching([tree.path()])?;
        tree.write("genslate-test-abc/file.txt", "x")?;
        assert_eq!(guard.changes()?, Vec::<String>::new());
        Ok(())
    }

    #[test]
    fn extra_prefixes_can_be_ignored() -> io::Result<()> {
        let tree = TempTree::new()?;
        let guard = NoTraceGuard::watching([tree.path()])?.ignoring("noise-")?;
        tree.write("noise-1.tmp", "x")?;
        assert_eq!(guard.changes()?, Vec::<String>::new());
        tree.write("signal.tmp", "x")?;
        assert_eq!(guard.changes()?.len(), 1);
        Ok(())
    }

    #[test]
    fn several_folders_are_watched_together() -> io::Result<()> {
        let tree = TempTree::new()?.dir("temp")?.dir("appdata")?;
        let guard = NoTraceGuard::watching([tree.join("temp"), tree.join("appdata")])?;
        tree.write("appdata/leak.bin", "x")?;
        let changes = guard.changes()?;
        assert_eq!(changes.len(), 1, "{changes:?}");
        Ok(())
    }

    #[test]
    #[should_panic(expected = "left a trace")]
    fn assert_unchanged_panics_and_names_the_change() {
        let setup = || -> io::Result<(TempTree, NoTraceGuard)> {
            let tree = TempTree::new()?;
            let guard = NoTraceGuard::watching([tree.path()])?;
            tree.write("leak.txt", "x")?;
            Ok((tree, guard))
        };
        let Ok((_tree, guard)) = setup() else {
            panic!("test setup failed");
        };
        guard.assert_unchanged();
    }

    #[test]
    fn new_watches_the_os_temp_folder() -> io::Result<()> {
        let guard = NoTraceGuard::new()?;
        assert!(
            guard.watched().iter().any(|dir| dir == &env::temp_dir()),
            "{:?}",
            guard.watched()
        );
        Ok(())
    }
}
