import { describe, expect, mock, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { PasswordField } from '../../../src/components/inputs';
import { stylesRoot } from '../../support/tailwind.harness';

function Controlled(props: { revealable?: boolean; onChange?: (value: string) => void }) {
  const [value, setValue] = useState('nord-polar-7');
  return (
    <PasswordField
      label="Vault password"
      value={value}
      onChange={(next) => {
        setValue(next);
        props.onChange?.(next);
      }}
      revealable={props.revealable}
    />
  );
}

describe('PasswordField', () => {
  test('password_field_hides_value_until_revealed', async () => {
    const user = userEvent.setup();
    render(<Controlled />);
    const input = screen.getByLabelText('Vault password');
    expect(input).toHaveAttribute('type', 'password');
    expect(input).toHaveValue('nord-polar-7');

    const toggle = screen.getByRole('button', { name: 'Show password' });
    expect(toggle).toHaveAttribute('aria-pressed', 'false');
    await user.click(toggle);
    expect(input).toHaveAttribute('type', 'text');
    // One scheme: the name stays stable and aria-pressed carries the state.
    expect(toggle).toHaveAccessibleName('Show password');
    expect(toggle).toHaveAttribute('aria-pressed', 'true');
    expect(screen.queryByRole('button', { name: 'Hide password' })).toBeNull();
    expect(input).toHaveFocus();

    await user.click(toggle);
    expect(input).toHaveAttribute('type', 'password');
    expect(toggle).toHaveAttribute('aria-pressed', 'false');
  });

  test('a keyboard-focused toggle keeps focus and the field draws no second ring', async () => {
    const user = userEvent.setup();
    render(<Controlled />);
    await user.tab();
    await user.tab();
    const toggle = screen.getByRole('button', { name: 'Show password' });
    expect(toggle).toHaveFocus();
    await user.keyboard('{Enter}');
    expect(toggle).toHaveFocus();
    expect(toggle).toHaveAttribute('aria-pressed', 'true');
    // The field's ring reacts only to its own input, so the toggle's ring is the only one.
    const css = readFileSync(join(stylesRoot, 'design-system.utilities.css'), 'utf8');
    expect(css).toContain(':has(:is(input, textarea):focus-visible)');
    expect(css).not.toMatch(/:has\(:focus-visible\)/);
  });

  test('never offers autocomplete or spellcheck', () => {
    render(<Controlled />);
    const input = screen.getByLabelText('Vault password');
    expect(input).toHaveAttribute('autocomplete', 'off');
    expect(input).toHaveAttribute('spellcheck', 'false');
  });

  test('reports edits through onChange with the new string', async () => {
    const user = userEvent.setup();
    const onChange = mock((_: string) => {});
    render(<Controlled onChange={onChange} />);
    const input = screen.getByLabelText('Vault password');
    await user.clear(input);
    await user.type(input, 'ab');
    expect(onChange).toHaveBeenLastCalledWith('ab');
    expect(input).toHaveValue('ab');
  });

  test('revealable={false} renders no toggle', () => {
    render(<Controlled revealable={false} />);
    expect(screen.queryByRole('button', { name: 'Show password' })).toBeNull();
    expect(screen.getByLabelText('Vault password')).toHaveAttribute('type', 'password');
  });

  test('shows the error and marks the input invalid', () => {
    render(<PasswordField label="Password" value="" onChange={() => {}} error="Wrong password." />);
    expect(screen.getByLabelText('Password')).toHaveAttribute('aria-invalid', 'true');
    expect(screen.getByText('Wrong password.')).toBeInTheDocument();
  });
});
