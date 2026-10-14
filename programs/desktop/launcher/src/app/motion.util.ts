/**
 * Motion timing that JavaScript has to wait for, read from the design tokens so it never drifts
 * from the CSS animation it waits on.
 */

/** Used only when `--gs-duration-slow` cannot be read (no stylesheet, as in unit tests). */
export const COLLAPSE_FALLBACK_MS = 360;

/** Milliseconds of a CSS time value (`360ms`, `0.36s`), or `null` if it is not one. */
export function durationMs(raw: string): number | null {
  const match = /^(\d+(?:\.\d+)?|\.\d+)(ms|s)$/.exec(raw.trim());
  if (match === null) return null;
  const value = Number(match[1]);
  return match[2] === 's' ? value * 1000 : value;
}

/**
 * How long the frame takes to shrink back after a tool closes (`--gs-duration-slow`): the
 * shell's hit area and the tool's DOM wait that long. Zero under reduced motion.
 */
export function collapseDelayMs(reducedMotion: boolean): number {
  if (reducedMotion) return 0;
  const token = getComputedStyle(document.documentElement).getPropertyValue('--gs-duration-slow');
  return durationMs(token) ?? COLLAPSE_FALLBACK_MS;
}
