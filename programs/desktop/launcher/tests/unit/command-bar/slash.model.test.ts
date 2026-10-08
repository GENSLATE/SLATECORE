import { describe, expect, test } from 'bun:test';

import { paramsFor, parseSlash, runsOnChoose } from '../../../src/features/command-bar/slash.model';
import { ACTIONS, app } from '../fixtures';

const APPS = [app('genslate/explorer', { description: 'Files' }), app('genslate/editor')];
const find = (id: string) => {
  const action = ACTIONS.find((candidate) => candidate.id === id);
  if (action === undefined) throw new Error(`fixture ${id}`);
  return action;
};

describe('parseSlash', () => {
  test('plain text is not a command', () => {
    expect(parseSlash('explorer', ACTIONS, APPS).active).toBe(false);
  });

  test('"/" lists every command; a prefix narrows them', () => {
    expect(parseSlash('/', ACTIONS, APPS).suggestions).toHaveLength(ACTIONS.length);
    const narrowed = parseSlash('/th', ACTIONS, APPS).suggestions;
    expect(narrowed[0]).toMatchObject({ kind: 'action', action: { id: 'theme' } });
  });

  test('a fully typed command name ranks first', () => {
    expect(parseSlash('/vault', ACTIONS, APPS).suggestions[0]).toMatchObject({
      kind: 'action',
      action: { id: 'vault' },
    });
    expect(parseSlash('/settings', ACTIONS, APPS).suggestions[0]).toMatchObject({
      kind: 'action',
      action: { id: 'settings' },
    });
  });

  test('choice arguments suggest matching values', () => {
    const parse = parseSlash('/theme d', ACTIONS, APPS);
    expect(parse.action?.id).toBe('theme');
    expect(parse.suggestions.map((s) => (s.kind === 'value' ? s.value : s.kind))).toEqual(['dark']);
    const sections = parseSlash('/settings v', ACTIONS, APPS);
    expect(sections.suggestions.map((s) => (s.kind === 'value' ? s.value : s.kind))).toEqual([
      'vault',
    ]);
  });

  test('app arguments suggest apps by fuzzy name', () => {
    const parse = parseSlash('/open exp', ACTIONS, APPS);
    expect(parse.suggestions[0]).toMatchObject({
      kind: 'value',
      value: 'genslate/explorer',
      label: 'Explorer',
    });
  });

  test('text arguments have no suggestions', () => {
    expect(parseSlash('/ask what is new', ACTIONS, APPS)).toMatchObject({
      argument: 'what is new',
      suggestions: [],
    });
  });
});

describe('paramsFor', () => {
  test('validates required and choice arguments', () => {
    const theme = find('theme');
    expect(paramsFor(theme, ' Dark ')).toEqual({ params: { mode: 'dark' } });
    expect(paramsFor(theme, 'purple')).toEqual({ error: 'Use one of: system, dark, light' });
    expect(paramsFor(theme, '')).toEqual({ error: '/theme needs a theme' });
  });

  test('optional and parameterless actions run as they are', () => {
    expect(paramsFor(find('ask'), '')).toEqual({ params: {} });
    expect(paramsFor(find('settings'), '')).toEqual({ params: {} });
    expect(paramsFor(find('vault'), 'ignored')).toEqual({ params: {} });
  });
});

describe('runsOnChoose', () => {
  test('commands without a required argument run when picked; others complete', () => {
    expect(runsOnChoose(find('settings'))).toBe(true);
    expect(runsOnChoose(find('vault'))).toBe(true);
    expect(runsOnChoose(find('ask'))).toBe(true);
    expect(runsOnChoose(find('theme'))).toBe(false);
    expect(runsOnChoose(find('open'))).toBe(false);
  });
});
