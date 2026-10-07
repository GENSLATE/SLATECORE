import { turboCommand } from '../lib/turbo';

/** Runs the repo-pinned turbo through bun and returns its exit code. */
export async function runTurbo(args: readonly string[]): Promise<number> {
  const { argv, env } = turboCommand(args);
  const child = Bun.spawn([...argv], {
    stdio: ['inherit', 'inherit', 'inherit'],
    env: { ...process.env, ...env },
  });
  return await child.exited;
}
