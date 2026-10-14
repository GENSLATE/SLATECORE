import { Icon, IconButton } from '@genslate/design-system';
import type { SettingsSection } from '../settings/settings.model';
import { SettingsTool } from '../settings/settings-tool.component';
import { ToolTeaser } from './tool-teaser.component';
import { type ToolId, toolInfo } from './tools.model';

export interface ToolViewProps {
  readonly tool: ToolId;
  readonly section: SettingsSection;
  readonly onSectionChange: (section: SettingsSection) => void;
  readonly onClose: () => void;
}

/**
 * The wide view a tool opens in (the frame widens to make room): a header with the tool's name
 * and a close button, then Settings, or a "Coming soon" teaser for the tools still to come.
 */
export function ToolView({ tool, section, onSectionChange, onClose }: ToolViewProps) {
  const info = toolInfo(tool);
  return (
    <section
      aria-label={info.label}
      data-slot="tool-view"
      data-tool={tool}
      className="flex h-full flex-col"
    >
      <header className="hairline-b flex h-11 shrink-0 items-center gap-2.5 pr-2 pl-4">
        <Icon name={info.icon} size={14} className="text-accent-fg" />
        <h1 className="flex-1 truncate font-semibold text-fg-strong text-sm">{info.label}</h1>
        <IconButton
          size="sm"
          label={`Close ${info.label}`}
          icon="codicon:close"
          tooltipShortcut="escape"
          onClick={onClose}
        />
      </header>
      <div key={tool} className="motion-fade-up min-h-0 flex-1">
        {tool === 'settings' ? (
          <SettingsTool section={section} onSectionChange={onSectionChange} />
        ) : (
          <ToolTeaser tool={tool} onClose={onClose} />
        )}
      </div>
    </section>
  );
}
