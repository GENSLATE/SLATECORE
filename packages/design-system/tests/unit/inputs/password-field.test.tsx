import { describe, expect, mock, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useRef, useState } from 'react';
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

describe('PasswordField, uncontrolled', () => {
  const SECRET = 'aurora-borealis-42';

  function Form({ error, onSubmit }: { error?: string; onSubmit: (secret: string) => void }) {
    const ref = useRef<HTMLInputElement>(null);
    return (
      <form
        onSubmit={(event) => {
          event.preventDefault();
          const input = ref.current;
          if (input === null) return;
          onSubmit(input.value);
          input.value = '';
        }}
      >
        <PasswordField ref={ref} label="Vault password" error={error} />
        <button type="submit">Unlock</button>
      </form>
    );
  }

  test('the secret lives only in the input: read through ref, then cleared', async () => {
    const user = userEvent.setup();
    const onSubmit = mock((_: string) => {});
    const view = render(<Form onSubmit={onSubmit} />);
    const input = screen.getByLabelText('Vault password');
    await user.type(input, SECRET);
    expect(input).toHaveValue(SECRET);
    // Not even Base UI's field state keeps a copy while typing.
    expect(reactStateContains(view.container, SECRET)).toBe(false);

    await user.keyboard('{Enter}');
    expect(onSubmit).toHaveBeenCalledWith(SECRET);
    expect(input).toHaveValue('');
    // A re-render (an error arrives) must not bring the old value back.
    view.rerender(<Form onSubmit={onSubmit} error="Wrong password." />);
    expect(input).toHaveValue('');
    expect(input).toHaveAttribute('aria-invalid', 'true');
    expect(reactStateContains(view.container, SECRET)).toBe(false);
  });

  test('a controlled field does hold the value in React state (the contrast)', async () => {
    const user = userEvent.setup();
    function Held() {
      const [value, setValue] = useState('');
      return <PasswordField label="Password" value={value} onChange={setValue} />;
    }
    const view = render(<Held />);
    await user.type(screen.getByLabelText('Password'), SECRET);
    expect(reactStateContains(view.container, SECRET)).toBe(true);
  });

  test('keeps the reveal toggle, input props and native events', async () => {
    const user = userEvent.setup();
    const onInput = mock(() => {});
    render(<PasswordField label="Vault password" onInput={onInput} placeholder="Password" />);
    const input = screen.getByLabelText('Vault password');
    expect(input).toHaveAttribute('type', 'password');
    expect(input).toHaveAttribute('placeholder', 'Password');
    expect(input).toHaveAttribute('autocomplete', 'off');
    await user.type(input, 'ab');
    expect(onInput).toHaveBeenCalledTimes(2);
    await user.click(screen.getByRole('button', { name: 'Show password' }));
    expect(input).toHaveAttribute('type', 'text');
    expect(input).toHaveValue('ab');
    expect(input).toHaveFocus();
  });

  test('description, disabled and the error replace each other as in the controlled field', () => {
    const { rerender } = render(
      <PasswordField label="Vault password" description="8 or more characters." disabled />,
    );
    const input = screen.getByLabelText('Vault password');
    expect(screen.getByText('8 or more characters.')).toBeInTheDocument();
    expect(input).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Show password' })).toBeDisabled();
    rerender(<PasswordField label="Vault password" description="8 or more." error="Too short." />);
    expect(screen.getByText('Too short.')).toBeInTheDocument();
    expect(screen.queryByText('8 or more.')).toBeNull();
  });
});

/**
 * Whether `text` sits anywhere in the React state or props under `container` (hooks of every
 * component, Base UI's included). DOM nodes are not searched: the input is allowed to hold it.
 */
function reactStateContains(container: HTMLElement, text: string): boolean {
  const rootKey = Object.keys(container).find((key) => key.startsWith('__reactContainer$'));
  const root: unknown = rootKey === undefined ? undefined : Reflect.get(container, rootKey);
  const seen = new Set<unknown>();
  const holds = (value: unknown, depth: number): boolean => {
    if (typeof value === 'string') return value.includes(text);
    if (typeof value !== 'object' || value === null || depth > 8 || seen.has(value)) return false;
    if (value instanceof Node) return false;
    seen.add(value);
    return Object.values(value).some((item) => holds(item, depth + 1));
  };
  const fibers: unknown[] = [root];
  while (fibers.length > 0) {
    const fiber = fibers.pop();
    if (typeof fiber !== 'object' || fiber === null) continue;
    const field = (name: string): unknown => Reflect.get(fiber, name);
    if (holds(field('memoizedState'), 0) || holds(field('memoizedProps'), 0)) return true;
    fibers.push(field('child'), field('sibling'));
  }
  return false;
}
