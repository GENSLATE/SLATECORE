import { beforeEach, describe, expect, test } from 'bun:test';
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { App } from '../../src/app/app.component';
import { AppProviders } from '../../src/app/app.providers';

/** Renders the app and lets Base UI's post-mount viewport measurement settle inside act. */
async function renderApp() {
  const result = render(
    <AppProviders>
      <App />
    </AppProviders>,
  );
  await act(async () => {});
  return result;
}

beforeEach(() => {
  localStorage.clear();
  localStorage.setItem('genslate.theme', 'polar-night');
  delete document.documentElement.dataset['theme'];
});

describe('Design Kit app', () => {
  test('renders the titlebar, sidebar navigation, main content and status bar', async () => {
    await renderApp();
    const banner = screen.getByRole('banner');
    expect(banner).toHaveAttribute('data-tauri-drag-region');
    expect(within(banner).getByRole('button', { name: /Search components/ })).toBeInTheDocument();
    expect(screen.getByRole('navigation', { name: 'Showcase pages' })).toBeInTheDocument();
    expect(screen.getByRole('main')).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 1, name: 'Colors' })).toBeInTheDocument();
    const status = screen.getByRole('contentinfo', { name: 'Status bar' });
    expect(within(status).getByText('SLATECORE')).toBeInTheDocument();
    expect(within(status).getByText('Browser')).toBeInTheDocument();
  });

  test('switches pages from the sidebar and remembers the choice', async () => {
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByRole('button', { name: 'Button' }));
    expect(screen.getByRole('heading', { level: 1, name: 'Button' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Button' })).toHaveAttribute('aria-current', 'page');
    expect(JSON.parse(localStorage.getItem('genslate.example.page') ?? 'null')).toBe('button');
  });

  test('the titlebar theme toggle flips data-theme', async () => {
    const user = userEvent.setup();
    await renderApp();
    expect(document.documentElement).toHaveAttribute('data-theme', 'polar-night');
    await user.click(screen.getByRole('button', { name: 'Switch to Snow Storm' }));
    expect(document.documentElement).toHaveAttribute('data-theme', 'snow-storm');
    await user.click(screen.getByRole('button', { name: 'Switch to Polar Night' }));
    expect(document.documentElement).toHaveAttribute('data-theme', 'polar-night');
  });

  test('keyboard: mod+B toggles the sidebar, mod+K opens the palette, mod+shift+L toggles the theme', async () => {
    const user = userEvent.setup();
    await renderApp();
    const shell = document.querySelector('[data-slot="app-shell"]');
    expect(shell).not.toHaveAttribute('data-sidebar-collapsed');
    // happy-dom's user agent is Linux, so `mod` is Control.
    await user.keyboard('{Control>}b{/Control}');
    expect(shell).toHaveAttribute('data-sidebar-collapsed');
    await user.keyboard('{Control>}b{/Control}');
    expect(shell).not.toHaveAttribute('data-sidebar-collapsed');
    await user.keyboard('{Control>}{Shift>}l{/Shift}{/Control}');
    expect(document.documentElement).toHaveAttribute('data-theme', 'snow-storm');
  });

  test('mod+K opens the command palette, which navigates to a page', async () => {
    const user = userEvent.setup();
    await renderApp();
    await user.keyboard('{Control>}k{/Control}');
    expect(await screen.findByRole('dialog', { name: 'Command palette' })).toBeInTheDocument();
    await user.keyboard('Segmented');
    await user.keyboard('{Enter}');
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Segmented Control' }),
    ).toBeInTheDocument();
  });

  test('the settings button opens the appearance inspector', async () => {
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByRole('button', { name: 'Appearance and about' }));
    const inspector = screen.getByRole('region', { name: 'Appearance' });
    await user.click(within(inspector).getByRole('button', { name: 'Light' }));
    expect(document.documentElement).toHaveAttribute('data-theme', 'snow-storm');
    await user.click(within(inspector).getByRole('button', { name: 'Close inspector' }));
    expect(screen.queryByRole('region', { name: 'Appearance' })).toBeNull();
  });

  test('theme_toggle_switches_polar_night_and_snow_storm', async () => {
    const user = userEvent.setup();
    await renderApp();
    const root = document.documentElement;
    const status = screen.getByRole('contentinfo', { name: 'Status bar' });
    expect(root).toHaveAttribute('data-theme', 'polar-night');
    expect(within(status).getByText('Polar Night')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Switch to Snow Storm' }));
    expect(root).toHaveAttribute('data-theme', 'snow-storm');
    expect(localStorage.getItem('genslate.theme')).toBe('snow-storm');
    expect(within(status).getByText('Snow Storm')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Switch to Polar Night' }));
    expect(root).toHaveAttribute('data-theme', 'polar-night');
    expect(localStorage.getItem('genslate.theme')).toBe('polar-night');
    expect(within(status).getByText('Polar Night')).toBeInTheDocument();
  });

  test('kit_header_and_document_title_say_slatecore_design_kit', async () => {
    document.title = '';
    await renderApp();
    expect(screen.getByRole('banner')).toHaveTextContent('SLATECORE Design Kit');
    expect(document.title).toBe('SLATECORE Design Kit');
    // The static page title (shown before any script runs) agrees.
    const html = await Bun.file(new URL('../../index.html', import.meta.url)).text();
    expect(html).toContain('<title>SLATECORE Design Kit</title>');
    expect(html).not.toMatch(/genslate example/i);
  });
});
