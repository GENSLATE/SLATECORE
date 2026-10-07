import { tv } from '../../../utils/cn.util';

export const colorSwatchVariants = tv({
  slots: {
    root: 'flex min-w-0',
    chip: [
      'relative shrink-0 overflow-hidden',
      // A 1px border sits above the fill so colours equal to the canvas still read as a chip.
      'after:pointer-events-none after:absolute after:inset-0 after:rounded-[inherit] after:border after:border-border after:content-[""]',
      // A transparency checkerboard shows through translucent colours: the one allowed pattern,
      // exempted by name in tests/unit/styles/styles.test.ts.
      'bg-[conic-gradient(var(--gs-color-fill-pressed)_25%,transparent_0_50%,var(--gs-color-fill-pressed)_0_75%,transparent_0)] bg-size-[8px_8px]',
    ],
    fill: 'absolute inset-0',
    caption: 'flex min-w-0 flex-col',
    name: 'truncate-flex font-medium text-fg text-sm',
    value: 'truncate-flex select-text font-mono text-2xs text-fg-muted',
  },
  variants: {
    layout: {
      tile: { root: 'flex-col gap-2', chip: 'h-14 w-full rounded-control' },
      row: { root: 'items-center gap-2.5', chip: 'size-7 rounded-sm' },
    },
  },
  defaultVariants: { layout: 'tile' },
});
