import {
  Badge,
  Button,
  FeatureTeaser,
  Icon,
  Switch,
  useReducedMotion,
} from '@genslate/design-system';
import { MOTION } from '@genslate/tokens';
import { type CSSProperties, useState } from 'react';
import { Specimen } from '../../components/specimen.component';

const EASINGS = [
  { name: 'ease-standard', key: 'standard', use: 'State changes: quick start, soft landing' },
  {
    name: 'ease-emphasized',
    key: 'emphasized',
    use: 'Deliberate entrances: popups, dialogs, thumbs',
  },
  { name: 'ease-decelerate', key: 'decelerate', use: 'Elements settling into place' },
] as const;

const DURATIONS = [
  { name: 'duration-instant', key: 'instant' },
  { name: 'duration-fast', key: 'fast' },
  { name: 'duration-base', key: 'base' },
  { name: 'duration-slow', key: 'slow' },
] as const;

const ROWS = [
  { icon: 'codicon:file-code', name: 'index.html' },
  { icon: 'codicon:file-code', name: 'main.tsx' },
  { icon: 'codicon:file-code', name: 'app.component.tsx' },
  { icon: 'codicon:symbol-misc', name: 'package.json' },
  { icon: 'codicon:markdown', name: 'README.md' },
  { icon: 'codicon:folder', name: 'src' },
] as const;

/** What `prefers-reduced-motion: reduce` does to an animation utility, applied by the switch. */
const calm = (reduced: boolean): CSSProperties | undefined =>
  reduced ? { animation: 'none' } : undefined;

function Track({
  label,
  meta,
  moved,
  reduced,
  easing,
  duration,
}: {
  label: string;
  meta: string;
  moved: boolean;
  reduced: boolean;
  easing: string;
  duration: string;
}) {
  return (
    <div className="grid grid-cols-[11rem_1fr_16rem] items-center gap-4">
      <span className="whitespace-nowrap font-mono text-accent-fg text-code">{label}</span>
      <div className="@container relative h-6 rounded-full border border-border-subtle bg-surface-sunken">
        <span
          className="absolute top-[3px] left-[3px] size-4 rounded-full bg-accent"
          style={{
            transform: moved ? 'translateX(calc(100cqw - 1.625rem))' : 'translateX(0)',
            transitionProperty: 'transform',
            transitionTimingFunction: `var(--gs-ease-${easing})`,
            transitionDuration: reduced ? '0.01ms' : `var(--gs-duration-${duration})`,
          }}
        />
      </div>
      <span className="truncate text-fg-muted text-sm">{meta}</span>
    </div>
  );
}

function Replay({ onClick }: { onClick: () => void }) {
  return (
    <Button size="sm" leadingIcon="codicon:debug-restart" onClick={onClick}>
      Replay
    </Button>
  );
}

export function MotionSection() {
  const [forced, setForced] = useState(false);
  const [moved, setMoved] = useState(false);
  const [runs, setRuns] = useState({ fade: 0, rows: 0, pop: 0 });
  const system = useReducedMotion();
  const reduced = forced || system;
  const replay = (key: keyof typeof runs) => setRuns((r) => ({ ...r, [key]: r[key] + 1 }));

  return (
    <div data-motion-stage data-reduced={reduced ? 'true' : 'false'} className="contents">
      <Specimen
        title="Reduced motion"
        description={
          system
            ? 'Your system already asks for reduced motion: every animation below is off.'
            : 'Your system allows motion. Flip the switch to preview what reduced motion does.'
        }
        aside={
          <Switch
            size="sm"
            labelPosition="end"
            label="Reduce motion"
            checked={reduced}
            disabled={system}
            onCheckedChange={setForced}
          />
        }
        stageClassName="flex-col items-start gap-1.5 text-base text-fg-secondary"
      >
        <p>
          With <code className="text-accent-fg">prefers-reduced-motion: reduce</code> every{' '}
          <code className="text-accent-fg">motion-*</code> utility drops its animation, and every
          duration token collapses to <code className="text-accent-fg">0.01ms</code> (not 0, so
          transition and animation end events still fire).
        </p>
        <p>
          The switch applies the same rule to the demos on this page. System setting:{' '}
          <Badge size="sm" tone={system ? 'accent' : 'neutral'}>
            {system ? 'reduce' : 'no preference'}
          </Badge>
        </p>
      </Specimen>

      <Specimen
        title="Easing"
        description="Transform and opacity only. Three curves cover every transition."
        aside={
          <Button size="sm" leadingIcon="codicon:play" onClick={() => setMoved(!moved)}>
            Play
          </Button>
        }
        stageClassName="flex-col items-stretch gap-3"
      >
        {EASINGS.map((easing) => (
          <Track
            key={easing.name}
            label={easing.name}
            meta={`cubic-bezier(${MOTION.easing[easing.key].points.join(', ')})`}
            moved={moved}
            reduced={reduced}
            easing={easing.key}
            duration="slow"
          />
        ))}
        <p className="text-fg-muted text-sm">
          {EASINGS.map((easing) => `${easing.name}: ${easing.use}`).join(' · ')}
        </p>
      </Specimen>

      <Specimen
        title="Duration"
        description="Short and calm: nothing in the interface takes longer than 360ms."
        aside={
          <Button size="sm" leadingIcon="codicon:play" onClick={() => setMoved(!moved)}>
            Play
          </Button>
        }
        stageClassName="flex-col items-stretch gap-3"
      >
        {DURATIONS.map((duration) => (
          <Track
            key={duration.name}
            label={duration.name}
            meta={`${MOTION.duration[duration.key]}ms`}
            moved={moved}
            reduced={reduced}
            easing="standard"
            duration={duration.key}
          />
        ))}
      </Specimen>

      <Specimen
        title="motion-fade-up"
        description="Content entering a view: it fades in while rising 4px. Used on every page of this Kit."
        aside={<Replay onClick={() => replay('fade')} />}
        code={'<section className="motion-fade-up">…</section>'}
      >
        <div
          key={runs.fade}
          data-utility="motion-fade-up"
          style={calm(reduced)}
          className="motion-fade-up flex w-full max-w-md flex-col gap-1 rounded-card border border-border bg-surface-raised p-4"
        >
          <span className="font-semibold text-fg-strong text-md">Project overview</span>
          <span className="text-base text-fg-muted">
            The block arrives as one piece: opacity 0 to 1 and a 4px rise, over the slow duration on
            the emphasized curve.
          </span>
        </div>
      </Specimen>

      <Specimen
        title="motion-row-in"
        description="List rows: a short 2px rise, staggered by --stagger. Rows past the twelfth enter together."
        aside={<Replay onClick={() => replay('rows')} />}
        code={
          '{rows.map((row, index) => (\n  <li className="motion-row-in" style={{ \'--stagger\': index }}>…</li>\n))}'
        }
        stageClassName="flex-col items-stretch gap-0 p-0"
      >
        <ul key={runs.rows} className="m-0 flex list-none flex-col p-0">
          {ROWS.map((row, index) => (
            <li
              key={row.name}
              data-utility="motion-row-in"
              style={{ '--stagger': index, ...calm(reduced) } as CSSProperties}
              className="motion-row-in not-first:hairline-t flex h-row-md items-center gap-2 px-6 text-base text-fg"
            >
              <Icon name={row.icon} size={16} className="text-fg-secondary" />
              {row.name}
              <span className="ml-auto text-2xs text-fg-muted tabular-nums">
                --stagger: {index}
              </span>
            </li>
          ))}
        </ul>
      </Specimen>

      <Specimen
        title="motion-pop"
        description="Launch and confirm feedback: a quick scale pop to 94% and back."
        aside={<Replay onClick={() => replay('pop')} />}
        code={'<button className="motion-pop">Launched</button>'}
      >
        <div
          key={runs.pop}
          data-utility="motion-pop"
          style={calm(reduced)}
          className="motion-pop inline-flex h-control-lg items-center gap-2 rounded-control border border-success-border bg-success-subtle px-4 font-medium text-md text-success-fg"
        >
          <Icon name="codicon:check" size={16} />
          Launched
        </div>
      </Specimen>

      <Specimen
        title="motion-pulse-soft"
        description="Coming soon teasers: a slow, calm opacity breath that never exceeds a 55% dip."
        code={'<Badge tone="accent" pill className="motion-pulse-soft">Coming soon</Badge>'}
        stageClassName="flex-col items-stretch gap-4"
      >
        <div className="flex flex-wrap items-center gap-3">
          <Badge
            tone="accent"
            pill
            data-utility="motion-pulse-soft"
            style={calm(reduced)}
            className="motion-pulse-soft"
          >
            Coming soon
          </Badge>
          <Badge
            tone="info"
            variant="outline"
            dot
            pill
            style={calm(reduced)}
            className="motion-pulse-soft"
          >
            Syncing
          </Badge>
        </div>
        <div className="max-w-md overflow-hidden rounded-card border border-border-subtle bg-surface-sidebar [[data-reduced=true]_&_*]:animate-none">
          <FeatureTeaser icon="codicon:sparkle" title="Ask SLATECORE">
            FeatureTeaser combines motion-fade-up on the card with motion-pulse-soft on its badge.
          </FeatureTeaser>
        </div>
      </Specimen>
    </div>
  );
}
