# genslate-testing

Shared test helpers for the SLATECORE crates. Add it as a dev-dependency only:

```toml
[dev-dependencies]
genslate-testing.workspace = true
```

Everything returns `std::io::Result`, so tests can use `?` (the workspace denies `unwrap` and
`expect`).

| Helper | What it does |
|---|---|
| `TempTree` | A temporary folder with a fluent builder (`.file(path, text)`, `.dir(path)`), `join`, `write` and `read`. Deleted when dropped. |
| `fake_suite(tmp)` | Builds `tmp/SLATECORE`, an empty portable install folder (`programs/`, `other/`, `storage/` with the shared profile folders and `vault/`) and returns its path. |
| `fake_repo(tmp)` | Builds `tmp/repo`, a checkout with `turbo.json`, `Cargo.toml` and the launcher mock-up (`programs/desktop/launcher/installDir` plus `programs/desktop/launcher/other`), and returns its path. |
| `REPO_MARKERS` | The two files that mark a checkout. `genslate-paths` has a test that keeps its own list equal to this one. |
| `NoTraceGuard` | Snapshots folder listings and fails a test if they changed. |

`fake_suite` and `fake_repo` build inside the folder you pass, so one test can build several
trees under different roots and compare them (this is how the "drive letter changed" tests work).
They leave `other/<app>/` empty on purpose: creating it is the job of the code under test.

## NoTraceGuard

SLATECORE LAUNCHER must not write outside its install folder. `NoTraceGuard::new()` snapshots
`%TEMP%`, `%APPDATA%` and `%LOCALAPPDATA%` (the ones that are set); `NoTraceGuard::watching(dirs)`
snapshots exactly the folders you give it, which is what unit tests use (empty stand-in folders
next to a temporary install).

```rust
let guard = genslate_testing::NoTraceGuard::new()?;
// ... start the launcher's setup code ...
guard.assert_unchanged(); // panics and lists every added or removed entry
```

* The listing is one level deep by default, which sees a new `%APPDATA%\<identifier>` folder or a
  stray file in `%TEMP%` and stays cheap on a busy PC. `.depth(n)` lists deeper.
* A folder that does not exist yet is watched for appearing.
* Entries named `genslate-test-*` (the folders `TempTree` creates in the system temp folder) are
  ignored, and `.ignoring("prefix")` adds more.
* `changes()` returns the differences as `+ added` and `- removed` lines instead of panicking.
* This is the first of three tiers: a cheap guard inside `cargo test`. The packaged exe is checked
  on Windows by a script that also diffs `HKCU\Software`, and a nightly Process Monitor trace.

## Tests

```text
cargo test -p genslate-testing
```

The library has `doctest = false` in `Cargo.toml`: `cargo test --workspace` links doctests with
every app's build-output folder on the search path, and tauri-build puts a stub `msvcrt.lib` there
(static CRT), so doctests cannot link on Windows. The examples are documentation only; the modules
have unit tests.
