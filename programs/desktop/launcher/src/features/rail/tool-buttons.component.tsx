import { IconButton } from '@genslate/design-system';

import { TOOLS, type ToolId } from '../tools/tools.model';

export interface ToolButtonsProps {
  /** The open tool, if any. */
  readonly active: ToolId | undefined;
  /** Shortcut that toggles Settings (shown in its tooltip). */
  readonly settingsShortcut: string;
  readonly onToggle: (tool: ToolId) => void;
}

/**
 * The tool-button row at the bottom of the rail, level with the command bar: Settings first,
 * then the tools still to come (they open "Coming soon" teasers). The open tool's button is
 * pressed; pressing it again closes the tool.
 */
export function ToolButtons({ active, settingsShortcut, onToggle }: ToolButtonsProps) {
  return (
    <div
      role="toolbar"
      aria-label="Tools"
      data-slot="tool-buttons"
      className="flex h-full items-center justify-between px-2"
    >
      {TOOLS.map((tool) => (
        <IconButton
          key={tool.id}
          size="sm"
          label={tool.label}
          icon={tool.icon}
          toggled={active === tool.id}
          tooltipShortcut={tool.id === 'settings' ? settingsShortcut : undefined}
          onClick={() => onToggle(tool.id)}
        />
      ))}
    </div>
  );
}
