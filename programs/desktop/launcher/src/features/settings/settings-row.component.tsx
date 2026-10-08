import { cn } from '@genslate/design-system';
import type { ReactNode } from 'react';

export interface SettingsGroupProps {
  /** Small uppercase caption above the group. */
  readonly title?: string | undefined;
  readonly children: ReactNode;
  readonly className?: string | undefined;
}

/** A flat card of settings rows, separated by 1px dividers. */
export function SettingsGroup({ title, children, className }: SettingsGroupProps) {
  return (
    <div className={cn('flex flex-col gap-1.5', className)}>
      {title === undefined ? null : (
        <h3 className="px-1 font-semibold text-2xs text-fg-muted uppercase tracking-wider">
          {title}
        </h3>
      )}
      <div className="flex flex-col divide-y divide-border-subtle rounded-card border border-border-subtle bg-surface-raised">
        {children}
      </div>
    </div>
  );
}

export interface SettingsRowProps {
  readonly label: ReactNode;
  readonly description?: ReactNode | undefined;
  /** The control on the right. */
  readonly control?: ReactNode | undefined;
  /** Id of the label, for controls named by it (`aria-labelledby`). */
  readonly labelId?: string | undefined;
  readonly className?: string | undefined;
}

/** One setting: label and description on the left, its control on the right. */
export function SettingsRow({ label, description, control, labelId, className }: SettingsRowProps) {
  return (
    <div className={cn('flex min-h-12 items-center gap-4 px-3.5 py-2.5', className)}>
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span id={labelId} className="font-medium text-fg-strong text-sm">
          {label}
        </span>
        {description === undefined ? null : (
          <span className="text-fg-muted text-xs leading-relaxed">{description}</span>
        )}
      </div>
      {control === undefined ? null : <div className="shrink-0">{control}</div>}
    </div>
  );
}
