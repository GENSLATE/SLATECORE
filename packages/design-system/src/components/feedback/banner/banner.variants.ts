import { tv } from '../../../utils/cn.util';

export const bannerVariants = tv({
  slots: {
    root: 'flex items-start gap-2.5 text-base text-fg',
    icon: 'mt-px shrink-0',
    body: 'flex min-w-0 flex-1 flex-col gap-0.5 py-px',
    title: 'font-semibold text-fg-strong',
    description: 'text-fg-secondary',
    actions: 'flex shrink-0 items-center gap-1.5 self-center',
    dismiss: '-my-1 -mr-1 shrink-0',
  },
  variants: {
    tone: {
      neutral: { root: 'bg-fill-hover', icon: 'text-fg-muted' },
      info: { root: 'bg-info-subtle', icon: 'text-info-fg' },
      success: { root: 'bg-success-subtle', icon: 'text-success-fg' },
      warning: { root: 'bg-warning-subtle', icon: 'text-warning-fg' },
      danger: { root: 'bg-danger-subtle', icon: 'text-danger-fg' },
    },
    variant: {
      inline: { root: 'rounded-card border px-3 py-2.5' },
      bar: { root: 'hairline-b px-3 py-1.5' },
    },
  },
  compoundVariants: [
    {
      variant: 'inline',
      tone: 'neutral',
      class: { root: 'border-border-subtle' },
    },
    {
      variant: 'inline',
      tone: 'info',
      class: { root: 'border-info-border' },
    },
    {
      variant: 'inline',
      tone: 'success',
      class: { root: 'border-success-border' },
    },
    {
      variant: 'inline',
      tone: 'warning',
      class: { root: 'border-warning-border' },
    },
    {
      variant: 'inline',
      tone: 'danger',
      class: { root: 'border-danger-border' },
    },
  ],
  defaultVariants: { tone: 'info', variant: 'inline' },
});
