import { tv } from '../../../utils/cn.util';

/**
 * Segmented control: a sunken track framed by a 1px border, and one raised thumb (a colour
 * step up with its own border) that slides between equal segments (transform only).
 */
export const segmentedControlVariants = tv({
  slots: {
    root: [
      'relative isolate inline-grid auto-cols-fr grid-flow-col items-stretch rounded-control border border-border-subtle bg-surface-sunken p-0.5',
      'data-disabled:opacity-45',
    ],
    thumb: [
      'pointer-events-none absolute inset-y-0.5 left-0.5 -z-10 w-[calc((100%-4px)/var(--segment-count))] rounded-xs',
      'translate-x-[calc(var(--segment-index)*100%)] border border-border bg-surface-raised',
      'transition-transform duration-base ease-emphasized motion-reduce:transition-none',
    ],
    item: [
      'relative inline-flex min-w-0 cursor-interactive select-none items-center justify-center gap-1.5 whitespace-nowrap rounded-xs px-3 font-medium text-fg-secondary',
      'focus-ring-inset transition-colors duration-fast ease-standard',
      'not-data-disabled:hover:text-fg-strong not-data-disabled:active:text-fg data-pressed:text-fg-strong',
      'data-disabled:cursor-not-allowed data-disabled:text-fg-disabled',
    ],
  },
  variants: {
    size: {
      sm: { root: 'h-control-sm', item: 'px-2 text-sm' },
      md: { root: 'h-control-md', item: 'text-base' },
      lg: { root: 'h-control-lg', item: 'px-4 text-md' },
    },
    fullWidth: {
      true: { root: 'grid w-full' },
    },
  },
  defaultVariants: { size: 'md', fullWidth: false },
});
