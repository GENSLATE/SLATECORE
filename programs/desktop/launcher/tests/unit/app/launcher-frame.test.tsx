import { describe, expect, test } from 'bun:test';
import { screen, waitFor, within } from '@testing-library/react';

import { COLLAPSE_FALLBACK_MS as COLLAPSE_MS } from '../../../src/app/motion.util';
import type { AppList } from '../../../src/ipc/launcher.types';
import { renderLauncher } from '../launcher.harness';

const frame = () => document.querySelector('[data-slot="launcher-window"]');

describe('launcher frame', () => {
  test('tabs_render_in_genslate_portapps_portableapps_order', async () => {
    const { backend } = await renderLauncher();
    const tabs = within(screen.getByRole('tablist', { name: 'App sources' })).getAllByRole('tab');
    const { tabs: expected } = await backend.listApps();
    const count = (source: string) => expected.find((tab) => tab.source === source)?.count;
    expect(tabs.map((tab) => tab.getAttribute('aria-label'))).toEqual([
      `GENSLATE, ${count('genslate')} apps`,
      `portapps.io, ${count('portapps')} apps`,
      `PortableApps.com, ${count('portableapps')} apps`,
    ]);
    for (const [index, source] of ['genslate', 'portapps', 'portableapps'].entries()) {
      expect(tabs[index]).toHaveTextContent(String(count(source)));
    }
    expect(tabs[0]).toHaveAttribute('aria-selected', 'true');
  });

  test('opening_settings_sets_expanded_and_esc_returns_to_apps', async () => {
    const { backend, user } = await renderLauncher();
    expect(frame()).not.toHaveAttribute('data-expanded');
    await user.click(screen.getByRole('button', { name: 'Settings' }));
    expect(frame()).toHaveAttribute('data-expanded');
    expect(screen.getByRole('region', { name: 'Settings' })).toBeInTheDocument();
    expect(backend.setExpanded).toHaveBeenLastCalledWith(true);
    expect(screen.queryByRole('group', { name: 'Apps' })).toBeNull();

    await user.keyboard('{Escape}');
    expect(frame()).not.toHaveAttribute('data-expanded');
    expect(screen.queryByRole('region', { name: 'Settings' })).toBeNull();
    expect(screen.getByRole('group', { name: 'Apps' })).toBeInTheDocument();
  });

  test('closing_tool_collapses_after_collapse_delay', async () => {
    const { backend, user } = await renderLauncher();
    await user.click(screen.getByRole('button', { name: 'Settings' }));
    expect(backend.setExpanded).toHaveBeenLastCalledWith(true);
    const closedAt = performance.now();
    await user.keyboard('{Escape}');
    // The hit area stays wide until the frame has finished shrinking.
    expect(backend.setExpanded).toHaveBeenLastCalledWith(true);
    await waitFor(() => expect(backend.setExpanded).toHaveBeenLastCalledWith(false), {
      timeout: COLLAPSE_MS * 4,
    });
    expect(performance.now() - closedAt).toBeGreaterThanOrEqual(COLLAPSE_MS - 20);
  });

  test('hiding the window resets it to the apps, so nothing half-typed stays behind', async () => {
    const { emit, user } = await renderLauncher();
    const search = screen.getByRole('combobox', { name: /Search apps/ });
    await user.type(search, '/vault{Enter}');
    await user.type(screen.getByLabelText('Vault password'), 'half-typed');
    await user.type(search, '/the');

    emit('willHide', undefined);
    expect(search).toHaveValue('');
    expect(frame()).not.toHaveAttribute('data-expanded');
    expect(screen.queryByRole('region', { name: 'Settings' })).toBeNull();
    // Once the frame has shrunk, the vault form (and the typed secret) is gone from the page.
    const secretFields = () => document.querySelectorAll('input[type="password"]').length;
    await waitFor(() => expect(secretFields()).toBe(0), { timeout: COLLAPSE_MS * 4 });
  });

  test('empty_catalog_shows_empty_state_per_tab', async () => {
    const list: AppList = {
      apps: [],
      recent: [],
      tabs: [
        { source: 'genslate', label: 'GENSLATE', count: 0 },
        { source: 'portapps', label: 'portapps.io', count: 0 },
        { source: 'portableapps', label: 'PortableApps.com', count: 0 },
      ],
    };
    const { user } = await renderLauncher({ apps: [], list });
    expect(screen.getByText('No GENSLATE apps yet')).toBeInTheDocument();
    expect(screen.getByText(/programs\/genslate/)).toBeInTheDocument();

    await user.click(screen.getByRole('tab', { name: /^portapps\.io/ }));
    expect(screen.getByText('No portapps.io apps yet')).toBeInTheDocument();
    expect(screen.getByText(/programs\/portapps\.io/)).toBeInTheDocument();

    await user.click(screen.getByRole('tab', { name: /^PortableApps\.com/ }));
    expect(screen.getByText('No PortableApps.com apps yet')).toBeInTheDocument();
    expect(screen.getByText(/programs\/portableapps\.com/)).toBeInTheDocument();
  });

  test('empty drive shows one designed empty state under the single heading', async () => {
    await renderLauncher({ apps: [] });
    expect(screen.queryByRole('tablist', { name: 'App sources' })).toBeNull();
    expect(screen.getByRole('heading', { name: 'GENSLATE apps' })).toBeInTheDocument();
    expect(screen.getByText('No GENSLATE apps yet')).toBeInTheDocument();
  });

  test('single_source_shows_heading_instead_of_tabs', async () => {
    await renderLauncher({ noThirdParty: true });
    expect(screen.queryByRole('tablist', { name: 'App sources' })).toBeNull();
    const heading = screen.getByRole('heading', { name: 'GENSLATE apps' });
    expect(heading.parentElement).toHaveTextContent(/GENSLATE apps\s*\d+/);
    expect(screen.getByRole('group', { name: 'Apps' })).toBeInTheDocument();
  });

  test('unavailable_group_is_collapsed_by_default', async () => {
    const { user } = await renderLauncher();
    const header = screen.getByRole('button', { name: /^Unavailable/ });
    expect(header).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByRole('option', { name: /Theater/ })).toBeNull();
    await user.click(header);
    expect(header).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('option', { name: /Theater/ })).toBeInTheDocument();
  });

  test('status_bar_shows_drive_name_and_free_space', async () => {
    const { backend } = await renderLauncher();
    const bar = document.querySelector('[data-slot="launcher-status-bar"]');
    if (!(bar instanceof HTMLElement)) throw new Error('status bar missing');
    const volume = await backend.volume();
    await waitFor(() => expect(bar).toHaveTextContent(volume?.name ?? 'missing'));
    expect(bar).toHaveTextContent(volume?.label ?? 'missing');
    expect(bar).toHaveTextContent(/\d+(\.\d)? GB free/);
  });

  test('an_ignored_settings_file_shows_a_warning_that_opens_it', async () => {
    const { backend, user } = await renderLauncher({ issue: 'line 3: expected a value' });
    const bar = document.querySelector('[data-slot="launcher-status-bar"]');
    if (!(bar instanceof HTMLElement)) throw new Error('status bar missing');
    await user.click(within(bar).getByRole('button', { name: 'Settings' }));
    expect(backend.openConfigFile).toHaveBeenCalledWith('settings');
  });

  test('the frame is flat: a 1px border and no shadow (only floating popups get one)', async () => {
    await renderLauncher();
    const panel = document.querySelector('[data-slot="launcher-frame"]');
    expect(panel?.className).toMatch(/\bborder\b/);
    expect(panel?.className).not.toMatch(/shadow/);
  });

  test('run_mode_badge_sits_in_the_titlebar_not_the_status_bar', async () => {
    await renderLauncher();
    const titleBar = document.querySelector('[data-slot="launcher-titlebar"]');
    const statusBar = document.querySelector('[data-slot="launcher-status-bar"]');
    expect(titleBar).toHaveTextContent('PREVIEW');
    expect(statusBar).not.toHaveTextContent('PREVIEW');
  });

  test('rail_lists_desktop_documents_downloads_music_pictures_videos_then_vault', async () => {
    await renderLauncher();
    const folders = within(screen.getByRole('list', { name: 'Folders' })).getAllByRole('button');
    expect(
      folders.map((button) => button.getAttribute('aria-label') ?? button.textContent),
    ).toEqual([
      'Desktop',
      'Documents',
      'Downloads',
      'Music',
      'Pictures',
      'Videos',
      'Vault (locked)',
    ]);
  });

  test('rail folders open the portable folders', async () => {
    const { backend, user } = await renderLauncher();
    await user.click(screen.getByRole('button', { name: 'Documents' }));
    expect(backend.openFolder).toHaveBeenCalledWith('documents');
  });

  test('typing searches every source and Enter launches with the launch pop', async () => {
    const { backend, user } = await renderLauncher();
    await user.type(screen.getByRole('combobox', { name: /Search apps/ }), 'firefox');
    expect(screen.getByRole('listbox', { name: 'Search results' })).toBeInTheDocument();
    await user.keyboard('{Enter}');
    expect(backend.launch).toHaveBeenCalledWith('portableapps/FirefoxPortable', undefined);
  });

  test('document_title_is_slatecore_launcher', async () => {
    const html = await Bun.file(new URL('../../../index.html', import.meta.url)).text();
    expect(html).toContain('<title>SLATECORE LAUNCHER</title>');
    const tray = await Bun.file(new URL('../../../tray-menu.html', import.meta.url)).text();
    expect(tray).toMatch(/<title>SLATECORE LAUNCHER[^<]*<\/title>/);
  });

  test('vite_dev_server_uses_port_1420_strict', async () => {
    const { default: config } = await import('../../../vite.config');
    expect(config.server?.port).toBe(1420);
    expect(config.server?.strictPort).toBe(true);
    expect(config.preview?.port).toBe(1420);
  });
});
