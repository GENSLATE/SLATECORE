import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { basename, join, relative } from 'node:path';
import {
  buildCss,
  deadCandidates,
  listFiles,
  srcRoot,
  stylesRoot,
} from '../../support/tailwind.harness';

const rel = (file: string) => relative(srcRoot, file).replaceAll('\\', '/');

const styleSheets = listFiles(stylesRoot).filter((file) => file.endsWith('.css'));
const variantFiles = listFiles(join(srcRoot, 'components')).filter((file) =>
  file.endsWith('.variants.ts'),
);
const recipeFiles = listFiles(join(srcRoot, 'recipes')).filter((file) =>
  file.endsWith('.recipe.ts'),
);
/** Every file that carries styling: the CSS layer, component variants and shared recipes. */
const styleSources = [...styleSheets, ...variantFiles, ...recipeFiles];

/**
 * The one allowed gradient: ColorSwatch's transparency checkerboard. It is not a surface
 * treatment, it is the standard pattern that shows through translucent colours.
 */
const swatchCheckerboard =
  'bg-[conic-gradient(var(--gs-color-fill-pressed)_25%,transparent_0_50%,var(--gs-color-fill-pressed)_0_75%,transparent_0)]';

/** Collects every class token a tailwind-variants component can emit. */
function collectClasses(value: unknown, into: Set<string>): void {
  if (typeof value === 'string') {
    for (const token of value.split(/\s+/)) if (token !== '') into.add(token);
  } else if (Array.isArray(value)) {
    for (const item of value) collectClasses(item, into);
  } else if (value != null && typeof value === 'object') {
    for (const item of Object.values(value)) collectClasses(item, into);
  }
}

interface TvLike {
  base?: unknown;
  slots?: unknown;
  variants?: unknown;
  compoundVariants?: { class?: unknown; className?: unknown }[];
  compoundSlots?: { class?: unknown; className?: unknown }[];
}

async function classesOf(file: string): Promise<Set<string>> {
  const classes = new Set<string>();
  const mod = (await import(file)) as Record<string, unknown>;
  for (const exported of Object.values(mod)) {
    if (typeof exported !== 'function') continue;
    const component = exported as unknown as TvLike;
    collectClasses(component.base, classes);
    collectClasses(component.slots, classes);
    collectClasses(component.variants, classes);
    for (const compound of [
      ...(component.compoundVariants ?? []),
      ...(component.compoundSlots ?? []),
    ]) {
      collectClasses(compound.class, classes);
      collectClasses(compound.className, classes);
    }
  }
  return classes;
}

/** The utility part of a class: variants (`hover:`, `data-[x=y]:`) and `!` stripped. */
function utilityOf(candidate: string): string {
  let depth = 0;
  let start = 0;
  for (let index = 0; index < candidate.length; index += 1) {
    const char = candidate[index];
    if (char === '[' || char === '(') depth += 1;
    else if (char === ']' || char === ')') depth -= 1;
    else if (char === ':' && depth === 0) start = index + 1;
  }
  return candidate.slice(start).replace(/^!|!$/g, '');
}

/** Returns the `{ … }` body that starts at `open` (the index of an opening brace). */
function blockAt(css: string, open: number): string {
  let depth = 0;
  for (let index = open; index < css.length; index += 1) {
    if (css[index] === '{') depth += 1;
    else if (css[index] === '}') {
      depth -= 1;
      if (depth === 0) return css.slice(open + 1, index);
    }
  }
  return css.slice(open + 1);
}

function blocksMatching(css: string, header: RegExp): string[] {
  const blocks: string[] = [];
  const pattern = new RegExp(header.source, 'g');
  for (let match = pattern.exec(css); match != null; match = pattern.exec(css)) {
    blocks.push(blockAt(css, css.indexOf('{', match.index)));
  }
  return blocks;
}

describe('flat Nord style rules', () => {
  test('no_gradients_bevels_or_inset_shadows_in_styles', () => {
    const forbidden: [string, RegExp][] = [
      ['gradient', /gradient/i],
      ['bevel', /bevel/i],
      ['inset shadow', /shadow-inset|inset-shadow|shadow-\[inset|box-shadow:[^;{}]*\binset\b/],
      ['inset ring', /inset-ring/],
      ['focus halo', /focus-halo/],
    ];
    const offenders: string[] = [];
    for (const file of styleSources) {
      let text = readFileSync(file, 'utf8');
      if (basename(file) === 'color-swatch.variants.ts')
        text = text.replace(swatchCheckerboard, '');
      for (const [label, pattern] of forbidden) {
        if (pattern.test(text)) offenders.push(`${rel(file)}: ${label}`);
      }
    }
    expect(offenders).toEqual([]);
  });

  test('only_floating_surfaces_use_the_popover_shadow', async () => {
    // Menus, popovers, selects (popup-surface), dialogs, toasts and tooltips float; nothing else.
    const floating = new Set([
      'popup-surface.recipe.ts',
      'dialog.variants.ts',
      'toast.variants.ts',
      'tooltip.variants.ts',
    ]);
    const offenders: string[] = [];
    for (const file of [...variantFiles, ...recipeFiles]) {
      for (const candidate of await classesOf(file)) {
        const utility = utilityOf(candidate);
        const isShadow = /^-?(shadow|ring|inset-ring|drop-shadow)(-|$)/.test(utility);
        if (!isShadow || utility === 'shadow-none') continue;
        if (utility === 'shadow-popover' && floating.has(basename(file))) continue;
        offenders.push(`${rel(file)}: ${candidate}`);
      }
    }
    for (const file of styleSheets) {
      const text = readFileSync(file, 'utf8');
      if (/box-shadow|backdrop-filter/.test(text)) offenders.push(`${rel(file)}: box-shadow`);
    }
    expect(offenders).toEqual([]);
  });

  test('no_translucent_glass_or_blur_surfaces', () => {
    const offenders = styleSources.filter((file) =>
      /surface-glass|backdrop-blur|backdrop-filter|backdrop-saturate/.test(
        readFileSync(file, 'utf8'),
      ),
    );
    expect(offenders.map(rel)).toEqual([]);
  });

  test('no_glass_option_in_the_component_api', () => {
    // Floating surfaces are always solid: no `glass` prop, variant or doc anywhere in the source.
    const offenders = listFiles(srcRoot).filter(
      (file) => /\.(ts|tsx)$/.test(file) && /\bglass\b/i.test(readFileSync(file, 'utf8')),
    );
    expect(offenders.map(rel)).toEqual([]);
  });

  test('no_raw_hex_or_dark_variant_outside_tokens', () => {
    const offenders: string[] = [];
    for (const file of listFiles(srcRoot)) {
      if (!/\.(ts|tsx|css)$/.test(file) || file.endsWith('.generated.ts')) continue;
      const text = readFileSync(file, 'utf8');
      if (/#[0-9a-fA-F]{3,8}\b/.test(text)) offenders.push(`${rel(file)}: raw hex`);
      if (/\b(rgba?|hsla?|oklch|oklab|lab|lch)\(/.test(text))
        offenders.push(`${rel(file)}: raw colour`);
      if (/(^|[\s'"`])dark:/m.test(text)) offenders.push(`${rel(file)}: dark: variant`);
    }
    expect(offenders).toEqual([]);
  });

  test('every_variants_file_defines_focus_visible', () => {
    // Display-only or container styles. Their focusable parts are other components (Button,
    // IconButton, TextField) or list rows that show the list-item highlight instead of DOM focus.
    const nonInteractive = new Set([
      'toolbar.variants.ts',
      'avatar.variants.ts',
      'code-block.variants.ts',
      'color-swatch.variants.ts',
      'icon.variants.ts',
      'kbd.variants.ts',
      'badge.variants.ts',
      'banner.variants.ts',
      'empty-state.variants.ts',
      'feature-teaser.variants.ts',
      'progress-bar.variants.ts',
      'skeleton.variants.ts',
      'spinner.variants.ts',
      'checkbox-group.variants.ts',
      'field.variants.ts',
      'search-field.variants.ts',
      'textarea.variants.ts',
      'panel.variants.ts',
      'separator.variants.ts',
      'alert-dialog.variants.ts',
      'command-palette.variants.ts',
      'context-menu.variants.ts',
      'menu.variants.ts',
      'tooltip.variants.ts',
      'window-context-menu.variants.ts',
    ]);
    const names = variantFiles.map((file) => basename(file));
    expect([...nonInteractive].filter((name) => !names.includes(name))).toEqual([]);

    const focusable = [...variantFiles, join(srcRoot, 'recipes', 'field.recipe.ts')].filter(
      (file) => !nonInteractive.has(basename(file)),
    );
    const missing = focusable.filter(
      (file) => !/focus-ring|focus-visible/.test(readFileSync(file, 'utf8')),
    );
    expect(missing.map(rel)).toEqual([]);
  });

  test('focus_rings_are_crisp_frost_outlines', () => {
    const utilities = readFileSync(join(stylesRoot, 'design-system.utilities.css'), 'utf8');
    for (const name of ['focus-ring', 'focus-ring-inset', 'focus-ring-within']) {
      const [body] = blocksMatching(utilities, new RegExp(`@utility ${name}\\s*\\{`));
      expect(body, name).toBeDefined();
      expect(body, name).toContain('var(--gs-color-focus)');
      expect(body, name).toContain('focus-visible');
      expect(body, name).not.toContain('box-shadow');
    }
  });
});

describe('motion layer', () => {
  const motionUtilities = ['motion-fade-up', 'motion-row-in', 'motion-pop', 'motion-pulse-soft'];

  test('reduced_motion_disables_animation_utilities', async () => {
    const css = await buildCss(motionUtilities);
    const reducedBlocks = blocksMatching(css, /@media \(prefers-reduced-motion: reduce\)\s*\{/);
    for (const name of motionUtilities) {
      const rules = blocksMatching(css, new RegExp(`\\.${name}\\s*\\{`));
      expect(rules.length, name).toBeGreaterThan(0);
      const rule = rules.join('\n');
      // It animates with the shared duration and easing tokens...
      const keyframes = /: (gs-[a-z-]+) /.exec(rule)?.[1];
      expect(keyframes, name).toBeDefined();
      expect(css, name).toContain(`@keyframes ${keyframes} {`);
      expect(rule, name).toMatch(/var\(--gs-(duration|ease)-/);
      // ...and stops under reduced motion (nested in the rule or as a top-level media block).
      const nested = blocksMatching(rule, /@media \(prefers-reduced-motion: reduce\)\s*\{/);
      const topLevel = reducedBlocks.flatMap((block) =>
        blocksMatching(block, new RegExp(`\\.${name}\\s*\\{`)),
      );
      const disabled = [...nested, ...topLevel].some((block) =>
        block.replace(/\s+/g, ' ').includes('animation: none'),
      );
      expect(disabled, `${name} under reduced motion`).toBe(true);
    }
  });

  test('row_in_staggers_by_the_stagger_index', async () => {
    const css = await buildCss(['motion-row-in']);
    const rule = blocksMatching(css, /\.motion-row-in\s*\{/).join('\n');
    expect(rule).toMatch(/animation-delay:[^;]*var\(--stagger/);
  });

  test('every_variant_and_recipe_class_generates_css', async () => {
    const candidates = new Set<string>();
    for (const file of [...variantFiles, ...recipeFiles]) {
      for (const candidate of await classesOf(file)) candidates.add(candidate);
    }
    // Group markers (`group/item`) only name a scope for `group-*` variants.
    const utilities = [...candidates].filter((c) => !/^(group|peer)(\/[\w-]+)?$/.test(c));
    expect(utilities.length).toBeGreaterThan(500);
    expect(await deadCandidates(utilities)).toEqual([]);
  });
});
