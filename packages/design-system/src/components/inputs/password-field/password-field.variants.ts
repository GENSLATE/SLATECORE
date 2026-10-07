import { tv } from '../../../utils/cn.util';

/** The show/hide toggle at the trailing edge of a PasswordField. */
export const passwordFieldToggleVariants = tv({
  base: [
    'focus-ring -mr-1 flex size-5 shrink-0 cursor-interactive items-center justify-center rounded-sm text-fg-muted',
    'transition-colors duration-fast ease-standard',
    'hover:bg-fill-hover hover:text-fg active:bg-fill-pressed',
    'aria-pressed:text-accent-fg',
    'disabled:pointer-events-none disabled:text-fg-disabled',
  ],
});
