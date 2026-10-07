import { tv } from '../utils/cn.util';

/**
 * Floating surfaces (menus, popovers, selects, the command palette): one solid Nord step above
 * the window, a 1px border and the soft `popover` shadow — the only shadow in the system. They
 * fade and scale in from .96 with a 4px rise from the Base UI `--transform-origin`, and leave
 * faster than they arrive.
 */
export const popupSurface = tv({
  base: [
    'relative z-popover origin-(--transform-origin) text-fg outline-none',
    'border border-border bg-surface-popover shadow-popover',
    'transition-[opacity,scale,translate] duration-base ease-emphasized',
    'data-starting-style:scale-96 data-starting-style:opacity-0',
    'data-ending-style:scale-96 data-ending-style:opacity-0 data-ending-style:duration-fast data-ending-style:ease-standard',
    'data-[side=bottom]:data-starting-style:-translate-y-1 data-[side=top]:data-starting-style:translate-y-1',
    'data-[side=left]:data-starting-style:translate-x-1 data-[side=right]:data-starting-style:-translate-x-1',
  ],
  variants: {
    size: {
      menu: 'min-w-44 rounded-popover p-1',
      popover: 'rounded-popover p-3',
      dialog: 'rounded-dialog',
    },
    /**
     * Kept for API compatibility with the SlateSuite components. SLATECORE surfaces are always
     * solid, so it changes nothing.
     */
    glass: {
      true: '',
      false: '',
    },
  },
  defaultVariants: { size: 'menu', glass: false },
});
