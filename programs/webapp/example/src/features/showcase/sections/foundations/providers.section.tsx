import {
  Badge,
  type CursorStyle,
  SegmentedControl,
  SegmentedControlItem,
  type ThemePreference,
  useCursorStyle,
  usePlatform,
  useReducedMotion,
  useTheme,
  useWindowState,
} from '@genslate/design-system';
import type { ReactNode } from 'react';
import { PropsTable } from '../../components/props-table.component';
import { Specimen } from '../../components/specimen.component';

const STACK = `<DesignSystemProvider platform={platform} windowState={windowState}>
  <TooltipProvider>
    <ToastProvider>
      {children}
      <ToastViewport />
    </ToastProvider>
  </TooltipProvider>
</DesignSystemProvider>`;

const PROVIDERS = [
  {
    name: 'DesignSystemProvider',
    description:
      'Composes the four providers below and sets the text direction. Apps mount this once.',
  },
  {
    name: 'ThemeProvider',
    description: 'Applies Polar Night or Snow Storm to <html data-theme> and persists the choice.',
  },
  {
    name: 'PlatformProvider',
    description: 'Tells components which OS they run on, so shortcuts read ⌘K or Ctrl+K.',
  },
  {
    name: 'WindowStateProvider',
    description: 'Feeds focus, maximize and fullscreen state to the window chrome.',
  },
  {
    name: 'CursorProvider',
    description: 'Switches between the themed Nord cursors and the operating system’s.',
  },
  {
    name: 'TooltipProvider',
    description: 'Groups tooltips so moving between triggers skips the open delay.',
  },
  {
    name: 'ToastProvider',
    description: 'Holds the toast queue behind useToast(); ToastViewport renders it.',
  },
] as const;

function Value({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1 rounded-card border border-border-subtle bg-surface-raised p-3">
      <span className="font-medium text-2xs text-fg-muted uppercase tracking-wider">{label}</span>
      <span className="select-text font-mono text-code text-fg">{children}</span>
    </div>
  );
}

const yesNo = (value: boolean) => (
  <Badge size="sm" tone={value ? 'success' : 'neutral'} dot>
    {value ? 'true' : 'false'}
  </Badge>
);

export function ProvidersSection() {
  const { theme, resolvedTheme, scheme, setTheme } = useTheme();
  const { cursorStyle, setCursorStyle } = useCursorStyle();
  const platform = usePlatform();
  const { isFocused, isMaximized, isFullscreen } = useWindowState();
  const reducedMotion = useReducedMotion();

  return (
    <>
      <Specimen
        title="Provider stack"
        description="One provider tree gives every component its theme, platform, window state and cursor."
        stageClassName="flex-col items-stretch gap-0 p-0"
        code={STACK}
      >
        <ul className="m-0 flex list-none flex-col p-0">
          {PROVIDERS.map((provider) => (
            <li
              key={provider.name}
              className="not-first:hairline-t grid grid-cols-[13rem_1fr] items-baseline gap-4 px-6 py-2.5"
            >
              <span className="font-mono text-accent-fg text-code">{provider.name}</span>
              <span className="text-base text-fg-secondary">{provider.description}</span>
            </li>
          ))}
        </ul>
      </Specimen>

      <Specimen
        title="Live values"
        description="What the hooks report right now. Change the theme or cursor and watch the values follow."
        stageClassName="flex-col items-stretch gap-4"
      >
        <div className="flex flex-wrap items-center gap-3">
          <SegmentedControl<ThemePreference>
            aria-label="Theme preference"
            size="sm"
            value={theme}
            onValueChange={setTheme}
          >
            <SegmentedControlItem value="polar-night">Polar Night</SegmentedControlItem>
            <SegmentedControlItem value="snow-storm">Snow Storm</SegmentedControlItem>
            <SegmentedControlItem value="system">System</SegmentedControlItem>
          </SegmentedControl>
          <SegmentedControl<CursorStyle>
            aria-label="Cursor style"
            size="sm"
            value={cursorStyle}
            onValueChange={setCursorStyle}
          >
            <SegmentedControlItem value="themed">SLATECORE</SegmentedControlItem>
            <SegmentedControlItem value="system">System</SegmentedControlItem>
          </SegmentedControl>
        </div>
        <div className="grid w-full grid-cols-[repeat(auto-fill,minmax(10rem,1fr))] gap-3">
          <Value label="theme">{theme}</Value>
          <Value label="resolvedTheme">{resolvedTheme}</Value>
          <Value label="scheme">{scheme}</Value>
          <Value label="platform">{platform}</Value>
          <Value label="cursorStyle">{cursorStyle}</Value>
          <Value label="useReducedMotion()">{yesNo(reducedMotion)}</Value>
          <Value label="isFocused">{yesNo(isFocused)}</Value>
          <Value label="isMaximized">{yesNo(isMaximized)}</Value>
          <Value label="isFullscreen">{yesNo(isFullscreen)}</Value>
        </div>
      </Specimen>

      <PropsTable
        caption="Hooks"
        rows={[
          {
            name: 'useTheme()',
            type: '{ theme, resolvedTheme, scheme, setTheme, toggleTheme }',
            description: 'The theme preference, what it resolves to, and its setters.',
          },
          {
            name: 'usePlatform()',
            type: "'macos' | 'windows' | 'linux' | 'web'",
            description: 'The platform from PlatformProvider, else a user-agent guess.',
          },
          {
            name: 'useWindowState()',
            type: '{ isFocused, isMaximized, isFullscreen }',
            description: 'Native window state; focused and restored outside a provider.',
          },
          {
            name: 'useCursorStyle()',
            type: "{ cursorStyle: 'themed' | 'system', setCursorStyle }",
            description: 'Reads and changes the cursor family.',
          },
          {
            name: 'useReducedMotion()',
            type: 'boolean',
            description: 'True when the OS asks for reduced motion.',
          },
          {
            name: 'useMediaQuery(query)',
            type: 'boolean',
            description: 'Tracks any CSS media query.',
          },
          {
            name: 'useHotkey(shortcut, handler)',
            type: 'void',
            description: '"mod+k" style global shortcuts; mod is ⌘ on macOS and Ctrl elsewhere.',
          },
          {
            name: 'useToast()',
            type: '{ add, close, … }',
            description: 'Queues toasts from anywhere under ToastProvider.',
          },
        ]}
      />
    </>
  );
}
