import { Input } from '@base-ui/react/input';
import { type MouseEvent, type PointerEvent, type ReactNode, useRef, useState } from 'react';
import { field, fieldInput } from '../../../recipes';
import { cn } from '../../../utils/cn.util';
import { Icon } from '../../display/icon';
import { FieldFrame } from '../field/field-frame.component';
import { TextField } from '../text-field/text-field.component';
import { textFieldAdornmentVariants } from '../text-field/text-field.variants';
import type { PasswordFieldProps } from './password-field.types';
import { passwordFieldToggleVariants } from './password-field.variants';

/**
 * A TextField for secrets: the value stays masked until the user reveals it with the trailing
 * eye toggle. Browsers never autofill, suggest or spell-check it.
 *
 * Controlled (`value` + `onChange`) or uncontrolled: leave both out and the secret lives only in
 * the `<input>` — no React state holds it, not even the field's own validity state. Read it
 * through `ref` when the form is submitted and clear it straight after (`input.value = ''`).
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

  const secretProps = {
    type: shown ? 'text' : 'password',
    autoComplete: 'off',
    spellCheck: false,
    autoCapitalize: 'off',
    autoCorrect: 'off',
  } as const;

  if (value === undefined) {
    return (
      <UncontrolledPassword
        {...props}
        {...secretProps}
        inputRef={setRefs}
        focusInput={() => inputRef.current?.focus()}
        label={label}
        error={error}
        disabled={disabled}
        trailing={trailing}
      />
    );
  }

  return (
    <TextField
      {...props}
      {...secretProps}
      ref={setRefs}
      label={label}
      error={error}
      disabled={disabled}
      value={value}
      onValueChange={onChange}
      trailing={trailing}
    />
  );
}

type UncontrolledPasswordProps = Omit<
  PasswordFieldProps,
  'value' | 'onChange' | 'revealable' | 'labels' | 'ref'
> & {
  readonly type: 'text' | 'password';
  readonly inputRef: (node: HTMLInputElement | null) => void;
  readonly focusInput: () => void;
  readonly trailing: ReactNode;
};

/**
 * The same chrome as TextField around an uncontrolled Base UI input. Base UI's change and Enter
 * handlers are skipped: they would copy the value into the field's validity state.
 */
function UncontrolledPassword({
  label,
  description,
  error,
  invalid,
  size = 'md',
  leading,
  trailing,
  disabled,
  name,
  className,
  controlClassName,
  inputClassName,
  inputRef,
  focusInput,
  onKeyDown,
  ...inputProps
}: UncontrolledPasswordProps) {
  const isInvalid = invalid ?? error != null;

  // Clicking the chrome around the input (padding, icons) focuses the input, like AppKit.
  const focusFromChrome = (event: PointerEvent<HTMLDivElement>) => {
    const target = event.target;
    if (
      target === event.currentTarget ||
      !(target instanceof Element && target.closest('button,input'))
    ) {
      event.preventDefault();
      focusInput();
    }
  };

  return (
    <FieldFrame
      label={label}
      description={description}
      error={error}
      invalid={isInvalid}
      disabled={disabled}
      name={name}
      className={className}
    >
      <div
        data-slot="text-field-control"
        data-size={size}
        data-invalid={isInvalid ? '' : undefined}
        data-disabled={disabled ? '' : undefined}
        className={cn(field({ size }), 'group/field', controlClassName)}
        onPointerDown={focusFromChrome}
      >
        {leading != null && (
          <span data-slot="text-field-leading" className={textFieldAdornmentVariants()}>
            {leading}
          </span>
        )}
        <Input
          data-slot="text-field-input"
          {...inputProps}
          ref={inputRef}
          disabled={disabled}
          className={cn(fieldInput(), inputClassName)}
          onChange={(event) => event.preventBaseUIHandler()}
          onKeyDown={(event) => {
            if (event.key === 'Enter') event.preventBaseUIHandler();
            onKeyDown?.(event);
          }}
        />
        {trailing != null && (
          <span data-slot="text-field-trailing" className={textFieldAdornmentVariants()}>
            {trailing}
          </span>
        )}
      </div>
    </FieldFrame>
  );
}
