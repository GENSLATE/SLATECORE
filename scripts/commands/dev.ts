/**
 * `bun run dev [app] [--web] [--kit]`: a dev session through turbo, always filtered to one
 * package (an unfiltered `turbo run dev` would start every dev task at once).
 */
import { access } from 'node:fs/promises';

import { defineCommand, fail } from '../lib/args';
import { log } from '../lib/log';
import { fromRoot } from '../lib/paths';
import { runTurbo } from './turbo.util';

const KIT = '@genslate/example';

await defineCommand({
  name: 'dev',
  summary: 'Run the Tauri launcher (or another app) with hot reload, or just its UI in a browser.',
  usage: '[app] [--web] [--kit]',
  options: {
    web: {
      type: 'boolean',
      short: 'w',
      description: 'Vite only, in a browser with the mock backend (no native window).',
    },
    kit: {
      type: 'boolean',
      short: 'k',
      description: 'The Design Kit (programs/webapp/example) on http://localhost:1430.',
    },
  },
  details: `
Examples
  bun run dev                # the Tauri launcher: native window + Vite hot reload
  bun run dev --web          # the launcher UI in a browser (mock backend), http://localhost:1420
  bun run dev --kit          # the Design Kit, http://localhost:1430
  bun run dev notes --web    # an app made with \`bun run new-app notes\``,
  async run({ values, positionals }) {
    const app = positionals[0];
    if (values.kit && (app !== undefined || values.web)) {
      fail('--kit starts the Design Kit on its own; drop the app name and --web');
    }
    if (app !== undefined && !/^[a-z][a-z0-9-]*$/.test(app)) fail(`"${app}" is not an app name`);
    if (app !== undefined) {
      await access(fromRoot('programs/desktop', app, 'package.json')).catch(() =>
        fail(`no app "${app}" in programs/desktop`),
      );
    }

    const filter = values.kit ? KIT : `@genslate/${app ?? 'launcher'}`;
    const task = values.web && !values.kit ? 'dev:web' : 'dev';
    log.title(`${task} · ${filter}`);
    process.exitCode = await runTurbo(['run', task, `--filter=${filter}`]);
  },
});
