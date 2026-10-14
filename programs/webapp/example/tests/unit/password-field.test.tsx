import { describe, expect, test } from 'bun:test';
import { DesignSystemProvider } from '@genslate/design-system';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { PasswordFieldSection } from '../../src/features/showcase/sections/inputs/password-field.section';

function renderPage() {
  return render(
    <DesignSystemProvider platform="linux" theme="polar-night">
      <PasswordFieldSection />
    </DesignSystemProvider>,
  );
}

describe('PasswordField page', () => {
  test('shows the field masked and reveals it on demand', async () => {
    const user = userEvent.setup();
    renderPage();
    const field = screen.getByLabelText('Passphrase');
    expect(field).toHaveAttribute('type', 'password');
    await user.click(screen.getAllByRole('button', { name: /show password/i })[0] as HTMLElement);
    expect(field).toHaveAttribute('type', 'text');
  });

  test('the uncontrolled specimen reads the secret once on submit and clears the field', async () => {
    const user = userEvent.setup();
    renderPage();
    const field = screen.getByLabelText('Vault password');
    const form = field.closest('form');
    if (form === null) throw new Error('the uncontrolled specimen is not a form');
    // Submitting empty shows the error, and typing clears it.
    await user.click(within(form).getByRole('button', { name: 'Unlock' }));
    expect(field).toHaveAttribute('aria-invalid', 'true');
    await user.type(field, 'aurora');
    expect(field).not.toHaveAttribute('aria-invalid');
    await user.keyboard('{Enter}');
    expect(field).toHaveValue('');
    expect(within(form).getByRole('status')).toHaveTextContent('Read once, then cleared');
  });

  test('both modes have a state matrix and the props table explains the union', () => {
    renderPage();
    for (const caption of ['Password field states', 'Uncontrolled password field states']) {
      const table = screen.getByRole('table', { name: caption });
      expect(
        within(table)
          .getAllByRole('columnheader')
          .map((cell) => cell.textContent),
      ).toEqual(['Variant', 'Rest', 'Hover', 'Focus', 'Filled', 'Invalid', 'Disabled']);
      expect(within(table).getAllByRole('rowheader')).toHaveLength(3);
    }
    expect(screen.queryByText(/always controlled/i)).toBeNull();
    expect(screen.queryByText(/never keeps a copy/i)).toBeNull();
    const row = screen.getByRole('row', { name: /value \/ onChange/ });
    expect(row).toHaveTextContent(/optional/i);
    expect(row).toHaveTextContent(/uncontrolled/i);
    expect(row).toHaveTextContent(/only in the DOM input/i);
  });
});
