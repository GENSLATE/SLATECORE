import { IconButton, ScrollArea } from '@genslate/design-system';
import type { ReactNode } from 'react';

export interface PanelSheetProps {
  readonly title: string;
  readonly onBack: () => void;
  readonly children: ReactNode;
}

/** A sub-view of the apps panel (Help, Properties, Run with arguments) with a back header. */
export function PanelSheet({ title, onBack, children }: PanelSheetProps) {
  return (
    <section
      aria-label={title}
      data-slot="panel-sheet"
      className="motion-fade-up flex h-full flex-col"
    >
      <header className="hairline-b flex h-10 shrink-0 items-center gap-1 px-1.5">
        <IconButton
          size="sm"
          label="Back"
          tooltipShortcut="escape"
          icon="codicon:arrow-left"
          onClick={onBack}
        />
        <h2 className="truncate font-semibold text-fg-strong text-sm">{title}</h2>
      </header>
      <ScrollArea className="min-h-0 flex-1" viewportClassName="p-3" scrollShadow>
        {children}
      </ScrollArea>
    </section>
  );
}
