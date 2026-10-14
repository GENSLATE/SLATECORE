/**
 * `bun run check [--ts] [--rust] [turbo args...]`: every gate CI runs, in one turbo invocation
 * (`--continue` reports every failure in one run), after a lockfile drift check, followed by the
 * repo-level TypeScript check (scripts and tooling) and the agent-folder drift check.
 *
 * `--ts` runs only the TypeScript and repo-wide gates, `--rust` only the Rust ones. Every other
 * argument goes to turbo (`bun run check --force`).
 */
import { checkAgents } from '../lib/agents-check';
import { log } from '../lib/log';
import { ROOT } from '../lib/paths';
import { run } from '../lib/run';
import { runTurbo } from './turbo.util';

const TS_TASKS = ['typecheck', 'check', '//#biome', '//#spell', '//#knip'] as const;
const RUST_TASKS = ['//#rust:fmt', '//#rust:lint', '//#rust:machete', '//#rust:deny'] as const;

const argv = Bun.argv.slice(2);
const onlyTs = argv.includes('--ts');
const onlyRust = argv.includes('--rust');
const passthrough = argv.filter((arg) => arg !== '--ts' && arg !== '--rust');
const both = onlyTs === onlyRust;
const ts = both || onlyTs;
const rust = both || onlyRust;

let failed = false;

log.title('Lockfile');
if ((await run(['bun', 'install', '--frozen-lockfile', '--dry-run'], { quiet: true })) !== 0) {
  log.error('bun.lock is out of date: run `bun install` and commit the result');
  process.exit(1);
}
log.success('bun.lock is up to date');

const tasks = [...(ts ? TS_TASKS : []), ...(rust ? RUST_TASKS : [])];
if ((await runTurbo(['run', ...tasks, '--continue', ...passthrough])) !== 0) failed = true;

if (ts) {
  log.title('Scripts and tooling types');
  const code = await run(['bun', 'x', '--no-install', 'tsc', '--noEmit', '-p', 'tsconfig.json'], {
    cwd: ROOT,
  });
  if (code !== 0) failed = true;

  log.title('Agent folders');
  const problems = await checkAgents(ROOT);
  for (const problem of problems) log.error(problem);
  if (problems.length > 0) failed = true;
  else log.success('agent folders are consistent');
}

process.exit(failed ? 1 : 0);
