import type { ReactNode } from 'react';
import type { TextFieldProps } from '../text-field/text-field.types';

export interface PasswordFieldLabels {
  /**
   * Accessible name of the reveal toggle (default "Show password"). It stays the same in both
   * states; `aria-pressed` tells whether the password is shown.
   */
  show?: string | undefined;
}

/**
 * Controlled: `value` and `onChange` together. Uncontrolled: neither, and the secret stays in the
 * `<input>` only (read it through `ref` on submit, then clear it).
 */
export type PasswordFieldValueProps =
  | {
      /** The secret, held by the caller. */
      value: string;
      /** Called with the new string on every edit. */
      onChange: (value: string) => void;
    }
  | { value?: undefined; onChange?: undefined };

export type PasswordFieldProps = Omit<
  TextFieldProps,
  | 'value'
  | 'defaultValue'
  | 'onValueChange'
  | 'onChange'
  | 'label'
  | 'error'
  | 'type'
  | 'autoComplete'
  | 'spellCheck'
  | 'clearable'
  | 'onClear'
  | 'trailing'
  | 'labels'
> &
  PasswordFieldValueProps & {
    /** Visible label, also the input's accessible name. */
    label: ReactNode;
    /** Error message; marks the input invalid while set. */
    error?: ReactNode | undefined;
    /** Shows the show/hide toggle (default true). */
    revealable?: boolean | undefined;
    labels?: PasswordFieldLabels | undefined;
  };
