/** Toolchain pins and the checks `bun run setup` runs against them (pure, so they are testable). */

/** `bun@1.4.2` → `1.4.2` from the root `package.json` `packageManager` field. */
export function readBunPin(packageJson: string): string {
  const parsed: unknown = JSON.parse(packageJson);
  const field =
    typeof parsed === 'object' && parsed !== null && 'packageManager' in parsed
      ? parsed.packageManager
      : undefined;
  const match = typeof field === 'string' ? /^bun@(\d+\.\d+\.\d+)$/.exec(field) : null;
  if (match?.[1] === undefined) {
    throw new Error('package.json "packageManager" must be an exact "bun@x.y.z" pin');
  }
  return match[1];
}

/** `channel = "1.99.0"` from `rust-toolchain.toml`. */
export function readRustPin(toolchainToml: string): string {
  const parsed = Bun.TOML.parse(toolchainToml) as { toolchain?: { channel?: unknown } };
  const channel = parsed.toolchain?.channel;
  if (typeof channel !== 'string' || !/^\d+\.\d+\.\d+$/.test(channel)) {
    throw new Error('rust-toolchain.toml [toolchain].channel must be an exact "x.y.z" version');
  }
  return channel;
}

/** A problem string when the running Bun is not the pinned one. */
export function bunProblem(pin: string, actual: string): string | undefined {
  return pin === actual
    ? undefined
    : `bun ${actual} is running but package.json pins bun@${pin}; install ${pin} (https://bun.sh)`;
}

/** A problem string when `rustc --version` output does not name the pinned version. */
export function rustProblem(pin: string, rustcVersion: string): string | undefined {
  return rustcVersion.trim().split(/\s+/).slice(0, 2).join(' ') === `rustc ${pin}`
    ? undefined
    : `${rustcVersion.trim() || 'rustc'} is not the pinned ${pin} (rust-toolchain.toml); run \`rustup show\` in the repo`;
}
