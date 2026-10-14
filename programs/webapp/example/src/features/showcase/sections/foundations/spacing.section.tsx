import { LAYOUT_SIZE, RADIUS, SPACE } from '@genslate/tokens';
import { Specimen } from '../../components/specimen.component';

const SURFACES = [
  { name: 'canvas', className: 'bg-canvas', use: 'The window background' },
  { name: 'surface-sidebar', className: 'bg-surface-sidebar', use: 'Sidebar and inspector panes' },
  { name: 'surface-panel', className: 'bg-surface-panel', use: 'Panels and editors' },
  { name: 'surface-raised', className: 'bg-surface-raised', use: 'Cards and raised thumbs' },
  { name: 'surface-sunken', className: 'bg-surface-sunken', use: 'Wells, tracks, code' },
  { name: 'surface-popover', className: 'bg-surface-popover', use: 'Menus and popovers' },
  { name: 'surface-dialog', className: 'bg-surface-dialog', use: 'Dialogs and the palette' },
] as const;

const BORDERS = [
  { name: 'border-subtle', className: 'border-border-subtle', use: 'Hairlines, cards at rest' },
  { name: 'border', className: 'border-border', use: 'Controls and field frames' },
  { name: 'border-strong', className: 'border-border-strong', use: 'Hover and emphasis' },
] as const;

const RADIUS_CLASS: Readonly<Record<string, string>> = {
  none: 'rounded-none',
  xs: 'rounded-xs',
  sm: 'rounded-sm',
  md: 'rounded-md',
  lg: 'rounded-lg',
  xl: 'rounded-xl',
  '2xl': 'rounded-2xl',
  full: 'rounded-full',
};

const SIZE_GROUPS = [
  { title: 'Chrome', keys: ['titlebar', 'statusbar', 'tabbar', 'toolbar', 'panel-header'] },
  {
    title: 'Controls & rows',
    keys: [
      'control-xs',
      'control-sm',
      'control-md',
      'control-lg',
      'control-xl',
      'row-sm',
      'row-md',
      'menu-item',
    ],
  },
  {
    title: 'Panes',
    keys: ['sidebar', 'sidebar-min', 'sidebar-max', 'command-center-max', 'palette', 'content-max'],
  },
] as const;

export function SpacingSection() {
  const spaces = Object.entries(SPACE)
    .filter(([key]) => key !== '0')
    .sort(([, a], [, b]) => a - b);
  return (
    <>
      <Specimen
        title="Spacing"
        description="Tailwind's 4px grid: p-2 is 8px. Half steps exist for optical tweaks."
        stageClassName="flex-col items-stretch gap-2"
      >
        {spaces.map(([key, px]) => (
          <div key={key} className="grid grid-cols-[4rem_3rem_1fr] items-center gap-3">
            <span className="font-mono text-accent-fg text-code">{key}</span>
            <span className="text-fg-muted text-sm tabular-nums">{px}px</span>
            <span className="h-3 rounded-xs bg-accent/70" style={{ width: px * 4 }} />
          </div>
        ))}
      </Specimen>

      <Specimen
        title="Radius"
        description="Controls 6 · menu items 4 · popovers and cards 8 · windows 10 · dialogs 12."
        stageClassName="grid grid-cols-[repeat(auto-fill,minmax(6.5rem,1fr))] gap-5"
      >
        {Object.entries(RADIUS.scale).map(([key, px]) => (
          <div key={key} className="flex flex-col items-center gap-2">
            <div
              className={`size-16 border border-accent-border bg-accent-subtle ${RADIUS_CLASS[key] ?? ''}`}
            />
            <span className="font-mono text-code text-fg">rounded-{key}</span>
            <span className="text-fg-muted text-xs tabular-nums">
              {px === 9999 ? 'pill' : `${px}px`}
            </span>
          </div>
        ))}
      </Specimen>

      <Specimen
        title="Elevation"
        description="Flat at rest: depth is a colour step plus a 1px border. Only floating popups carry a shadow."
        stageClassName="grid grid-cols-[repeat(auto-fill,minmax(10rem,1fr))] gap-4 bg-canvas p-6"
      >
        {SURFACES.map((surface) => (
          <div
            key={surface.name}
            className={`flex h-24 flex-col justify-end gap-0.5 rounded-card border border-border-subtle p-3 ${surface.className}`}
          >
            <span className="font-mono text-code text-fg">{surface.name}</span>
            <span className="text-fg-muted text-xs">{surface.use}</span>
          </div>
        ))}
        {BORDERS.map((border) => (
          <div
            key={border.name}
            className={`flex h-24 flex-col justify-end gap-0.5 rounded-card border bg-canvas p-3 ${border.className}`}
          >
            <span className="font-mono text-code text-fg">{border.name}</span>
            <span className="text-fg-muted text-xs">{border.use}</span>
          </div>
        ))}
        <div className="flex h-24 flex-col justify-end gap-0.5 rounded-popover border border-border bg-surface-popover p-3 shadow-popover">
          <span className="font-mono text-code text-fg">shadow-popover</span>
          <span className="text-fg-muted text-xs">
            The one shadow: popups, dialogs, toasts, tooltips
          </span>
        </div>
      </Specimen>

      <Specimen
        title="Layout sizes"
        description="Fixed sizes of desktop chrome: titlebar 38, status bar 24, controls 20 to 36."
        bare
      >
        <div className="grid grid-cols-3 gap-4">
          {SIZE_GROUPS.map((group) => (
            <div
              key={group.title}
              className="flex flex-col rounded-card border border-border-subtle py-2"
            >
              <span className="px-4 pt-1 pb-2 font-semibold text-2xs text-fg-muted uppercase tracking-wider">
                {group.title}
              </span>
              {group.keys.map((key) => (
                <div
                  key={key}
                  className="flex items-center justify-between gap-2 px-4 py-1.5 text-sm"
                >
                  <span className="font-mono text-code text-fg">{key}</span>
                  <span className="text-fg-muted tabular-nums">{LAYOUT_SIZE[key]}px</span>
                </div>
              ))}
            </div>
          ))}
        </div>
      </Specimen>
    </>
  );
}
