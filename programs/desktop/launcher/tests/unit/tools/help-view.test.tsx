import { describe, expect, test } from 'bun:test';
import { screen, within } from '@testing-library/react';

import { renderLauncher } from '../launcher.harness';

describe('Help view', () => {
  test('help_view_shows_arrow_keys_as_symbols', async () => {
    const { user } = await renderLauncher();
    await user.type(screen.getByRole('combobox', { name: /Search apps/ }), '/help{Enter}');
    const help = screen.getByRole('region', { name: 'Help' });
    expect(help).not.toHaveTextContent(/arrow(up|down)/i);
    const move = within(help).getByText('Move').parentElement;
    expect(move).toHaveTextContent('↑');
    expect(move).toHaveTextContent('↓');
  });

  test('help_view_lists_every_slash_command', async () => {
    const { user } = await renderLauncher();
    await user.type(screen.getByRole('combobox', { name: /Search apps/ }), '/help{Enter}');
    const help = screen.getByRole('region', { name: 'Help' });
    for (const command of ['open', 'theme', 'size', 'pin', 'settings', 'vault', 'rescan', 'help']) {
      expect(help).toHaveTextContent(`/${command}`);
    }
    expect(help).toHaveTextContent('/ask');
  });
});
