/**
 * `bun run version <patch|minor|major|x.y.z> [--dry-run]`: bump the version everywhere, then
 * refresh both lockfiles.
 */
import { relative } from 'node:path';

import { defineCommand, fail } from '../lib/args';
import { log } from '../lib/log';
import { fromRoot, ROOT } from '../lib/paths';
import { run } from '../lib/run';
import { planVersionBump, writeVersionEdits } from '../lib/version';

await defineCommand({
  name: 'version',
  summary:
    'Bump the version in every package.json, the Cargo workspace and literal tauri.conf.json versions.',
  usage: '<patch|minor|major|x.y.z> [--dry-run]',
  options: {
    'dry-run': { type: 'boolean', short: 'n', description: 'Print the changes without writing.' },
  },
  details: `
Apps whose tauri.conf.json has \`"version": "../package.json"\` follow their package.json automatically.
After writing, \`cargo update --workspace\` and \`bun install\` move Cargo.lock and bun.lock.`,
  async run({ values, positionals }) {
    const bump = positionals[0] ?? fail('missing <patch|minor|major|x.y.z>');
    const plan = await planVersionBump(ROOT, bump);
    const dryRun = values['dry-run'];
    log.title(`${plan.current} → ${plan.next}${dryRun ? ' (dry run)' : ''}`);
    for (const edit of plan.edits) log.info(relative(ROOT, fromRoot(edit.file)));
    if (!dryRun) {
      await writeVersionEdits(ROOT, plan.edits);
      if ((await run(['cargo', 'update', '--workspace', '--offline'])) !== 0) {
        await run(['cargo', 'update', '--workspace']);
      }
      await run(['bun', 'install']);
    }
    log.success(`version ${plan.next}`);
  },
});
