/**
 * `bun run package [--exe <path>] [--no-build] [--webview2 <cab>] [--out <dir>] [--release]`: the portable
 * zips in `release/`. Builds the launcher with `tauri build --no-bundle` (no installer, ever),
 * stages the installDir layout and writes `slatecore-launcher-<version>-base.zip`, plus with
 * `--webview2` the runtime add-on. Older builds move to `release/.archive/`.
 */
import { readFile, stat } from 'node:fs/promises';
import { resolve } from 'node:path';

import { sha256File } from '../lib/archive';
import { defineCommand, fail } from '../lib/args';
import { log } from '../lib/log';
import { fromRoot, ROOT } from '../lib/paths';
import {
  assertLockResolved,
  assertTagMatchesVersion,
  buildRelease,
  licenseGaps,
  looksLikeWindowsExe,
  parseLock,
  readVersion,
  type Webview2Options,
} from '../lib/release';
import { runOrThrow } from '../lib/run';
import { BUILD_ARGV, BUILT_EXE, LAUNCHER_DIR, LOCK_FILE } from '../lib/targets';

const mebibytes = (bytes: number): string => `${(bytes / 1024 / 1024).toFixed(1)} MiB`;

await defineCommand({
  name: 'package',
  summary: 'Build the portable zips in release/ (base, and with --webview2 the runtime add-on).',
  usage: '[--exe <path>] [--no-build] [--webview2 <cab>] [--out <dir>] [--release]',
  options: {
    exe: {
      type: 'string',
      description: 'Package this existing slatecore-launcher.exe instead of building (no build).',
    },
    'no-build': {
      type: 'boolean',
      description: `Do not build; package ${BUILT_EXE} as it is.`,
    },
    webview2: {
      type: 'string',
      description: 'The Fixed Version cab from Microsoft: also writes the runtime add-on zip.',
    },
    out: {
      type: 'string',
      description: 'Write the zips here instead of release/.',
    },
    release: {
      type: 'boolean',
      description:
        'A release build: refuse when the GENSLATE license text is missing (also on when GITHUB_REF_TYPE=tag).',
    },
  },
  details: `
Examples
  bun run package                                   # build, then release/slatecore-launcher-<version>-base.zip
  bun run package --no-build                        # package the exe that is already built
  bun run package --no-build --webview2 <file>.cab  # also the WebView2 runtime add-on

The runtime add-on needs ${LOCK_FILE} filled in (version, url, sha256, size);
it refuses to run while the lock still says UNRESOLVED or the cab does not match it.
The build runs on Windows only; elsewhere use --exe with a build from a Windows machine.`,
  async run({ values }) {
    const exe = values.exe === undefined ? fromRoot(BUILT_EXE) : resolve(values.exe);
    const releaseDir = values.out === undefined ? fromRoot('release') : resolve(values.out);
    const version = await readVersion(ROOT);

    // The tag names the release: a mismatch would publish the wrong version's zips.
    try {
      assertTagMatchesVersion(process.env, version);
    } catch (error) {
      fail(error instanceof Error ? error.message : String(error));
    }

    // A tag build is a release build, as is --release. A missing license text stops it before the
    // long build; any other build only warns.
    const release = values.release || process.env['GITHUB_REF_TYPE'] === 'tag';
    const gaps = await licenseGaps(ROOT);
    for (const gap of gaps) {
      log.warn(
        `WARNING: ${gap} is empty. The GENSLATE license text has to come from the owner before a tagged release.`,
      );
    }
    if (release && gaps.length > 0) {
      fail(`refusing a release build: the license text is missing (${gaps.join(', ')})`);
    }

    // Refuse before any work: the add-on needs a lock a person has verified.
    let webview2: Webview2Options | undefined;
    if (values.webview2 !== undefined) {
      const lock = parseLock(await readFile(fromRoot(LOCK_FILE), 'utf8'));
      assertLockResolved(lock);
      webview2 = { cab: resolve(values.webview2), lock };
    }

    if (values.exe === undefined && !values['no-build']) {
      if (process.platform !== 'win32') {
        fail(
          'the launcher builds on Windows only (x86_64-pc-windows-msvc). Build it on a Windows machine, then run `bun run package --exe <path to slatecore-launcher.exe>` here.',
        );
      }
      log.title(`build · ${LAUNCHER_DIR}`);
      await runOrThrow(BUILD_ARGV, { cwd: fromRoot(LAUNCHER_DIR) });
    }

    const info = await stat(exe).catch(() => undefined);
    if (info === undefined || !info.isFile()) fail(`the launcher exe is missing: ${exe}`);
    if (!(await looksLikeWindowsExe(exe))) {
      // A path given with --exe is the caller's choice; anything else must be a real program.
      if (values.exe === undefined) fail(`${exe} is not a Windows executable (no MZ header)`);
      log.warn(`${exe} is not a Windows executable (no MZ header); packaging it as asked`);
    }

    log.title(`package · ${version}`);
    const result = await buildRelease({
      root: ROOT,
      releaseDir,
      exe,
      version,
      release,
      ...(webview2 === undefined ? {} : { webview2 }),
    });
    for (const old of result.archived) log.info(`archived ${old}`);
    for (const zip of [result.base, result.runtime]) {
      if (zip === undefined) continue;
      const size = (await stat(zip)).size;
      log.success(`${zip} (${mebibytes(size)})`);
      log.info(`sha256 ${await sha256File(zip)}`);
    }
  },
});
