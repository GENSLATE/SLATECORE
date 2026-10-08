/**
 * `.claude/project/screenshots/<app>/<app>-<polar-night|snow-storm>-<view>.png`: one folder per
 * app, a kebab-case view name, and every view captured in both themes. The owner's workflow rule
 * reuses these for READMEs and websites, so a stray name or a missing theme is drift.
 */
import { lstat } from 'node:fs/promises';
import { join } from 'node:path';

import { type Context, exists, list, problem } from './context';

const ROOT = '.claude/project/screenshots';
const THEMES = ['polar-night', 'snow-storm'] as const;
const KEBAB = '[a-z0-9]+(?:-[a-z0-9]+)*';

export async function checkScreenshots(ctx: Context): Promise<void> {
  if (!(await exists(ctx, ROOT))) return;
  for (const app of await list(ctx, ROOT)) {
    const appRel = `${ROOT}/${app}`;
    if (!(await lstat(join(ctx.root, appRel))).isDirectory()) {
      if (app !== '.gitkeep')
        problem(ctx, appRel, `must live in ${ROOT}/<app>/, one folder per app`);
      continue;
    }
    if (!new RegExp(`^${KEBAB}$`).test(app)) {
      problem(ctx, appRel, 'the app folder name must be lowercase kebab-case');
    }
    const named = new RegExp(
      `^${app.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}-(${THEMES.join('|')})-(${KEBAB})\\.png$`,
    );
    const seen = new Set<string>(); // "<theme>/<view>"
    for (const file of await list(ctx, appRel)) {
      const match = named.exec(file);
      if (match !== null) seen.add(`${match[1]}/${match[2]}`);
      else if (/\.(?:png|jpe?g|webp|gif)$/i.test(file)) {
        problem(
          ctx,
          `${appRel}/${file}`,
          `name must be ${app}-<polar-night|snow-storm>-<view>.png`,
        );
      }
    }
    for (const key of seen) {
      const [theme = '', view = ''] = key.split('/');
      for (const other of THEMES.filter((candidate) => candidate !== theme)) {
        if (!seen.has(`${other}/${view}`)) {
          const file = `${appRel}/${app}-${other}-${view}.png`;
          problem(
            ctx,
            file,
            'missing; every view is captured in both themes (Polar Night and Snow Storm)',
          );
        }
      }
    }
  }
}
