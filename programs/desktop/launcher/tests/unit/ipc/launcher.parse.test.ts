import { describe, expect, test } from 'bun:test';

import { createMockBackend, MOCK_VAULT_PASSWORD } from '../../../src/ipc/launcher.mock';
import {
  isVaultError,
  PayloadError,
  parseActionOutcome,
  parseAppList,
  parseContext,
  parseEntries,
  parseLockReport,
  parseSettings,
  parseTrayMenuAnchor,
  parseVaultOpened,
  parseVaultStatus,
  parseVerifyReport,
  parseVolume,
} from '../../../src/ipc/launcher.parse';

type Path = readonly (string | number)[];

/** A JSON round trip: what actually crosses the IPC boundary. */
const json = (value: unknown): unknown => JSON.parse(JSON.stringify(value));

/** Every leaf (primitive or null) and every container in `value`, by path. */
function paths(value: unknown, prefix: Path = []): Path[] {
  if (Array.isArray(value)) {
    return [prefix, ...value.flatMap((item, index) => paths(item, [...prefix, index]))];
  }
  if (typeof value === 'object' && value !== null) {
    return [
      prefix,
      ...Object.entries(value).flatMap(([key, item]) => paths(item, [...prefix, key])),
    ];
  }
  return [prefix];
}

/** A value of the wrong kind for whatever sits at `current`. */
function wrong(current: unknown): unknown {
  if (typeof current === 'string') return 42;
  if (typeof current === 'number') return 'x';
  if (typeof current === 'boolean') return 'yes';
  if (current === null) return {};
  return 'x';
}

function at(value: unknown, path: Path): unknown {
  return path.reduce<unknown>(
    (node, key) => (typeof node === 'object' && node !== null ? Reflect.get(node, key) : undefined),
    value,
  );
}

/** A copy of `value` with the node at `path` replaced (or removed, for `undefined`). */
function edit(value: unknown, path: Path, next: unknown): unknown {
  const copy = json(value);
  const parent = at(copy, path.slice(0, -1));
  const key = path.at(-1);
  if (typeof parent !== 'object' || parent === null || key === undefined) return next;
  if (next === undefined) Reflect.deleteProperty(parent, key);
  else Reflect.set(parent, key, next);
  return copy;
}

/** Each field of `valid`, given the wrong kind of value or left out, is rejected. */
function expectEveryFieldChecked(parse: (raw: unknown) => unknown, valid: unknown) {
  expect(parse(json(valid))).toEqual(json(valid));
  for (const path of paths(valid)) {
    const label = path.join('.') || '(root)';
    expect(() => parse(edit(valid, path, wrong(at(valid, path)))), label).toThrow(PayloadError);
    const key = path.at(-1);
    if (typeof key === 'string') {
      expect(() => parse(edit(valid, path, undefined)), `${label} missing`).toThrow(PayloadError);
    }
  }
}

describe('launcher.parse', () => {
  test('app lists: every field of every app and tab is checked', async () => {
    const list = await createMockBackend().listApps();
    const sample = { ...list, apps: list.apps.slice(0, 3), recent: list.recent.slice(0, 2) };
    expectEveryFieldChecked(parseAppList, sample);
  });

  test('app status, tab source and recent ids must be known values', async () => {
    const list = json(await createMockBackend().listApps());
    expect(() => parseAppList(edit(list, ['apps', 0, 'status'], 'exploded'))).toThrow(
      'apps[0].status',
    );
    expect(() => parseAppList(edit(list, ['tabs', 0, 'source'], 'steam'))).toThrow(
      'tabs[0].source',
    );
    expect(() => parseAppList(edit(list, ['recent', 0], 7))).toThrow('recent apps[0]');
    expect(() => parseAppList(edit(list, ['apps', 0, 'favorite'], 1))).toThrow('apps[0].favorite');
  });

  test('parsed payloads drop fields the launcher does not know', async () => {
    const list = json(await createMockBackend().listApps());
    const parsed = parseAppList(edit(list, ['apps', 0, 'token'], 'leak'));
    expect(parsed.apps[0]).not.toHaveProperty('token');
  });

  test('context: settings, every action and its params, layout', async () => {
    const context = await createMockBackend({ issue: 'line 3' }).context();
    expectEveryFieldChecked(parseContext, context);
    const raw = json(context);
    expect(() => parseContext(edit(raw, ['mode'], 'standalone'))).toThrow('context.mode');
    expect(() => parseContext(edit(raw, ['actions', 0, 'effect'], 'magic'))).toThrow(
      'context.actions[0].effect',
    );
    expect(() => parseContext(edit(raw, ['actions', 1, 'params', 0, 'type'], 'number'))).toThrow(
      'context.actions[1].params[0].type',
    );
    expect(() =>
      parseContext(edit(raw, ['layout', 'normalWidth'], Number.POSITIVE_INFINITY)),
    ).toThrow('context.layout.normalWidth');
  });

  test('settings: every key and its type, unions included', async () => {
    const { settings } = await createMockBackend().context();
    expectEveryFieldChecked(parseSettings, settings);
    const raw = json(settings);
    expect(() => parseSettings(edit(raw, ['config', 'appearance', 'theme'], 'dark'))).toThrow(
      'settings.config.appearance.theme',
    );
    expect(() => parseSettings(edit(raw, ['config', 'status', 'mode'], 'fans'))).toThrow(
      'settings.config.status.mode',
    );
  });

  test('vault status is built field by field, locked and unlocked', async () => {
    const backend = createMockBackend();
    expectEveryFieldChecked(parseVaultStatus, await backend.vaultStatus());
    expectEveryFieldChecked(parseVaultStatus, await backend.vaultUnlock(MOCK_VAULT_PASSWORD));
    const raw = json(await backend.vaultStatus());
    expect(() => parseVaultStatus(edit(raw, ['state'], 'open'))).toThrow('vault status.state');
    expect(() => parseVaultStatus(edit(raw, ['failedAttempts'], Number.NaN))).toThrow(
      'vault status.failedAttempts',
    );
  });

  test('vault entries, lock and verify reports, opened files', async () => {
    const backend = createMockBackend({ vault: 'unlocked' });
    const entries = await backend.vaultList('');
    expectEveryFieldChecked(parseEntries, entries);
    expect(() => parseEntries(edit(entries, [0, 'kind'], 'link'))).toThrow('vault entries[0].kind');
    expectEveryFieldChecked(parseLockReport, {
      synced: ['ideas.md'],
      unsynced: ['budget-2026.xlsx'],
      wipedFiles: 2,
      undeletable: ['Photos/aurora-01.jpg'],
    });
    const report = {
      filesChecked: 6,
      orphanBlobs: 1,
      problems: [{ path: 'ideas.md', problem: 'tampered' }],
    };
    expectEveryFieldChecked(parseVerifyReport, report);
    expect(() => parseVerifyReport(edit(report, ['problems', 0, 'problem'], 'gone'))).toThrow(
      'verify report.problems[0].problem',
    );
    expectEveryFieldChecked(parseVaultOpened, await backend.vaultOpen('ideas.md'));
  });

  test('action outcomes: done with an optional message, or a UI action', () => {
    expectEveryFieldChecked(parseActionOutcome, { kind: 'done', message: 'Rescanned' });
    expectEveryFieldChecked(parseActionOutcome, { kind: 'done', message: null });
    expectEveryFieldChecked(parseActionOutcome, { kind: 'ui', id: 'settings' });
    expect(() => parseActionOutcome({ kind: 'later' })).toThrow('action outcome.kind');
    expect(() => parseActionOutcome({ kind: 'ui' })).toThrow('action outcome.id');
  });

  test('volume (or none) and the tray menu anchor', async () => {
    expectEveryFieldChecked(parseVolume, await createMockBackend().volume());
    expect(parseVolume(null)).toBeNull();
    expectEveryFieldChecked(parseTrayMenuAnchor, {
      x: 540,
      y: 420,
      opensUp: true,
      alignEnd: false,
    });
  });

  test('vault errors need a known code and well-typed extras', () => {
    expect(isVaultError({ code: 'WRONG_PASSWORD', message: 'Wrong password.' })).toBe(true);
    expect(isVaultError({ code: 'THROTTLED', message: 'Wait.', retryAfterMs: 30_000 })).toBe(true);
    expect(isVaultError({ code: 'UNSYNCED_EDITS', message: 'x', paths: ['ideas.md'] })).toBe(true);
    expect(isVaultError({ code: 'SOMETHING_NEW', message: 'x' })).toBe(false);
    expect(isVaultError({ code: 'THROTTLED', message: 'x', retryAfterMs: 'soon' })).toBe(false);
    expect(isVaultError({ code: 'UNSYNCED_EDITS', message: 'x', paths: [1] })).toBe(false);
    expect(isVaultError({ code: 'IO' })).toBe(false);
    expect(isVaultError('IO')).toBe(false);
  });
});
