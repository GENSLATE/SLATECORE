import { cn, Icon, IconButton, Kbd } from '@genslate/design-system';
import type { KeyboardEvent, Ref } from 'react';

import { SlashMenu, type SlashMenuProps } from './slash-menu.component';

export interface CommandBarProps {
  readonly inputRef: Ref<HTMLInputElement>;
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly onKeyDown: (event: KeyboardEvent<HTMLInputElement>) => void;
  /** The listbox the input controls (apps or slash suggestions). */
  readonly controls: string;
  readonly activeDescendant: string | undefined;
  /** Shown while a slash command is being typed. */
  readonly slash: SlashMenuProps | null;
  readonly focusShortcut: string;
  /** The AI tools teaser is open. */
  readonly aiOpen: boolean;
  readonly onAsk: () => void;
}

/**
 * The bottom band's search / slash bar: plain text searches every tab; `/` turns it into a
 * command line (suggestions float above). The sparkle is the reserved AI entry point: it opens
 * the AI tools teaser, as `/ask` does.
 */
export function CommandBar({
  inputRef,
  value,
  onChange,
  onKeyDown,
  controls,
  activeDescendant,
  slash,
  focusShortcut,
  aiOpen,
  onAsk,
}: CommandBarProps) {
  const isSlash = value.startsWith('/');
  return (
    <div
      data-slot="command-bar"
      className="ml-auto flex h-full w-full max-w-[calc(var(--launcher-normal)-var(--spacing-launcher-rail))] items-center px-2"
    >
      <div className="relative w-full">
        {slash === null ? null : <SlashMenu {...slash} />}
        <div
          className={cn(
            'flex h-control-xl items-center gap-2 rounded-control border border-border-subtle bg-field pr-1 pl-2.5',
            'focus-ring-within transition-colors duration-fast ease-standard hover:border-border',
          )}
        >
          <Icon
            name={isSlash ? 'codicon:terminal' : 'codicon:search'}
            size={14}
            className={cn(
              'shrink-0 transition-colors duration-fast ease-standard',
              isSlash ? 'text-accent-fg' : 'text-fg-muted',
            )}
          />
          <input
            ref={inputRef}
            type="text"
            role="combobox"
            aria-label="Search apps or type / for commands"
            aria-expanded={slash !== null}
            aria-controls={controls}
            aria-activedescendant={activeDescendant}
            aria-autocomplete="list"
            autoComplete="off"
            autoCorrect="off"
            spellCheck={false}
            placeholder="Search apps or type /"
            value={value}
            onChange={(event) => onChange(event.target.value)}
            onKeyDown={onKeyDown}
            className="min-w-0 flex-1 bg-transparent text-fg-strong text-sm outline-none placeholder:text-fg-muted"
          />
          {value === '' ? <Kbd shortcut={focusShortcut} size="sm" className="shrink-0" /> : null}
          <IconButton
            size="sm"
            label="Ask AI"
            tooltip="Ask AI · coming soon"
            icon="codicon:sparkle"
            toggled={aiOpen}
            onClick={onAsk}
          />
        </div>
      </div>
    </div>
  );
}
