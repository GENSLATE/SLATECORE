import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { DesignSystemProvider, TooltipProvider } from '@genslate/design-system';
import { act, render, screen } from '@testing-library/react';
import { AppSidebar } from '../../src/features/navigation/app-sidebar.component';
import { WindowContextMenuSection } from '../../src/features/showcase/sections/window/window-context-menu.section';

const indexHtml = readFileSync(new URL('../../index.html', import.meta.url), 'utf8');

describe('Design Kit sidebar', () => {
  test('keeps a clear inset under the last page so it does not sit against the status bar', async () => {
    render(
      <DesignSystemProvider platform="linux" theme="polar-night">
        <AppSidebar selectedId="colors" onSelect={() => {}} />
      </DesignSystemProvider>,
    );
    await act(async () => {});
    const viewport = screen
      .getByRole('navigation', { name: 'Showcase pages' })
      .querySelector('[data-slot="scroll-area-viewport"]');
    expect(viewport).toHaveClass('pb-3');
  });
});

describe('Design Kit document head', () => {
  test('declares an inline SVG favicon in the Nord Frost accent, so the browser never asks for /favicon.ico', () => {
    const icon =
      /<link\s+rel="icon"\s+type="image\/svg\+xml"\s+href="(data:image\/svg\+xml,[^"]+)"/.exec(
        indexHtml,
      );
    expect(icon).not.toBeNull();
    const svg = decodeURIComponent(icon?.[1]?.replace('data:image/svg+xml,', '') ?? '');
    expect(svg).toContain('<svg');
    expect(svg.toLowerCase()).toContain('#88c0d0');
  });

  test('keeps the document title', () => {
    expect(indexHtml).toContain('<title>SLATECORE Design Kit</title>');
  });
});

describe('Window Context Menu page', () => {
  test('sets the last-command readout apart from the description beside it', () => {
    render(
      <DesignSystemProvider platform="linux" theme="polar-night">
        <TooltipProvider>
          <WindowContextMenuSection />
        </TooltipProvider>
      </DesignSystemProvider>,
    );
    const readout = screen.getByText(/Last command:/);
    expect(readout).toHaveTextContent('Last command: Nothing yet');
    expect(readout).toHaveClass('pl-6');
  });
});
