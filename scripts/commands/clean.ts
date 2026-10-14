/**
 * `bun run clean [--deep] [--keep-target] [--dry-run]`: remove build output and caches. Turbo has
 * no clean command, so this lists and removes the folders itself.
 */
import { relative } from 'node:path';

import { defineCommand } from '../lib/args';
import { planClean, removePaths } from '../lib/clean';
import { log } from '../lib/log';
import { ROOT } from '../lib/paths';

await defineCommand({
  name: 'clean',
  summary:
    'Remove .turbo, dist, out, coverage, release/* (the archive stays) and the Cargo target/ contents.',
  usage: '[--deep] [--keep-target] [--dry-run]',
  options: {
    deep: {
      type: 'boolean',
      short: 'd',
      description: 'Also delete every node_modules (run `bun install` afterwards).',
    },
    'keep-target': { type: 'boolean', description: 'Leave the Cargo target/ folder alone.' },
    'dry-run': { type: 'boolean', short: 'n', description: 'List what would be removed.' },
  },
  details: `
release/.archive/ and the tracked placeholders (release/.gitkeep, target/.gitkeep) are never removed.`,
  async run({ values }) {
    const targets = await planClean(ROOT, {
      deep: values.deep,
      keepTarget: values['keep-target'],
    });
    for (const target of targets) log.info(relative(ROOT, target));
    if (values['dry-run']) {
      log.success(`${targets.length} paths would be removed`);
      return;
    }
    const failed = await removePaths(targets);
    for (const target of failed) log.warn(`could not remove ${relative(ROOT, target)} (in use?)`);
    log.success(`removed ${targets.length - failed.length} paths`);
    if (failed.length > 0) process.exitCode = 1;
  },
});
