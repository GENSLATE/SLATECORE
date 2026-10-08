import { describe, expect, test } from 'bun:test';
import { fireEvent, screen, waitFor } from '@testing-library/react';

import { app } from '../fixtures';
import { renderTrayMenu, type SpiedBackend, settle } from '../launcher.harness';

const APPS = [
  app('genslate/terminal', { favorite: true }),
  app('genslate/theater', { status: 'missing-exe' }),
];
const LIST = {
  apps: APPS,
  tabs: [{ source: 'genslate' as const, label: 'GENSLATE', count: 2 }],
  recent: ['genslate/terminal', 'genslate/theater'],
};

const setup = () => renderTrayMenu({ apps: APPS, recent: LIST.recent, list: LIST });

const openSubmenu = async (name: string) => {
  fireEvent.click(await screen.findByRole('menuitem', { name }));
  return screen.findByRole('menu', { name });
};

describe('TrayMenu', () => {
  test('opens with the header and the top-level rows', async () => {
    await setup();
    const menu = await screen.findByRole('menu', { name: 'SLATECORE LAUNCHER' });
    expect(menu.querySelector('[data-slot="menu-header"]')).toHaveTextContent(
      /SLATECORE LAUNCHER.*Preview · SLATECORE.*v0\.1\.0/,
    );
    for (const name of ['Recent', 'Favorites', 'Folders', 'Appearance', 'Settings']) {
      expect(screen.getByRole('menuitem', { name })).toHaveAttribute('aria-haspopup', 'menu');
    }
    expect(screen.getByRole('menuitem', { name: /Show Launcher/ })).toHaveTextContent(
      'Ctrl+Alt+Space',
    );
    expect(screen.getByRole('menuitem', { name: 'Quit SLATECORE LAUNCHER' })).toBeInTheDocument();
  });

  test('rows call the shell and the window hides once the menu has closed', async () => {
    const { backend } = await setup();
    fireEvent.click(await screen.findByRole('menuitem', { name: 'Pin on Top' }));
    expect(backend.setPinned).toHaveBeenCalledWith(true);
    await waitFor(() => expect(backend.hideTrayMenu).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole('menu')).toBeNull();
  });

  test('Help opens the launcher on its help view', async () => {
    const { backend } = await setup();
    fireEvent.click(await screen.findByRole('menuitem', { name: 'Help' }));
    expect(backend.show).toHaveBeenCalledWith('help');
  });

  test('Recent launches ready apps and disables missing ones', async () => {
    const { backend } = await setup();
    await openSubmenu('Recent');
    expect(screen.getByRole('menuitem', { name: 'Theater' })).toHaveAttribute(
      'aria-disabled',
      'true',
    );
    fireEvent.click(screen.getByRole('menuitem', { name: 'Terminal' }));
    expect(backend.launch).toHaveBeenCalledWith('genslate/terminal');
    // The launch refreshes the catalog (Recent changes): let that land before the test ends.
    await settle();
  });

  test('Appearance marks the current theme and writes a new one', async () => {
    const { backend } = await renderTrayMenu({ theme: 'polar-night' });
    await openSubmenu('Appearance');
    expect(screen.getByRole('menuitemradio', { name: 'Polar Night' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    fireEvent.click(screen.getByRole('menuitemradio', { name: 'Snow Storm' }));
    expect(backend.setSetting).toHaveBeenCalledWith('theme', 'snow-storm');
  });

  test('Settings opens the Settings tool and the launcher files, with no startup toggle', async () => {
    const { backend } = await setup();
    await openSubmenu('Settings');
    expect(screen.queryByRole('menuitemcheckbox')).toBeNull();
    fireEvent.click(screen.getByRole('menuitem', { name: 'Open Settings…' }));
    expect(backend.show).toHaveBeenCalledWith('settings');
  });

  test('the shell can close it and reopen it at a new anchor', async () => {
    const { backend, emit } = await setup();
    await screen.findByRole('menu');
    emit('trayMenuClose', undefined);
    await waitFor(() => expect(backend.hideTrayMenu).toHaveBeenCalledTimes(1));
    emit('trayMenuOpen', { x: 20, y: 20, opensUp: false, alignEnd: false });
    expect(await screen.findByRole('menu', { name: 'SLATECORE LAUNCHER' })).toBeInTheDocument();
  });

  test('Folders opens a portable folder', async () => {
    const { backend } = await setup();
    await openSubmenu('Folders');
    fireEvent.click(screen.getByRole('menuitem', { name: 'Documents' }));
    expect(backend.openFolder).toHaveBeenCalledWith('documents');
  });

  test('Show, the launcher files, Rescan and Quit run the shared commands', async () => {
    const show = await setup();
    fireEvent.click(await screen.findByRole('menuitem', { name: /Show Launcher/ }));
    expect(show.backend.show).toHaveBeenCalledWith('apps');
    show.unmount();

    for (const [row, call] of [
      [
        'Edit settings.toml',
        (b: SpiedBackend) => b.openConfigFile.mock.calls[0]?.[0] === 'settings',
      ],
      [
        'Edit keybindings.toml',
        (b: SpiedBackend) => b.openConfigFile.mock.calls[0]?.[0] === 'keybindings',
      ],
      ['Open Logs', (b: SpiedBackend) => b.openConfigFile.mock.calls[0]?.[0] === 'logs'],
      ['Rescan Apps', (b: SpiedBackend) => b.rescan.mock.calls.length === 1],
    ] as const) {
      const view = await setup();
      await openSubmenu('Settings');
      fireEvent.click(screen.getByRole('menuitem', { name: row }));
      expect(call(view.backend)).toBe(true);
      await settle();
      view.unmount();
    }

    const quit = await setup();
    fireEvent.click(await screen.findByRole('menuitem', { name: 'Quit SLATECORE LAUNCHER' }));
    expect(quit.backend.quit).toHaveBeenCalledTimes(1);
  });

  test('tray rows hold no command logic of their own: they go through the registry', async () => {
    const dir = new URL('../../../src/features/tray-menu/', import.meta.url);
    for (const name of ['tray-menu.component.tsx', 'tray-menu-submenus.component.tsx']) {
      const source = await Bun.file(new URL(name, dir)).text();
      expect(source, name).not.toMatch(
        /backend\.(launch|setPinned|setSetting|show|rescan|openFolder|openConfigFile|quit)\(/,
      );
    }
  });
});
