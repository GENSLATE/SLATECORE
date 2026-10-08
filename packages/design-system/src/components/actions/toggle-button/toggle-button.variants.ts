import { tv } from '../../../utils/cn.util';

export const toggleButtonVariants = tv({
  base: [
    'relative inline-flex shrink-0 cursor-interactive select-none items-center justify-center gap-1.5 whitespace-nowrap border border-transparent font-medium text-fg-secondary',
    'focus-ring transition-[background-color,border-color,color,opacity] duration-fast ease-standard',
    'not-data-disabled:hover:text-fg-strong',
    'data-pressed:border-accent-border data-pressed:bg-accent-subtle data-pressed:text-accent-fg',
    'not-data-disabled:data-pressed:hover:text-accent-fg',
    'window-inactive:data-pressed:border-border window-inactive:data-pressed:bg-fill-selected-inactive window-inactive:data-pressed:text-fg',
    'data-disabled:cursor-not-allowed data-disabled:opacity-40',
  ],
  variants: {
    variant: {
      ghost:
        'bg-transparent not-data-disabled:not-data-pressed:hover:bg-fill-hover not-data-disabled:active:bg-fill-pressed',
      secondary: [
        'border-border bg-control',
        'not-data-disabled:not-data-pressed:hover:border-border-strong not-data-disabled:not-data-pressed:hover:bg-control-hover',
        'not-data-disabled:active:bg-control-pressed',
      ],
    },
    size: {
      xs: 'h-control-xs rounded-sm px-[calc(--spacing(1.5)-1px)] text-xs',
      sm: 'h-control-sm rounded-control px-[calc(--spacing(2)-1px)] text-sm',
      md: 'h-control-md rounded-control px-[calc(--spacing(2.5)-1px)] text-base',
      lg: 'h-control-lg rounded-control px-[calc(--spacing(3)-1px)] text-md',
    },
    iconOnly: { true: 'px-0' },
  },
  compoundVariants: [
    { iconOnly: true, size: 'xs', class: 'w-control-xs' },
    { iconOnly: true, size: 'sm', class: 'w-control-sm' },
    { iconOnly: true, size: 'md', class: 'w-control-md' },
    { iconOnly: true, size: 'lg', class: 'w-control-lg' },
  ],
  defaultVariants: { variant: 'ghost', size: 'md', iconOnly: false },
});

export const toggleGroupVariants = tv({
  base: 'inline-flex items-center gap-0.5 data-[orientation=vertical]:flex-col',
});
