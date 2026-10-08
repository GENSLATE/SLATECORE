import { describe, expect, test } from 'bun:test';
import { DesignSystemProvider } from '@genslate/design-system';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { PasswordFieldSection } from '../../src/features/showcase/sections/inputs/password-field.section';

describe('PasswordField page', () => {
  test('shows the field masked and reveals it on demand', async () => {
    const user = userEvent.setup();
    render(
      <DesignSystemProvider platform="linux" theme="polar-night">
        <PasswordFieldSection />
      </DesignSystemProvider>,
    );
    const field = screen.getByLabelText('Passphrase');
    expect(field).toHaveAttribute('type', 'password');
    await user.click(screen.getAllByRole('button', { name: /show password/i })[0] as HTMLElement);
    expect(field).toHaveAttribute('type', 'text');
  });
});
