import { describe, expect, mock, test } from 'bun:test';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { PasswordField } from '../../../src/components/inputs';

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
    const hide = screen.getByRole('button', { name: 'Hide password' });
    expect(hide).toHaveAttribute('aria-pressed', 'true');
    expect(input).toHaveFocus();

    await user.click(hide);
    expect(input).toHaveAttribute('type', 'password');
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
