import { tv } from '../../../utils/cn.util';

export const avatarVariants = tv({
  slots: {
    root: 'relative inline-flex shrink-0 select-none items-center justify-center align-middle',
    // Opaque base so stacked avatars never show through; a 1px border sits above image and fallback.
    clip: [
      'relative flex size-full items-center justify-center overflow-hidden bg-canvas',
      'after:pointer-events-none after:absolute after:inset-0 after:rounded-[inherit] after:border after:border-border-subtle after:content-[""]',
    ],
    image: 'size-full object-cover',
    fallback:
      'flex size-full items-center justify-center bg-accent-subtle font-semibold text-accent-fg uppercase',
    // A canvas-coloured outline cuts the dot out of the avatar edge.
    status: 'absolute right-0 bottom-0 rounded-full outline-2 outline-canvas',
  },
  variants: {
    size: {
      xs: { root: 'size-4', fallback: 'text-[8px]', status: 'size-1.5 outline-1' },
      sm: { root: 'size-5', fallback: 'text-[9px]', status: 'size-1.5 outline-1' },
      md: { root: 'size-6', fallback: 'text-2xs', status: 'size-2' },
      lg: { root: 'size-8', fallback: 'text-xs', status: 'size-2.5' },
      xl: { root: 'size-10', fallback: 'text-sm', status: 'size-3' },
    },
    shape: {
      circle: { clip: 'rounded-full' },
      square: { clip: 'rounded-sm' },
    },
    status: {
      online: { status: 'bg-success' },
      away: { status: 'bg-warning' },
      busy: { status: 'bg-danger' },
      offline: { status: 'bg-fg-disabled' },
    },
  },
  defaultVariants: { size: 'md', shape: 'circle' },
});
