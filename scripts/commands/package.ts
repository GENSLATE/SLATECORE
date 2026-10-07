/**
 * `bun run package`: placeholder. The portable zip packaging (installDir layout, optional
 * WebView2 runtime, `release/.archive`) arrives with the release task and replaces this file.
 */
import { log } from '../lib/log';

log.error(
  'bun run package is not implemented yet: portable zip packaging arrives with the release task.',
);
log.info('Until then `bun run build` builds the launcher without producing a release archive.');
process.exit(1);
