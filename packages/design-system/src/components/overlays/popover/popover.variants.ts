import { tv } from '../../../utils/cn.util';

export const popoverVariants = tv({
  slots: {
    positioner: 'z-popover',
    popup: 'min-w-48 max-w-(--available-width)',
    // A diamond centred on the popup's 1px border, bordered on its two outer edges only.
    arrow: [
      'size-2.5 rotate-45 border-border bg-surface-popover',
      'data-[side=bottom]:-top-1.5 data-[side=left]:-right-1.5 data-[side=top]:-bottom-1.5 data-[side=right]:-left-1.5',
      'data-[side=bottom]:border-t data-[side=top]:border-r data-[side=top]:border-b data-[side=bottom]:border-l',
      'data-[side=left]:border-t data-[side=left]:border-r data-[side=right]:border-b data-[side=right]:border-l',
    ],
    title: 'pr-6 font-semibold text-base text-fg-strong',
    description: 'mt-1 text-fg-secondary text-sm',
    close: [
      'focus-ring absolute top-2 right-2 flex size-5 items-center justify-center rounded-sm text-fg-muted',
      'transition-colors duration-fast ease-standard hover:bg-fill-hover hover:text-fg active:bg-fill-pressed',
    ],
  },
});
