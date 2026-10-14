//! Shared test helpers for SLATECORE crates.
//!
//! Use it as a `[dev-dependencies]` entry. Everything returns [`std::io::Result`] so tests
//! can use `?` (the workspace denies `unwrap`/`expect`).
//!
//! * [`TempTree`]: a temporary folder with a fluent builder for files and folders.
//! * [`fake_suite`] / [`fake_repo`]: the portable installDir layout and a repository checkout
//!   that contains its development mock-up, built under any folder you give them.
//! * [`NoTraceGuard`]: snapshots `%TEMP%`, `%APPDATA%` and `%LOCALAPPDATA%` so a test can prove
//!   that the code under test left nothing behind.
//!
//! ```
//! # fn main() -> std::io::Result<()> {
//! let tree = genslate_testing::TempTree::new()?
//!     .file("config/app.toml", "name = \"demo\"\n")?
//!     .dir("logs")?;
//! assert!(tree.join("config/app.toml").is_file());
//! assert_eq!(tree.read("config/app.toml")?, "name = \"demo\"\n");
//! # Ok(())
//! # }
//! ```
#![forbid(unsafe_code)]

mod no_trace;
mod repo;
mod tree;

pub use no_trace::NoTraceGuard;
pub use repo::{REPO_MARKERS, fake_repo, fake_suite};
pub use tree::TempTree;
