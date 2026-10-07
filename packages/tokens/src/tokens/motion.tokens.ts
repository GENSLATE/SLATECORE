/**
 * Motion: short, calm durations and three curves. Under `prefers-reduced-motion: reduce` every
 * duration resolves to `REDUCED_MOTION_DURATION` and enter distance/scale are neutralised.
 *
 * CSS variables: `--gs-duration-{instant,fast,base,slow}` and `--gs-ease-{standard,emphasized,decelerate}`.
 */

import type { MotionTokens } from '../token.types';

/**
 * Duration every motion token takes under `prefers-reduced-motion: reduce`. Not `0ms`: a zero
 * duration suppresses `transitionend` / `animationend`, which components still wait for.
 */
export const REDUCED_MOTION_DURATION = '0.01ms';

export const MOTION: MotionTokens = {
  duration: { instant: 0, fast: 120, base: 200, slow: 360 },
  easing: {
    /** Default for state changes: quick start, soft landing. */
    standard: { points: [0.2, 0, 0, 1] },
    /** Entrances that should feel deliberate (popovers, dialogs). */
    emphasized: { points: [0.16, 1, 0.3, 1] },
    /** Elements settling into place. */
    decelerate: { points: [0.05, 0.7, 0.1, 1] },
  },
  distance: 4,
  scaleFrom: 0.96,
};
