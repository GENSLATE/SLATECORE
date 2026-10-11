import { win32 } from 'node:path';

/**
 * What `bun run package` builds and where everything lives: names, folders and the one build
 * command. Plain constants so the stager, the release step, the command and the tests agree.
 */

/** Product name as README and About write it. */
export const PRODUCT = 'SLATECORE LAUNCHER by GENSLATE';
/** The shipped program (Cargo package `genslate-launcher`, binary `slatecore-launcher`). */
export const EXE_NAME = 'slatecore-launcher.exe';
/** Where `tauri build --no-bundle` leaves it, relative to the repo root. */
export const BUILT_EXE = 'target/release/slatecore-launcher.exe';
/** The launcher app, relative to the repo root. */
export const LAUNCHER_DIR = 'programs/desktop/launcher';
/** The seed install folder and the seed per-app folder inside `LAUNCHER_DIR`. */
export const SEED_INSTALL_DIR = `${LAUNCHER_DIR}/installDir`;
export const SEED_APP_DIR = `${LAUNCHER_DIR}/other/launcher`;
/** The README that goes to the root of the base zip (`{{version}}` is replaced). */
export const README_SOURCE = `${SEED_INSTALL_DIR}/README.md`;

/**
 * The build: Tauri without bundling. No installer mode is ever used, because the launcher is
 * portable and must not install anything. Run from `LAUNCHER_DIR`.
 */
export const BUILD_ARGV = ['bun', 'x', '--no-install', 'tauri', 'build', '--no-bundle'] as const;

/** Where the exe sits inside installDir. */
export const EXE_ZIP_PATH = `programs/genslate/launcher/${EXE_NAME}`;
/** Empty folders cannot be kept by every zip tool, so each gets a zero byte file. */
export const PLACEHOLDER = '.gitkeep';
/** The folders that ship empty, represented by a placeholder file. */
export const PLACEHOLDER_DIRS: readonly string[] = [
  'programs/portableapps.com',
  'programs/portapps.io',
  'storage/users/shared/Desktop',
  'storage/users/shared/Documents',
  'storage/users/shared/Downloads',
  'storage/users/shared/Music',
  'storage/users/shared/Pictures',
  'storage/users/shared/Videos',
  'other/launcher/cache',
  'other/launcher/logs',
  'other/launcher/database',
];
/**
 * Folders that ship empty as an explicit directory entry and NOT as a placeholder: the vault
 * counts every file it did not write as a plain file waiting to be encrypted, so a placeholder
 * would show up as one. The vault creates its folder itself if an extractor drops the entry.
 */
export const DIRECTORY_ONLY_DIRS: readonly string[] = ['storage/vault'];

/** Seeds copied from `SEED_APP_DIR` into `other/launcher/`. */
export const SETTINGS_SEED = 'configs/settings.toml';
/**
 * What ships as `configs/settings.toml`: this text, never the working tree file. In dev mode the
 * launcher writes the developer's own settings into the seed file, and those must not be
 * packaged. Comments only, so every setting is its default.
 */
export const DEFAULT_SETTINGS = [
  '# SLATECORE LAUNCHER settings. A missing key means its default, so an empty file is fine.',
  '# The launcher writes only the keys you change in Settings; edit the file by hand if you like.',
  '',
].join('\n');
/** Documents for maintainers (how to package, how the portability test works): not shipped. */
export const MAINTAINER_DOCS: readonly string[] = ['release.md', 'portability.md'];
/** The GENSLATE license text is the owner's to supply; an empty seed is reported. */
export const GENSLATE_LICENSE = 'other/launcher/licenses/genslate.LICENSE.md';
export const SEED_TREES: readonly string[] = ['documents', 'licenses', 'resources/icons'];
/** The generated metadata of the launcher itself, stamped with the version. */
export const METADATA_ZIP_PATH = 'other/launcher/configs/metadata/launcher.toml';

/** `release/` file names. */
export const RELEASE_PREFIX = 'slatecore-launcher-';
export const baseZipName = (version: string): string => `${RELEASE_PREFIX}${version}-base.zip`;
export const runtimeZipName = (version: string): string =>
  `${RELEASE_PREFIX}${version}-webview2-runtime.zip`;
/** Older builds are moved here. */
export const ARCHIVE_DIR = '.archive';
/** Scratch folder inside `release/` while the new zips are written. */
export const BUILDING_DIR = '.building';

/** The optional WebView2 runtime add-on. */
export const LOCK_FILE = 'scripts/webview2.lock.json';
export const RUNTIME_ZIP_DIR = 'other/shared/webview2';
export const RUNTIME_NOTICE_ZIP_PATH = 'other/launcher/licenses/webview2.NOTICE.md';
/** The runtime is only ever taken from these hosts (and their subdomains). */
export const MICROSOFT_DOMAIN = 'microsoft.com';
/** The program that marks a folder as a WebView2 runtime. */
export const RUNTIME_EXE = 'msedgewebview2.exe';
/** Name of the folder `expand.exe` creates from Microsoft's cab. */
export const RUNTIME_FOLDER =
  /^Microsoft\.WebView2\.FixedVersionRuntime\.(\d+\.\d+\.\d+\.\d+)\.x64$/;

/**
 * A tool in Windows' System32, by absolute path (so Git Bash's `expand` or `reg` are never
 * picked up). There is no guess at `C:\Windows`: without `SystemRoot` this throws.
 */
export function systemExe(
  name: string,
  env: Readonly<Record<string, string | undefined>> = process.env,
): string {
  const root = env['SystemRoot'];
  if (root === undefined || root === '') {
    throw new Error(`SystemRoot is not set, so ${name} cannot be found (this runs on Windows)`);
  }
  return win32.join(root, 'System32', name);
}
