import type { CodiconRef } from '@genslate/design-system';

/** The tools in the rail's button row; Settings is first and the only real tool in v1. */
export type ToolId = 'settings' | 'manager' | 'storage' | 'diagnostics' | 'ai';

export interface ToolInfo {
  readonly id: ToolId;
  readonly label: string;
  readonly icon: CodiconRef;
}

export const TOOLS: readonly ToolInfo[] = [
  { id: 'settings', label: 'Settings', icon: 'codicon:settings-gear' },
  { id: 'manager', label: 'App manager', icon: 'codicon:extensions' },
  { id: 'storage', label: 'Storage & backup', icon: 'codicon:database' },
  { id: 'diagnostics', label: 'Diagnostics', icon: 'codicon:pulse' },
  { id: 'ai', label: 'AI tools', icon: 'codicon:sparkle' },
];

export function toolInfo(id: ToolId): ToolInfo {
  return TOOLS.find((tool) => tool.id === id) ?? { id, label: id, icon: 'codicon:tools' };
}
