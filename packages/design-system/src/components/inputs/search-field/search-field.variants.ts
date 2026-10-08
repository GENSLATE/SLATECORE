import { tv } from '../../../utils/cn.util';

/**
 * The `⌘F`-style hint at the trailing edge of an empty search field. It shares the control's flex
 * row with the input (never layered over it) and keeps its own width. `-mr-1` tucks it into the
 * control padding, aligned with the clear button, so a narrow field leaves more room to the input.
 */
export const searchFieldHintVariants = tv({
  base: [
    'pointer-events-none -mr-1 whitespace-nowrap rounded-xs border border-border-subtle px-1 font-sans text-fg-muted text-xs tabular-nums',
    'transition-opacity duration-fast ease-standard group-focus-within/field:opacity-0',
  ],
});

/**
 * The search `<input>`: hides the native search chrome (the field has its own clear button) and
 * ends a placeholder or value that does not fit in an ellipsis instead of cutting a glyph in half.
 */
export const searchFieldInputVariants = tv({
  base: [
    'text-ellipsis',
    '[&::-webkit-search-cancel-button]:appearance-none [&::-webkit-search-decoration]:appearance-none',
  ],
});
