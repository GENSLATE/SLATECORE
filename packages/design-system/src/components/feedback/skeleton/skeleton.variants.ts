import { tv } from '../../../utils/cn.util';

/** Loading placeholders: flat fill blocks that breathe softly while content loads. */
export const skeletonVariants = tv({
  base: ['relative block shrink-0 overflow-hidden bg-fill-pressed'],
  variants: {
    shape: {
      text: 'my-1 h-2.5 w-full rounded-full',
      rect: 'rounded-control',
      circle: 'rounded-full',
    },
    animated: {
      true: 'animate-pulse-soft motion-reduce:animate-none',
    },
  },
  defaultVariants: { shape: 'rect', animated: true },
});
