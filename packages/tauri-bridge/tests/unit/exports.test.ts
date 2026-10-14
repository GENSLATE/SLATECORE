import { describe, expect, test } from 'bun:test';

import * as bridge from '../../src';

const FUNCTIONS = [
  'invokeCommand',
  'listenEvent',
  'isTauri',
  'isCommandError',
  'customSchemeUrl',
  'useWindowControls',
  'useSystemTheme',
  'setNativeTheme',
  'detectPlatform',
  'useAppInfo',
  'createChannel',
  'channelBytes',
  'invokeBytes',
] as const;

describe('public surface', () => {
  test('bridge_exports_the_documented_surface', () => {
    for (const name of FUNCTIONS) {
      expect(typeof bridge[name]).toBe('function');
    }
    expect(bridge.UNAVAILABLE).toMatchObject({ kind: 'unavailable' });
    expect(typeof bridge.commands.appInfo).toBe('function');
    expect(typeof bridge.commands.openExternal).toBe('function');
  });

  test('isCommandError() accepts the UNAVAILABLE rejection', () => {
    expect(bridge.isCommandError(bridge.UNAVAILABLE)).toBe(true);
    expect(bridge.isCommandError('nope')).toBe(false);
  });
});
