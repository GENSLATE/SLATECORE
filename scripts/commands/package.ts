/**
 * `bun run package [--exe <path>] [--no-build] [--webview2 <cab>] [--out <dir>]`: the portable
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
  buildRelease,
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
  usage: '[--exe <path>] [--no-build] [--webview2 <cab>] [--out <dir>]',
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
    const magic = new Uint8Array(await Bun.file(exe).slice(0, 2).arrayBuffer());
    if (magic[0] !== 0x4d || magic[1] !== 0x5a) {
      log.warn(`${exe} is not a Windows executable (no MZ header); packaging it anyway`);
    }

    log.title(`package · ${version}`);
    const result = await buildRelease({
      root: ROOT,
      releaseDir,
      exe,
      version,
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
