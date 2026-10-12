import { describe, expect, test } from 'bun:test';
import {
  CHROME_COLOR_KEYS,
  colorToHex,
  contrastRatio,
  relativeLuminance,
  resolveHex,
  SEMANTIC_COLOR_KEYS,
  type TextPairKey,
  THEMES,
  themeValue,
} from '@genslate/tokens';
import { buttonVariants } from '../../../src/components/actions/button/button.variants';
import { toggleButtonVariants } from '../../../src/components/actions/toggle-button/toggle-button.variants';
import { avatarVariants } from '../../../src/components/display/avatar/avatar.variants';
import { kbdVariants } from '../../../src/components/display/kbd/kbd.variants';
import { badgeVariants } from '../../../src/components/feedback/badge/badge.variants';

/**
 * Contrast measured from the classes a component really emits: the `text-*` and `bg-*` token
 * utilities are looked up in both themes, translucent fills are composited over the surface the
 * component sits on, and the pair has to reach WCAG AA (4.5:1) for non-disabled text.
 */

const COLOR_KEYS = new Set<string>([...SEMANTIC_COLOR_KEYS, ...CHROME_COLOR_KEYS]);
const SURFACES = ['canvas', 'surface-sidebar', 'surface-raised', 'surface-popover'] as const;
const themes = Object.values(THEMES);

/** The token a `prefix-<token>` utility names, ignoring sizes such as `text-xs`. */
function tokenOf(classes: string, prefix: 'text' | 'bg'): TextPairKey | undefined {
  for (const candidate of classes.split(/\s+/)) {
    const name = candidate.startsWith(`${prefix}-`) ? candidate.slice(prefix.length + 1) : '';
    if (COLOR_KEYS.has(name)) return name as TextPairKey;
  }
  return undefined;
}

/** Contrast of `fg` text on `bg` (or on the bare surface) with the fill laid over `layers`. */
function measure(
  theme: (typeof themes)[number],
  fg: TextPairKey,
  bg: TextPairKey | undefined,
  surface: (typeof SURFACES)[number],
  layers: readonly TextPairKey[] = [],
): number {
  let base = resolveHex(theme, surface);
  for (const layer of layers) base = colorToHex(themeValue(theme, layer), base);
  const fill = bg === undefined ? base : colorToHex(themeValue(theme, bg), base);
  return contrastRatio(colorToHex(themeValue(theme, fg), fill), fill);
}

describe('Badge text contrast', () => {
  const tones = ['neutral', 'accent', 'success', 'warning', 'danger', 'info'] as const;
  const variants = ['subtle', 'solid', 'outline'] as const;

  for (const variant of variants) {
    for (const tone of tones) {
      test(`${variant}_${tone}_text_reaches_aa_on_every_surface_in_both_themes`, () => {
        const classes = badgeVariants({ tone, variant }).root();
        const fg = tokenOf(classes, 'text');
        const bg = tokenOf(classes, 'bg');
        expect(fg, `text token in "${classes}"`).toBeDefined();
        if (fg === undefined) return;
        for (const theme of themes) {
          for (const surface of SURFACES) {
            const ratio = measure(theme, fg, bg, surface);
            expect(
              ratio,
              `${theme.id}: ${fg} on ${bg ?? surface} over ${surface}`,
            ).toBeGreaterThanOrEqual(4.5);
          }
        }
      });
    }
  }
});

describe('Kbd text contrast', () => {
  const root = kbdVariants({}).root();
  const key = kbdVariants({}).key();

  test('inline_text_reaches_aa_on_surfaces_and_their_hover_fill', () => {
    const fg = tokenOf(root, 'text');
    expect(fg).toBeDefined();
    if (fg === undefined) return;
    for (const theme of themes) {
      for (const surface of SURFACES) {
        for (const layers of [[], ['fill-hover']] as const) {
          const ratio = measure(theme, fg, undefined, surface, layers);
          expect(
            ratio,
            `${theme.id}: ${fg} over ${surface} ${layers.join('+')}`,
          ).toBeGreaterThanOrEqual(4.5);
        }
      }
    }
  });

  test('keycap_text_reaches_aa_on_its_control_fill', () => {
    const fg = tokenOf(key, 'text');
    const bg = tokenOf(key, 'bg');
    expect(bg).toBe('control');
    if (fg === undefined) return;
    for (const theme of themes) {
      for (const surface of SURFACES) {
        const ratio = measure(theme, fg, bg, surface);
        expect(ratio, `${theme.id}: ${fg} on ${bg} over ${surface}`).toBeGreaterThanOrEqual(4.5);
      }
    }
  });
});

describe('Avatar initials contrast', () => {
  test('initials_reach_aa_on_the_tinted_fallback_in_both_themes', () => {
    const fallback = avatarVariants({}).fallback();
    const fg = tokenOf(fallback, 'text');
    const bg = tokenOf(fallback, 'bg');
    expect(fg).toBeDefined();
    expect(bg).toBe('accent-subtle');
    if (fg === undefined) return;
    for (const theme of themes) {
      for (const surface of SURFACES) {
        const ratio = measure(theme, fg, bg, surface);
        expect(ratio, `${theme.id}: ${fg} on ${bg} over ${surface}`).toBeGreaterThanOrEqual(4.5);
      }
    }
  });
});

describe('Button pressed colours', () => {
  /** The token a state utility such as `active:bg-danger-active` names. */
  const stateToken = (classes: string, state: 'hover' | 'active'): TextPairKey | undefined => {
    const match = new RegExp(`${state}:bg-([a-z-]+)`).exec(classes);
    const name = match?.[1] ?? '';
    return COLOR_KEYS.has(name) ? (name as TextPairKey) : undefined;
  };

  for (const variant of ['primary', 'danger'] as const) {
    test(`${variant}_pressed_is_a_step_beyond_hover_and_keeps_its_text_readable`, () => {
      const classes = buttonVariants({ variant }).root();
      const rest = tokenOf(classes, 'bg');
      const hover = stateToken(classes, 'hover');
      const active = stateToken(classes, 'active');
      const fg = tokenOf(classes, 'text');
      expect([rest, hover, active].every((token) => token !== undefined)).toBe(true);
      if (!rest || !hover || !active || !fg) return;
      expect(new Set([rest, hover, active]).size, 'rest, hover and pressed differ').toBe(3);
      for (const theme of themes) {
        const luminance = (token: TextPairKey) => relativeLuminance(resolveHex(theme, token));
        expect(luminance(active), `${theme.id}: pressed is deeper than hover`).toBeLessThan(
          luminance(hover),
        );
        expect(luminance(active), `${theme.id}: pressed is deeper than rest`).toBeLessThan(
          luminance(rest),
        );
        for (const bg of [rest, hover, active]) {
          const ratio = contrastRatio(resolveHex(theme, fg), resolveHex(theme, bg));
          expect(ratio, `${theme.id}: ${fg} on ${bg}`).toBeGreaterThanOrEqual(4.5);
        }
      }
    });
  }
});

describe('ToggleButton pressed text contrast', () => {
  test('pressed_label_reaches_aa_on_its_accent_tint_over_every_surface', () => {
    const classes = toggleButtonVariants({}).split(/\s+/);
    const pressed = (prefix: string) =>
      classes.find((c) => c.startsWith(`data-pressed:${prefix}-`) && !c.includes('border'));
    const fg = pressed('text')?.replace('data-pressed:text-', '');
    const bg = pressed('bg')?.replace('data-pressed:bg-', '');
    expect(bg).toBe('accent-subtle');
    expect(COLOR_KEYS.has(fg ?? '')).toBe(true);
    for (const theme of themes) {
      for (const surface of SURFACES) {
        const ratio = measure(theme, fg as TextPairKey, bg as TextPairKey, surface);
        expect(ratio, `${theme.id}: ${fg} on ${bg} over ${surface}`).toBeGreaterThanOrEqual(4.5);
      }
    }
  });
});
