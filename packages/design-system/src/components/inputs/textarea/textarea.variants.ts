import { tv } from '../../../utils/cn.util';

export const textareaVariants = tv({
  base: [
    // No outline reset: the field recipe's focus-ring-within draws the Frost edge on the textarea.
    'scrollbar-thin block w-full placeholder:text-fg-muted',
    'data-disabled:cursor-not-allowed',
  ],
  variants: {
    size: {
      sm: 'px-2 py-1',
      md: 'px-2.5 py-1.5',
      lg: 'px-3 py-2',
    },
    autoGrow: {
      true: 'resize-none overflow-y-auto',
      false: 'resize-y',
    },
  },
  defaultVariants: { size: 'md', autoGrow: false },
});
