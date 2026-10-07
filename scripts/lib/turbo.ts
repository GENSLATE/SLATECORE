/** Pure description of how the root commands start the repo-pinned turbo. */

export interface TurboCommand {
  readonly argv: readonly string[];
  readonly env: Readonly<Record<string, string>>;
}

/**
 * `bun x --no-install turbo <args>`: the locally installed turbo only (never fetched), array
 * spawn so nothing is shell-quoted, telemetry off. Extra CLI args pass straight through
 * (`bun run check --force` reaches turbo).
 */
export function turboCommand(
  args: readonly string[],
  bun: string = process.execPath,
): TurboCommand {
  return {
    argv: [bun, 'x', '--no-install', 'turbo', ...args],
    env: { TURBO_TELEMETRY_DISABLED: '1' },
  };
}
