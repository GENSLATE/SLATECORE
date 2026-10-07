/**
 * `bun run tokens [--check] [turbo args...]`: regenerate every design-token output (CSS, Tailwind
 * theme, TS, JSON and `crates/design-tokens`). `--check` fails on drift instead of writing.
 * `generate` is cached: on a hit turbo re-writes the cached files, which also repairs hand-edited
 * generated files.
 */
import { runTurbo } from './turbo.util';

const argv = Bun.argv.slice(2);
const check = argv.includes('--check');
const rest = argv.filter((arg) => arg !== '--check');

process.exit(
  await runTurbo(['run', check ? 'check' : 'generate', '--filter=@genslate/tokens', ...rest]),
);
