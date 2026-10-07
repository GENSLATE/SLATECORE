/**
 * `bun run new-app <name> [--title ...] [--description ...] [--identifier ...] [--port ...]`:
 * scaffold a Tauri desktop app into `programs/desktop/<name>` and its Rust core crate into
 * `crates/<name>-core` from `scripts/templates`, register the crates in the Cargo workspace,
 * run `bun install` and generate the bundle icons.
 */
import { cp, rm } from 'node:fs/promises';
import { join, relative } from 'node:path';

import { defineCommand, fail } from '../lib/args';
import { log } from '../lib/log';
import { scaffoldApp } from '../lib/new-app';
import { fromRoot, ROOT } from '../lib/paths';
import { run, runOrThrow } from '../lib/run';

await defineCommand({
  name: 'new-app',
  summary: 'Scaffold a SLATECORE Tauri desktop app in programs/desktop/<name> from the templates.',
  usage: '<name> [--title <Name>] [--description <text>] [--identifier <id>] [--port <port>]',
  options: {
    title: {
      type: 'string',
      description: 'Display name without the suite prefix. Default: the title-cased <name>.',
    },
    description: {
      type: 'string',
      description: 'One line, no trailing period.',
    },
    identifier: {
      type: 'string',
      description: 'Bundle id. Default: xyz.genslate.slatecore.<name>.',
    },
    port: {
      type: 'string',
      description: 'Vite port (HMR = port + 1). Default: the next free pair from 1440.',
    },
    'no-install': {
      type: 'boolean',
      description: 'Skip `bun install` (and so the bundle icons, which need the Tauri CLI).',
    },
  },
  details: `
Example
  bun run new-app notes    # programs/desktop/notes + crates/notes-core, window "SLATECORE Notes"`,
  async run({ values, positionals }) {
    const name = positionals[0] ?? fail('missing <name>');
    const result = await scaffoldApp(ROOT, fromRoot('scripts/templates'), {
      name,
      ...(values.title === undefined ? {} : { title: values.title }),
      ...(values.description === undefined ? {} : { description: values.description }),
      ...(values.identifier === undefined ? {} : { identifier: values.identifier }),
      ...(values.port === undefined ? {} : { port: Number(values.port) }),
    });
    log.title(
      `Created ${String(result.vars['title'])} in ${relative(ROOT, result.appDir)} (port ${String(result.vars['port'])})`,
    );
    log.info(`${result.written.length} files written; crates/${name}-core added to the workspace`);
    log.warn(`placeholder icon in ${relative(ROOT, result.iconPath)}: design the real glyph there`);

    if (!values['no-install']) await runOrThrow(['bun', 'install']);
    await bundleIcons(result.appDir, result.iconPath, values['no-install'] === true);
    log.success(`${String(result.vars['title'])} is ready`);
    log.info(`next: bun run dev ${name} --web`);
  },
});

/**
 * Generates `src-tauri/icons` from the app's SVG with the Tauri CLI (desktop sizes only), or
 * copies the launcher's icons when the CLI cannot run.
 */
async function bundleIcons(dir: string, svg: string, skip: boolean): Promise<void> {
  const out = join(dir, 'src-tauri/icons');
  // `bun run tauri` uses the app's own CLI (`bun x` could fetch an unrelated package).
  const generated =
    !skip && (await run(['bun', 'run', 'tauri', 'icon', svg, '-o', out], { cwd: dir })) === 0;
  if (generated) {
    // The CLI also writes mobile icons; SLATECORE apps are Windows desktop apps.
    await rm(join(out, 'android'), { recursive: true, force: true });
    await rm(join(out, 'ios'), { recursive: true, force: true });
    log.info(`bundle icons generated from ${relative(ROOT, svg)}`);
    return;
  }
  const fallback = fromRoot('programs/desktop/launcher/src-tauri/icons');
  if (await Bun.file(join(fallback, 'icon.ico')).exists()) {
    await cp(fallback, out, { recursive: true });
    log.warn(
      `icons copied from the launcher; regenerate with \`bun run tauri icon src/assets/app-icon.svg\` in ${relative(ROOT, dir)}`,
    );
  } else {
    log.warn(
      `no bundle icons yet: run \`bun run tauri icon src/assets/app-icon.svg\` in ${relative(ROOT, dir)}`,
    );
  }
}
