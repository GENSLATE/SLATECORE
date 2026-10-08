# genslate-paths

Finds the folders SLATECORE LAUNCHER (by GENSLATE) is allowed to use, and refuses to run
anywhere else.

The launcher is portable. Everything it controls (configs, cache, logs, database, the WebView2
data folder, `TEMP`, the vault's session folder) lives inside its own install folder
(`installDir`), so a USB stick can be plugged into any PC and unplugged without a trace. This
crate is the only place that decides where those folders are. It has **no operating-system
fallback**: it never reads `%APPDATA%`, `%LOCALAPPDATA%` or the user profile, and it has no
dependency that could. If the install folder cannot be found, cannot be written or is on a
network share, it returns a `PathsError` and the caller shows a message and exits.

## Layout

```text
installDir/
├─ programs/   genslate/<app>/, portableapps.com/, portapps.io/
├─ other/      <app>/{configs,cache,database,logs,documents,licenses,resources}
│              shared/webview2/<version>-x64/   (optional bundled WebView2 runtime)
└─ storage/    users/<profile>/{Desktop,Documents,Downloads,Music,Pictures,Videos}
               vault/                            (encrypted files)
```

For the launcher (`app = "launcher"`):

| `AppPaths` | Location |
|---|---|
| `config_dir` | `other/launcher/configs` |
| `cache_dir` | `other/launcher/cache` |
| `database_dir` | `other/launcher/database` |
| `log_dir` | `other/launcher/logs` |
| `webview2_dir` | `other/launcher/cache/webview2` |
| `temp_dir` | `other/launcher/cache/tmp` |
| `vault_session_dir()` | `other/launcher/cache/vault-session` |
| `vault_dir()` | `storage/vault` |
| `profile_dir("shared")` | `storage/users/shared` |
| `bundled_webview2_root()` | `other/shared/webview2` |

`documents_dir()`, `licenses_dir()`, `resources_dir()` and `app_dir()` complete the set.

Names become folder names, so they are checked:

* App names are lowercase kebab-case (letters, digits, `-`; not starting or ending with `-`);
  `shared` is reserved for the folder above, and Windows device names (`con`, `prn`, `aux`, `nul`,
  `com1`-`com9`, `lpt1`-`lpt9`) are refused.
* Profile names must be one plain folder name. `profile_dir(name)` never leaves `storage/users`:
  for anything else (empty, `..`, `../x`, `/etc`, `C:\x`, `a/b`, a device name, a trailing dot or
  space) it answers `storage/users/_invalid`. `try_profile_dir(name)` returns
  `PathsError::InvalidName` instead.

## Modes

There are two, and the first match wins (`detect_layout`):

1. **`Suite`**: the executable is somewhere below `X/programs/` and `X` also has `other/` and
   `storage/`. `X` is the root, whatever its name or drive letter. The environment variable
   `GENSLATE_INSTALL_DIR` forces this mode with the given folder as the root (tests, wrapper
   scripts), but that folder must still have `other/` and `storage/`; otherwise the answer is
   `NotInstalled`. The `programs` folder keeps the letter case it has on disk.
2. **`Dev`**: a debug build running from a SLATECORE checkout (the folder with `turbo.json` and
   `Cargo.toml`). `programs/` and `storage/` come from the mock-up install folder
   `programs/desktop/launcher/installDir`, `other/` from `programs/desktop/launcher/other`.

Anything else is `PathsError::NotInstalled`. A release build never uses the checkout.

## Errors

| Error | Meaning |
|---|---|
| `NotInstalled` | No install folder found (or a forced one lacks `other/` or `storage/`). |
| `NotWritable(path)` | The folder exists but refuses writes (read-only medium, write-protected drive, missing permission). |
| `UncPath(path)` | The install is on a network share (`\\server\share`, `//server/share`, `\\?\UNC\…`). A mapped network drive is seen through its UNC form. |
| `InvalidName(name)` | The app name is not lowercase kebab-case, or a profile name is not one plain folder name. |
| `Io { path, source }` | Any other file system failure. |

The messages are written for a native message box. They name the folder and what to do.

## Using it

```rust
// At startup, before the first window:
let paths = genslate_paths::resolve("launcher")?; // finds the install, creates the folders, proves they are writable
let settings = paths.config_dir.join("settings.toml");
```

`resolve_with` and `detect_layout` take an explicit `Environment` (debug flag, exe path,
overrides) and write nothing, which is how the tests exercise every mode. `Environment::detect`
canonicalises the exe, the forced install folder and the checkout, so a symlink or a mapped network
drive is seen for what it is before the UNC check. `AppPaths::create_dirs` creates the app's
folders and then writes a probe file, because a write-protected drive lets `create_dir_all`
succeed on folders that already exist. Removing the probe is best effort (Defender or the indexer
can hold it open for a moment on Windows); a stale empty `.write-probe` is harmless.

## Never persist these paths

Drive letters change between PCs. Everything this crate returns is absolute and valid for this run
only. State that must survive (favorites, recent apps, window choices) stores paths relative to
`Layout::root`, or just an app id, and joins them to the current root when it is read back.

## Tests

```text
cargo test -p genslate-paths
```

They use `genslate-testing` to build fake install folders and checkouts under temporary folders:
the same tree under two roots resolves identically relative to the root, spaces and Unicode in the
path work, every returned path is inside the root, UNC roots are rejected, and an unwritable
install produces `NotWritable` while stand-ins for `%TEMP%`, `%APPDATA%` and `%LOCALAPPDATA%`
stay untouched. UNC detection looks at the path text, so those tests run on any OS.
