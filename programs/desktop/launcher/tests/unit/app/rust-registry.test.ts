/**
 * The real command registry is Rust (`crates/launcher-core/src/actions.rs`); the browser mock
 * copies it. These tests read the Rust source itself, so a renamed id or parameter there fails
 * here instead of turning a slash command into a silent no-op.
 */
import { describe, expect, test } from 'bun:test';

import {
  COMMAND_IDS,
  type CommandHost,
  runCommand,
} from '../../../src/app/launcher-commands.model';
import { createMockBackend } from '../../../src/ipc/launcher.mock';
import { app } from '../fixtures';
import { spyBackend } from '../launcher.harness';

interface RustParam {
  readonly name: string;
  readonly kind: 'app' | 'choice' | 'text';
  readonly values: readonly string[];
  readonly required: boolean;
}

interface RustAction {
  readonly id: string;
  readonly params: readonly RustParam[];
  /** `Effect::<Name>` in kebab case (`ui`, `writes-config`, …), as the registry serialises it. */
  readonly effect: string;
}

/** Effects the shell's `run_action` hands back to the UI (`{ kind: 'ui' }`) instead of doing. */
const UI_EFFECTS: readonly string[] = ['ui', 'ai'];

const ACTIONS_RS = new URL(
  '../../../../../../crates/launcher-core/src/actions.rs',
  import.meta.url,
);

const quoted = (list: string) => [...list.matchAll(/"([^"]*)"/g)].map((match) => match[1] ?? '');

/** Parses `REGISTRY` out of actions.rs: ids, parameter names, kinds and choice values. */
async function rustRegistry(): Promise<RustAction[]> {
  const source = await Bun.file(ACTIONS_RS).text();
  const constants = new Map(
    [...source.matchAll(/pub const (\w+): &\[&str\] = &\[([^\]]*)\];/g)].map((match) => [
      match[1] ?? '',
      quoted(match[2] ?? ''),
    ]),
  );
  const registry = /pub const REGISTRY: &\[ActionSpec\] = &\[([\s\S]*?)\n\];/.exec(source)?.[1];
  if (registry === undefined) throw new Error('REGISTRY not found in actions.rs');
  const appParam: RustParam = { name: 'app', kind: 'app', values: [], required: true };

  return registry
    .split('ActionSpec {')
    .slice(1)
    .map((chunk) => {
      const id = /\bid: "([^"]+)"/.exec(chunk)?.[1];
      if (id === undefined) throw new Error(`action without an id: ${chunk}`);
      const variant = /effect: Effect::(\w+)/.exec(chunk)?.[1];
      if (variant === undefined) throw new Error(`action without an effect: ${id}`);
      const effect = variant.replace(/(?<=[a-z])(?=[A-Z])/g, '-').toLowerCase();
      if (/params: &\[APP\]/.test(chunk)) return { id, params: [appParam], effect };
      const params = [
        ...chunk.matchAll(
          /ParamSpec \{\s*name: "(\w+)",[\s\S]*?kind: ParamKind::(\w+)(?: \{\s*values: (?:&\[([^\]]*)\]|(\w+)),?\s*\})?,\s*required: (true|false)/g,
        ),
      ].map((match): RustParam => {
        const [, name = '', kind = '', inline, constant, required] = match;
        const values =
          inline !== undefined ? quoted(inline) : (constants.get(constant ?? '') ?? []);
        return {
          name,
          kind: kind === 'Choice' ? 'choice' : kind === 'App' ? 'app' : 'text',
          values,
          required: required === 'true',
        };
      });
      return { id, params, effect };
    });
}

/** Every way the slash bar can call `action`: each choice value, plus no optional parameter. */
function calls(action: RustAction): Record<string, string>[] {
  const required: Record<string, string> = {};
  for (const param of action.params) {
    if (!param.required) continue;
    required[param.name] =
      param.kind === 'app' ? 'Terminal' : param.kind === 'choice' ? (param.values[0] ?? '') : 'x';
  }
  const variants = action.params.flatMap((param) =>
    param.kind === 'choice'
      ? param.values.map((value) => ({ ...required, [param.name]: value }))
      : [],
  );
  return [required, ...variants];
}

function host() {
  const backend = spyBackend(createMockBackend());
  const effects: string[] = [];
  const value: CommandHost = {
    backend,
    apps: [app('genslate/terminal')],
    pinned: false,
    tool: undefined,
    launch: () => effects.push('launch'),
    show: () => effects.push('show'),
    inform: () => effects.push('inform'),
    fail: () => effects.push('fail'),
  };
  /** Names of the backend calls made so far. */
  const backendCalls = () =>
    Object.entries(backend).flatMap(([name, spy]) => (spy.mock.calls.length > 0 ? [name] : []));
  return { host: value, backend, effects, backendCalls };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('the Rust command registry', () => {
  test('is parsed from actions.rs', async () => {
    const registry = await rustRegistry();
    expect(registry.length).toBeGreaterThanOrEqual(15);
    const folder = registry.find((action) => action.id === 'folder');
    expect(folder?.params).toEqual([
      {
        name: 'name',
        kind: 'choice',
        values: ['desktop', 'documents', 'downloads', 'music', 'pictures', 'videos', 'storage'],
        required: true,
      },
    ]);
    expect(registry.find((action) => action.id === 'config')?.params).toEqual([]);
    expect(registry.find((action) => action.id === 'tab')?.effect).toBe('ui');
    expect(registry.find((action) => action.id === 'fav')?.effect).toBe('writes-config');
  });

  test('every ui and ai action runs in the launcher, never through the shell', async () => {
    // run_action only answers `{ kind: 'ui' }` for these, so a fallthrough would do nothing.
    const registry = await rustRegistry();
    const local: readonly string[] = COMMAND_IDS;
    const ui = registry.filter((action) => UI_EFFECTS.includes(action.effect));
    expect(ui.map((action) => action.id)).toEqual(
      expect.arrayContaining(['tab', 'tools', 'settings', 'vault', 'help', 'ask']),
    );
    for (const action of ui) {
      expect({ id: action.id, local: local.includes(action.id) }).toEqual({
        id: action.id,
        local: true,
      });
      for (const params of calls(action)) {
        for (const tool of [undefined, 'settings', 'manager'] as const) {
          const { host: h, backend, effects } = host();
          runCommand({ ...h, tool }, action.id, params);
          await flush();
          const what = `/${action.id} ${JSON.stringify(params)} (tool open: ${tool ?? 'none'})`;
          expect({ what, runAction: backend.runAction.mock.calls.length }).toEqual({
            what,
            runAction: 0,
          });
          // A ui action changes the view: it shows something (or says why it cannot).
          expect({ what, shown: effects.some((effect) => effect !== 'fail') }).toEqual({
            what,
            shown: true,
          });
        }
      }
    }
  });

  test('every action the launcher handles itself acts on the Rust parameter names', async () => {
    const registry = await rustRegistry();
    const local: readonly string[] = COMMAND_IDS;
    const handled = registry.filter((action) => local.includes(action.id));
    expect(handled.map((action) => action.id)).toContain('folder');
    for (const action of handled) {
      for (const params of calls(action)) {
        const { host: h, backend, effects, backendCalls } = host();
        runCommand(h, action.id, params);
        await flush();
        const what = `/${action.id} ${JSON.stringify(params)}`;
        // A local handler never hands its own id back to the shell, and never does nothing.
        expect({ what, runAction: backend.runAction.mock.calls.length }).toEqual({
          what,
          runAction: 0,
        });
        expect({ what, acted: effects.length + backendCalls().length > 0 }).toEqual({
          what,
          acted: true,
        });
        expect({ what, failed: effects.includes('fail') }).toEqual({ what, failed: false });
      }
    }
  });

  test('every other action goes to the shell with its parameters unchanged', async () => {
    const registry = await rustRegistry();
    const local: readonly string[] = COMMAND_IDS;
    for (const action of registry.filter((entry) => !local.includes(entry.id))) {
      expect({ id: action.id, effect: UI_EFFECTS.includes(action.effect) }).toEqual({
        id: action.id,
        effect: false,
      });
      const { host: h, backend } = host();
      const [params = {}] = calls(action);
      runCommand(h, action.id, params);
      expect(backend.runAction).toHaveBeenCalledWith(action.id, params);
    }
  });

  test('the browser mock lists Rust ids with the Rust parameter names', async () => {
    const registry = await rustRegistry();
    const { actions } = await createMockBackend().context();
    // The whole registry, in its order, so the slash menu in a browser is the real one.
    expect(actions.map((action) => [action.id, action.effect])).toEqual(
      registry.map((action) => [action.id, action.effect]),
    );
    for (const action of actions) {
      const rust = registry.find((entry) => entry.id === action.id);
      expect({ id: action.id, params: action.params.map((param) => param.name) }).toEqual({
        id: action.id,
        params: rust?.params.map((param) => param.name) ?? ['<not in actions.rs>'],
      });
    }
  });
});
