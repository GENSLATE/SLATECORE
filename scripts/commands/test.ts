/**
 * `bun run test [turbo args...]`: every package's tests and the Rust workspace tests through
 * turbo (`--continue` reports every failure), then the repo scripts' own tests.
 */
import { run } from '../lib/run';
import { runTurbo } from './turbo.util';

const turboCode = await runTurbo([
  'run',
  'test',
  '//#rust:test',
  '--continue',
  ...Bun.argv.slice(2),
]);
const scriptsCode = await run(['bun', 'test', 'scripts/tests']);

process.exit(turboCode !== 0 ? turboCode : scriptsCode);
