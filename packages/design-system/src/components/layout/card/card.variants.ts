import { tv } from '../../../utils/cn.util';

export const cardVariants = tv({
  slots: {
    root: 'flex min-w-0 flex-col rounded-card text-fg',
    header: 'flex items-start gap-3',
    headerText: 'flex min-w-0 flex-1 flex-col gap-0.5',
    title: 'font-semibold text-fg-strong text-md',
    description: 'text-base text-fg-muted',
    actions: 'flex shrink-0 items-center gap-1',
    body: 'min-w-0',
    footer: 'flex items-center justify-end gap-2',
  },
  variants: {
    // Flat cards: a colour step from the surface beneath plus a 1px border, never a shadow.
    variant: {
      outline: { root: 'border border-border bg-surface-raised/40' },
      raised: { root: 'border border-border-subtle bg-surface-raised' },
      sunken: { root: 'border border-border-subtle bg-surface-sunken' },
    },
    padding: {
      none: {},
      sm: { root: 'gap-2 p-3' },
      md: { root: 'gap-3 p-4' },
      lg: { root: 'gap-4 p-6' },
    },
    interactive: {
      true: {
        root: [
          'focus-ring cursor-interactive transition-colors duration-fast ease-standard',
          'hover:border-border-strong hover:bg-fill-hover active:bg-fill-pressed',
          'aria-disabled:pointer-events-none aria-disabled:opacity-50 data-disabled:pointer-events-none data-disabled:opacity-50',
        ],
      },
    },
  },
  defaultVariants: { variant: 'outline', padding: 'md', interactive: false },
});
