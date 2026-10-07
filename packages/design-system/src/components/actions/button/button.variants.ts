import { tv } from '../../../utils/cn.util';

/**
 * Flat push buttons at desktop density. Every variant carries a 1px border (transparent where
 * the fill is enough) so all of them share one box. `secondary` is the default: a control fill
 * framed by a border. Hover lifts one colour step, press settles one step deeper.
 */
export const buttonVariants = tv({
  slots: {
    root: [
      'group/button relative inline-flex shrink-0 cursor-interactive select-none items-center justify-center whitespace-nowrap border border-transparent font-medium',
      'focus-ring transition-[background-color,border-color,color,opacity] duration-fast ease-standard',
      'data-disabled:cursor-not-allowed data-disabled:opacity-45',
    ],
    content: 'inline-flex min-w-0 items-center justify-center gap-[inherit]',
    spinner: 'absolute inset-0 m-auto',
  },
  variants: {
    variant: {
      primary: {
        root: [
          'bg-accent text-on-accent',
          'not-data-disabled:hover:bg-accent-hover not-data-disabled:active:bg-accent-active',
          'window-inactive:border-border window-inactive:bg-control window-inactive:text-fg',
        ],
      },
      secondary: {
        root: [
          'border-border bg-control text-fg',
          'not-data-disabled:hover:border-border-strong not-data-disabled:hover:bg-control-hover',
          'not-data-disabled:active:bg-control-pressed',
        ],
      },
      ghost: {
        root: [
          'bg-transparent text-fg',
          'not-data-disabled:hover:bg-fill-hover not-data-disabled:active:bg-fill-pressed',
        ],
      },
      danger: {
        root: [
          'bg-danger text-on-danger',
          'not-data-disabled:hover:bg-danger-hover not-data-disabled:active:bg-danger',
        ],
      },
      link: {
        root: [
          'h-auto! border-0 bg-transparent px-0! font-normal text-accent-fg',
          'not-data-disabled:hover:underline not-data-disabled:hover:underline-offset-2',
          'not-data-disabled:active:text-accent-active',
        ],
      },
    },
    size: {
      xs: { root: 'h-control-xs gap-1 rounded-sm px-1.5 text-xs' },
      sm: { root: 'h-control-sm gap-1 rounded-control px-2 text-sm' },
      md: { root: 'h-control-md gap-1.5 rounded-control px-3 text-base' },
      lg: { root: 'h-control-lg gap-2 rounded-control px-4 text-md' },
    },
    fullWidth: {
      true: { root: 'w-full' },
    },
    loading: {
      true: {
        root: 'cursor-progress data-disabled:cursor-progress',
        content: 'invisible',
      },
    },
    iconOnly: {
      true: {},
    },
  },
  compoundVariants: [
    { size: 'xs', iconOnly: true, class: { root: 'w-control-xs px-0' } },
    { size: 'sm', iconOnly: true, class: { root: 'w-control-sm px-0' } },
    { size: 'md', iconOnly: true, class: { root: 'w-control-md px-0' } },
    { size: 'lg', iconOnly: true, class: { root: 'w-control-lg px-0' } },
  ],
  defaultVariants: {
    variant: 'secondary',
    size: 'md',
    fullWidth: false,
    loading: false,
    iconOnly: false,
  },
});
