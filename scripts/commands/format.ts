/**
 * `bun run format [turbo args...]`: biome `--write` and rustfmt over the whole repo (both tasks
 * are uncached, so they always run).
 */
import { runTurbo } from './turbo.util';

process.exit(await runTurbo(['run', '//#biome:fix', '//#rust:fmt:fix', ...Bun.argv.slice(2)]));
