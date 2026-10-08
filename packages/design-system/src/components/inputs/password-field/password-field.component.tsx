import { type MouseEvent, useRef, useState } from 'react';
import { Icon } from '../../display/icon';
import { TextField } from '../text-field/text-field.component';
import type { PasswordFieldProps } from './password-field.types';
import { passwordFieldToggleVariants } from './password-field.variants';

/**
 * A TextField for secrets: the value stays masked until the user reveals it with the trailing
 * eye toggle. Browsers never autofill, suggest or spell-check it.
 */
export function PasswordField({
  value,
  onChange,
  label,
  error,
  revealable = true,
  labels,
  disabled,
  ref,
  ...props
}: PasswordFieldProps) {
  const [revealed, setRevealed] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const shown = revealable && revealed;

  const setRefs = (node: HTMLInputElement | null) => {
    inputRef.current = node;
    if (typeof ref === 'function') ref(node);
    else if (ref) ref.current = node;
  };

  const toggle = (event: MouseEvent<HTMLButtonElement>) => {
    setRevealed((current) => !current);
    // A pointer click keeps typing in the field; a keyboard press leaves focus on the toggle.
    if (event.detail > 0) inputRef.current?.focus();
  };

  const trailing = revealable ? (
    <button
      type="button"
      data-slot="password-field-toggle"
      aria-label={labels?.show ?? 'Show password'}
      aria-pressed={shown}
      disabled={disabled}
      className={passwordFieldToggleVariants()}
      onClick={toggle}
    >
      <Icon name={shown ? 'codicon:eye-closed' : 'codicon:eye'} size={14} />
    </button>
  ) : undefined;

  return (
    <TextField
      {...props}
      ref={setRefs}
      label={label}
      error={error}
      disabled={disabled}
      value={value}
      onValueChange={onChange}
      type={shown ? 'text' : 'password'}
      autoComplete="off"
      spellCheck={false}
      autoCapitalize="off"
      autoCorrect="off"
      trailing={trailing}
    />
  );
}
