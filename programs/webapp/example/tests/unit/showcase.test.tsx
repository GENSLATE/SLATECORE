import { describe, expect, test } from 'bun:test';
import * as designSystem from '@genslate/design-system';
import { DesignSystemProvider, ToastProvider, TooltipProvider } from '@genslate/design-system';
import { act, render } from '@testing-library/react';
import { SHOWCASE_GROUPS, SHOWCASE_SECTIONS } from '../../src/features/showcase/showcase.registry';

/** Exported React components: PascalCase function exports (contexts, constants and hooks are not). */
const componentExports = Object.entries(designSystem)
  .filter(([name, value]) => /^[A-Z][a-z]/.test(name) && typeof value === 'function')
  .map(([name]) => name);

describe('showcase registry', () => {
  test('lists the nine categories in sidebar order with unique page ids', () => {
    expect(SHOWCASE_GROUPS.map((group) => group.id)).toEqual([
      'foundations',
      'window',
      'layout',
      'actions',
      'inputs',
      'navigation',
      'overlays',
      'feedback',
      'display',
    ]);
    const ids = SHOWCASE_SECTIONS.map((section) => section.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const group of SHOWCASE_GROUPS) {
      expect(group.sections.length).toBeGreaterThan(0);
      for (const section of group.sections) expect(section.category).toBe(group.id);
    }
  });

  test('every_exported_component_has_a_showcase_section', () => {
    const covered = new Set(SHOWCASE_SECTIONS.flatMap((section) => section.covers));
    const missing = componentExports.filter((name) => !covered.has(name));
    expect(missing).toEqual([]);

    // The other direction: a section may only claim components the design system still exports.
    const exported = new Set(componentExports);
    const stale = [...covered].filter((name) => !exported.has(name));
    expect(stale).toEqual([]);
  });

  test('has the new pages: PasswordField, the motion utilities and the providers', () => {
    const ids = SHOWCASE_SECTIONS.map((section) => section.id);
    expect(ids).toContain('password-field');
    expect(ids).toContain('motion');
    expect(ids).toContain('providers');
    const passwordField = SHOWCASE_SECTIONS.find((section) => section.id === 'password-field');
    expect(passwordField?.category).toBe('inputs');
    expect(passwordField?.covers).toContain('PasswordField');
  });

  for (const section of SHOWCASE_SECTIONS) {
    test(`renders the ${section.id} page`, async () => {
      const Page = section.component;
      const { container } = render(
        <DesignSystemProvider platform="linux" theme="polar-night">
          <TooltipProvider>
            <ToastProvider>
              <Page />
            </ToastProvider>
          </TooltipProvider>
        </DesignSystemProvider>,
      );
      // Base UI measures scroll areas after mount (a state update): let it settle inside act.
      await act(async () => {});
      expect(container.textContent?.length ?? 0).toBeGreaterThan(0);
    });
  }
});
