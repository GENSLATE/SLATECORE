import { tv } from '../../../utils/cn.util';

/**
 * Trigger: the field chrome as a button, value left, chevrons right. Keyboard focus gets the
 * field's crisp Frost edge (`focus-ring-within` matches `:focus-visible`); an open list keeps
 * the border Frost.
 */
export const selectTriggerVariants = tv({
  base: [
    'group/select cursor-interactive select-none justify-between text-left',
    'focus-visible:border-focus',
    'not-data-disabled:active:bg-fill-pressed',
    'data-popup-open:border-accent',
    'data-invalid:border-danger-border',
    'data-disabled:cursor-not-allowed',
  ],
});

export const selectValueVariants = tv({
  base: 'min-w-0 flex-1 truncate data-placeholder:text-fg-muted',
});

export const selectIconVariants = tv({
  base: [
    'flex shrink-0 items-center text-fg-muted transition-colors duration-fast ease-standard',
    'group-hover/select:text-fg',
  ],
});

export const selectPositionerVariants = tv({
  base: 'z-popover outline-none',
});

export const selectPopupVariants = tv({
  base: 'min-w-(--anchor-width) data-[side=none]:min-w-[calc(var(--anchor-width)+1rem)] data-[side=none]:scale-100',
});

export const selectListVariants = tv({
  base: 'scrollbar-thin max-h-(--available-height) scroll-py-4 overflow-y-auto overscroll-contain outline-none',
});

export const selectScrollArrowVariants = tv({
  base: [
    'absolute inset-x-0 z-raised flex h-4 cursor-default items-center justify-center bg-surface-popover text-fg-muted',
    'data-[direction=up]:top-0 data-[direction=up]:rounded-t-popover',
    'data-[direction=down]:bottom-0 data-[direction=down]:rounded-b-popover',
  ],
});

export const selectItemIndicatorVariants = tv({
  base: 'absolute left-1.5 flex size-4 items-center justify-center',
});

export const selectGroupLabelVariants = tv({
  base: 'flex h-6 select-none items-center pr-2 pl-7 font-semibold text-fg-muted text-xs',
});

export const selectSeparatorVariants = tv({
  base: 'mx-2 my-1 h-px bg-border-subtle',
});
