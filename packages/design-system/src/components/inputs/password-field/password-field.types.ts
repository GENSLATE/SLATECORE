import type { ReactNode } from 'react';
import type { TextFieldProps } from '../text-field/text-field.types';

export interface PasswordFieldLabels {
  /** Accessible name of the toggle while the value is hidden. */
  show?: string | undefined;
  /** Accessible name of the toggle while the value is shown. */
  hide?: string | undefined;
}

export interface PasswordFieldProps
  extends Omit<
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
  > {
  /** The secret (always controlled: the field never keeps a copy of its own). */
  value: string;
  /** Called with the new string on every edit. */
  onChange: (value: string) => void;
  /** Visible label, also the input's accessible name. */
  label: ReactNode;
  /** Error message; marks the input invalid while set. */
  error?: ReactNode | undefined;
  /** Shows the show/hide toggle (default true). */
  revealable?: boolean | undefined;
  labels?: PasswordFieldLabels | undefined;
}
