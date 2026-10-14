import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { DesignSystemProvider, TooltipProvider } from '@genslate/design-system';
import { render, screen, within } from '@testing-library/react';
import { ButtonSection } from '../../src/features/showcase/sections/actions/button.section';
import { SelectSection } from '../../src/features/showcase/sections/inputs/select.section';

const srcRoot = join(import.meta.dir, '../../src');
const indexHtml = readFileSync(join(import.meta.dir, '../../index.html'), 'utf8');

function sourceFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? sourceFiles(path) : [path];
  });
}

function renderPage(page: React.ReactNode) {
  return render(
    <DesignSystemProvider platform="linux" theme="polar-night">
      <TooltipProvider>{page}</TooltipProvider>
    </DesignSystemProvider>,
  );
}

describe('Button page state matrix', () => {
  test('forces pressed with the same colour token the :active state applies', () => {
    renderPage(<ButtonSection />);
    const row = screen.getByRole('row', { name: /Danger/ });
    const [, hover, pressed] = within(row).getAllByRole('button');
    expect(hover).toHaveClass('bg-danger-hover');
    expect(pressed).toHaveClass('bg-danger-active');
  });

  test('no Kit source fakes a state with a filter', () => {
    const offenders = sourceFiles(srcRoot)
      .filter((file) => /\.(tsx|ts)$/.test(file))
      .filter((file) =>
        /\b(brightness|contrast|saturate|blur|invert)-\d+|\bfilter\b-/.test(
          readFileSync(file, 'utf8'),
        ),
      )
      .map((file) => file.slice(srcRoot.length));
    expect(offenders).toEqual([]);
  });
});

describe('Select page', () => {
  test('every select trigger has an accessible name', () => {
    renderPage(<SelectSection />);
    const triggers = screen.getAllByRole('combobox');
    expect(triggers.length).toBeGreaterThan(20);
    // Named by aria-label, aria-labelledby or a <label for> pointing at the trigger.
    const unnamed = triggers.filter(
      (trigger) =>
        !trigger.getAttribute('aria-label') &&
        !trigger.getAttribute('aria-labelledby') &&
        document.querySelector(`label[for="${trigger.id}"]`) === null,
    );
    expect(unnamed.map((trigger) => trigger.outerHTML)).toEqual([]);
  });
});

describe('Document head', () => {
  test('explains why the pre-paint hex backgrounds are exempt from the token rule', () => {
    const style = /<style>([\s\S]*?)<\/style>/.exec(indexHtml)?.[1] ?? '';
    expect(style).toMatch(/exempt/i);
    expect(style).toMatch(/before (the )?first paint/i);
  });
});
