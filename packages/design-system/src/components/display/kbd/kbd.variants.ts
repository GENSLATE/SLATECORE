import { tv } from '../../../utils/cn.util';

export const kbdVariants = tv({
  slots: {
    root: 'inline-flex shrink-0 items-center whitespace-nowrap font-sans text-fg-muted',
    key: [
      // A flat keycap: one control step with a 1px border.
      'inline-flex items-center justify-center rounded-xs border border-border bg-control font-medium font-sans text-fg-secondary',
    ],
    separator: 'text-fg-disabled',
  },
  variants: {
    variant: {
      keycap: { root: 'gap-0.5' },
      inline: { root: 'tabular-nums tracking-wide', key: '' },
    },
    size: {
      sm: { root: 'text-2xs', key: 'h-4 min-w-4 px-[calc(--spacing(1)-1px)] text-2xs' },
      md: { root: 'text-xs', key: 'h-5 min-w-5 px-[calc(--spacing(1.5)-1px)] text-xs' },
    },
  },
  defaultVariants: { variant: 'keycap', size: 'md' },
});
