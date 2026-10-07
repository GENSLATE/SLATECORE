import { tv } from '../utils/cn.util';

/**
 * Text-entry chrome shared by TextField, PasswordField, SearchField, Textarea, Select triggers
 * and NumberField: a flat field one colour step below its surface, framed by a 1px border that
 * strengthens on hover and becomes a crisp 2px Frost edge on focus.
 */
export const field = tv({
  base: [
    'flex w-full min-w-0 items-center gap-1.5 rounded-control border border-border bg-field text-fg',
    'transition-[border-color,outline-color,background-color] duration-fast ease-standard',
    'not-data-disabled:hover:border-border-strong',
    'focus-ring-within',
    'has-[[aria-invalid=true]]:border-danger-border data-invalid:border-danger-border',
    'not-data-disabled:data-invalid:hover:border-danger',
    'data-disabled:cursor-not-allowed data-disabled:bg-surface-sunken data-disabled:text-fg-disabled',
  ],
  variants: {
    size: {
      sm: 'h-control-sm px-2 text-sm',
      md: 'h-control-md px-2.5 text-base',
      lg: 'h-control-lg px-3 text-md',
    },
    multiline: {
      true: 'h-auto items-start py-1.5',
    },
  },
  defaultVariants: { size: 'md' },
});

/** The bare `<input>` inside a `field`. */
export const fieldInput = tv({
  base: [
    'h-full min-w-0 flex-1 bg-transparent text-inherit outline-none',
    'placeholder:text-fg-muted disabled:cursor-not-allowed disabled:placeholder:text-fg-disabled',
  ],
});
