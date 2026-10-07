import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import {
  emitIndexCss,
  emitPrimitivesCss,
  emitScalesCss,
  emitThemeCss,
} from '../../scripts/emit/css.emitter';
import { checkContrast } from '../../scripts/emit/data.emitter';
import { emitTailwindThemeCss } from '../../scripts/emit/tailwind.emitter';
import {
  CHROME_COLOR_KEYS,
  type ColorValue,
  CURSOR_KEYS,
  CURSORS,
  contrastRatio,
  cursorCss,
  cursorDataUri,
  cursorSvg,
  DERIVED,
  MOTION,
  mix,
  NORD,
  opticalTracking,
  PRIMITIVE_HEX,
  REDUCED_MOTION_DURATION,
  resolveHex,
  SEMANTIC_COLOR_KEYS,
  SHADOW_KEYS,
  shiftLightness,
  THEMES,
  type ThemeDefinition,
} from '../../src/index';

const SRC = join(import.meta.dir, '../../src');
const themeList = Object.values(THEMES);

const OFFICIAL_NORD = {
  'nord-0': '#2E3440',
  'nord-1': '#3B4252',
  'nord-2': '#434C5E',
  'nord-3': '#4C566A',
  'nord-4': '#D8DEE9',
  'nord-5': '#E5E9F0',
  'nord-6': '#ECEFF4',
  'nord-7': '#8FBCBB',
  'nord-8': '#88C0D0',
  'nord-9': '#81A1C1',
  'nord-10': '#5E81AC',
  'nord-11': '#BF616A',
  'nord-12': '#D08770',
  'nord-13': '#EBCB8B',
  'nord-14': '#A3BE8C',
  'nord-15': '#B48EAD',
} as const;

/** Root Nord colour a primitive is derived from (itself for Nord colours). */
function rootOf(ref: string): string {
  const derived = (DERIVED as Record<string, { from: string } | undefined>)[ref];
  return derived ? derived.from : ref;
}

const POLAR_NIGHT = new Set(['nord-0', 'nord-1', 'nord-2', 'nord-3']);
const SNOW_STORM = new Set(['nord-4', 'nord-5', 'nord-6']);
const SURFACE_KEYS = [
  'canvas',
  'surface',
  'surface-sidebar',
  'surface-panel',
  'surface-raised',
  'surface-popover',
  'surface-dialog',
  'surface-sunken',
  'field',
] as const;

function surfaceRef(theme: ThemeDefinition, key: (typeof SURFACE_KEYS)[number]): string {
  const value: ColorValue = theme.color[key];
  if (value === 'transparent') throw new Error(`${theme.id}/${key} is transparent`);
  return value.ref;
}

describe('Nord palette', () => {
  test('nord_primitives_match_official_palette', () => {
    const actual = Object.fromEntries(
      Object.entries(NORD).map(([key, value]) => [key, value.hex.toUpperCase()]),
    );
    expect(actual).toEqual(OFFICIAL_NORD);
  });

  test('derived shades re-derive from their op', () => {
    for (const [key, d] of Object.entries(DERIVED)) {
      const base = NORD[d.from].hex;
      const expected =
        d.op.kind === 'mix'
          ? mix(base, PRIMITIVE_HEX[d.op.with] ?? '', d.op.weight / 100)
          : shiftLightness(base, d.op.kind === 'lighten' ? d.op.deltaL : -d.op.deltaL);
      expect(d.hex, key).toBe(expected);
    }
  });

  test('pins a few derived values', () => {
    expect(DERIVED['nord-4-m0-85'].hex).toBe('#bfc5d0');
    expect(DERIVED['nord-0-d03'].hex).toBe('#272c38');
  });
});

describe('themes', () => {
  test('dark_theme_surfaces_come_from_polar_night_and_light_from_snow_storm', () => {
    for (const key of SURFACE_KEYS) {
      const dark = rootOf(surfaceRef(THEMES['polar-night'], key));
      const light = rootOf(surfaceRef(THEMES['snow-storm'], key));
      expect(POLAR_NIGHT.has(dark), `dark/${key} -> ${dark}`).toBe(true);
      expect(SNOW_STORM.has(light), `light/${key} -> ${light}`).toBe(true);
    }
  });

  test('semantic_colors_reference_primitives_only', () => {
    for (const theme of themeList) {
      for (const [key, value] of Object.entries({ ...theme.color, ...theme.component })) {
        if (value === 'transparent') continue;
        expect(PRIMITIVE_HEX[value.ref], `${theme.id}/${key} -> ${value.ref}`).toBeDefined();
      }
    }
    // No free-standing hex anywhere in the token source except the Nord table itself.
    const hex = /#[0-9a-fA-F]{3,8}\b/g;
    const sources = [
      'themes/nord.polar-night.theme.ts',
      'themes/nord.snow-storm.theme.ts',
      'themes/theme.contrast.ts',
      'tokens/layout.tokens.ts',
      'tokens/motion.tokens.ts',
    ];
    for (const file of sources) {
      expect(readFileSync(join(SRC, file), 'utf8').match(hex) ?? [], file).toEqual([]);
    }
    const colorSource = readFileSync(join(SRC, 'tokens/color.tokens.ts'), 'utf8');
    const literals = [...new Set((colorSource.match(hex) ?? []).map((h) => h.toLowerCase()))];
    expect(literals.sort()).toEqual(
      [...Object.values(OFFICIAL_NORD).map((h) => h.toLowerCase()), '#000000', '#ffffff'].sort(),
    );
  });

  test('derived tints are computed with the mix() helper, never hand-written', () => {
    for (const d of Object.values(DERIVED)) {
      if (d.op.kind !== 'mix') continue;
      const other = PRIMITIVE_HEX[d.op.with];
      expect(other).toBeDefined();
      expect(d.hex).toBe(mix(NORD[d.from].hex, other ?? '', d.op.weight / 100));
    }
  });

  test('define every semantic and chrome key', () => {
    for (const theme of themeList) {
      for (const key of [...SEMANTIC_COLOR_KEYS, ...CHROME_COLOR_KEYS]) {
        expect(resolveHex(theme, key)).toMatch(/^#[0-9a-f]{6}$/);
      }
    }
  });

  test('contrast_report_passes_aa_for_text_tokens', () => {
    const results = checkContrast(themeList);
    expect(results.length).toBeGreaterThan(0);
    expect(results.filter((r) => !r.pass)).toEqual([]);
    // Every text-on-surface pair is held to AA body text (4.5:1) at least.
    const textPairs = results.filter(
      (r) => r.fg === 'fg' || r.fg === 'fg-secondary' || r.fg === 'fg-muted',
    );
    expect(textPairs.length).toBeGreaterThan(0);
    for (const r of textPairs) {
      expect(r.ratio, `${r.theme}: ${r.fg}/${r.bg}`).toBeGreaterThanOrEqual(4.5);
    }
    // The committed report agrees.
    const committed = JSON.parse(
      readFileSync(join(SRC, 'generated/json/contrast-report.json'), 'utf8'),
    ) as { passed: boolean };
    expect(committed.passed).toBe(true);
  });

  test('body text is comfortably readable', () => {
    const dark = THEMES['polar-night'];
    expect(contrastRatio(resolveHex(dark, 'fg'), resolveHex(dark, 'canvas'))).toBeGreaterThan(7);
  });
});

describe('motion', () => {
  test('motion_tokens_present_and_reduced_motion_zeroes_durations', () => {
    expect(Object.keys(MOTION.duration)).toEqual(['instant', 'fast', 'base', 'slow']);
    expect(Object.keys(MOTION.easing)).toEqual(['standard', 'emphasized', 'decelerate']);
    expect(REDUCED_MOTION_DURATION).toBe('0.01ms');

    const css = emitScalesCss();
    const [normal = '', reduced = ''] = css.split('@media (prefers-reduced-motion: reduce)');
    for (const key of ['instant', 'fast', 'base', 'slow'] as const) {
      expect(normal).toContain(`--gs-duration-${key}: ${MOTION.duration[key] || '0'}`);
      expect(reduced).toContain(`--gs-duration-${key}: 0.01ms;`);
    }
    for (const key of ['standard', 'emphasized', 'decelerate']) {
      expect(normal).toMatch(new RegExp(`--gs-ease-${key}: cubic-bezier\\(`));
    }
    expect(reduced).not.toMatch(/--gs-duration-\w+: (?!0\.01ms)/);
  });

  test('tailwind theme exposes the motion tokens', () => {
    const tw = emitTailwindThemeCss();
    for (const key of ['standard', 'emphasized', 'decelerate']) {
      expect(tw).toContain(`--ease-${key}: var(--gs-ease-${key});`);
    }
    for (const key of ['instant', 'fast', 'base', 'slow']) {
      expect(tw).toContain(`--transition-duration-${key}: var(--gs-duration-${key});`);
    }
  });
});

describe('flat style', () => {
  test('flat_theme_has_no_shadow_inset_or_gradient_tokens', () => {
    expect([...SHADOW_KEYS]).toEqual(['popover']);
    for (const theme of themeList) {
      expect(Object.keys(theme.shadow)).toEqual(['popover']);
      for (const layers of Object.values(theme.shadow)) {
        for (const layer of layers) expect('inset' in layer).toBe(false);
      }
    }
    const css = [
      emitPrimitivesCss(),
      emitScalesCss(),
      ...themeList.map((t, i) => emitThemeCss(t, i === 0)),
      emitIndexCss([]),
      emitTailwindThemeCss(),
    ].join('\n');
    const names = [...css.matchAll(/^\s*(--[\w*-]+):/gm)].map((m) => m[1] ?? '');
    const forbidden = names.filter((n) => /(shadow-inset|inset-shadow|bevel|gradient)/.test(n));
    // Only Tailwind's default `--inset-shadow-*` is mentioned, to reset it to `initial`.
    expect(forbidden).toEqual(['--inset-shadow-*']);
    expect(css).toContain('--inset-shadow-*: initial;');
    expect(css).not.toMatch(/(linear|radial|conic)-gradient/);
    expect(css).not.toMatch(/--gs-shadow-[\w-]+: inset /);
    expect(new Set(names.filter((n) => n.startsWith('--gs-shadow-')))).toEqual(
      new Set(['--gs-shadow-popover']),
    );
  });
});

describe('scales', () => {
  test('optical tracking tightens as size grows', () => {
    expect(opticalTracking(11)).toBeGreaterThan(opticalTracking(13));
    expect(opticalTracking(13)).toBeGreaterThan(opticalTracking(24));
  });
});

describe('cursors', () => {
  test('every theme paints every cursor as a themed image with a native fallback', () => {
    for (const theme of themeList) {
      for (const key of CURSOR_KEYS) {
        const css = cursorCss(key, theme);
        expect(css, `${theme.id}/${key}`).toMatch(
          /^url\("data:image\/svg\+xml,[^"]+"\) \d+ \d+, [a-z-]+$/,
        );
        expect(css.endsWith(`, ${CURSORS[key].fallback}`)).toBe(true);
      }
    }
  });

  test('the data URI escapes everything CSS and URLs need', () => {
    const uri = cursorDataUri('default', THEMES['polar-night']);
    expect(uri).not.toMatch(/[<>"#]/);
    expect(decodeURIComponent(uri.replace('data:image/svg+xml,', ''))).toBe(
      cursorSvg('default', THEMES['polar-night']).replaceAll('"', "'"),
    );
  });

  test('glyphs follow the theme colours', () => {
    expect(cursorSvg('default', THEMES['polar-night'])).toContain(NORD['nord-6'].hex);
    expect(cursorSvg('default', THEMES['snow-storm'])).toContain(NORD['nord-0'].hex);
    expect(cursorSvg('interactive', THEMES['polar-night'])).toContain(NORD['nord-8'].hex);
  });

  test('hotspots sit inside the 32px image', () => {
    for (const { hotspot } of Object.values(CURSORS)) {
      for (const n of hotspot) {
        expect(Number.isInteger(n)).toBe(true);
        expect(n).toBeGreaterThanOrEqual(0);
        expect(n).toBeLessThan(32);
      }
    }
  });
});
