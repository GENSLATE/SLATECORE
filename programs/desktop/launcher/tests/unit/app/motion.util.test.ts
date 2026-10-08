import { afterEach, describe, expect, test } from 'bun:test';

import { COLLAPSE_FALLBACK_MS, collapseDelayMs, durationMs } from '../../../src/app/motion.util';

afterEach(() => document.documentElement.style.removeProperty('--gs-duration-slow'));

describe('motion timing', () => {
  test('durationMs reads CSS time values', () => {
    expect(durationMs('360ms')).toBe(360);
    expect(durationMs(' 0.36s ')).toBe(360);
    expect(durationMs('0.01ms')).toBe(0.01);
    expect(durationMs('')).toBeNull();
    expect(durationMs('slow')).toBeNull();
    expect(durationMs('-5ms')).toBeNull();
  });

  test('the collapse waits for --gs-duration-slow, with a fallback when it is not set', () => {
    expect(collapseDelayMs(false)).toBe(COLLAPSE_FALLBACK_MS);
    document.documentElement.style.setProperty('--gs-duration-slow', '200ms');
    expect(collapseDelayMs(false)).toBe(200);
  });

  test('reduced motion collapses at once', () => {
    document.documentElement.style.setProperty('--gs-duration-slow', '360ms');
    expect(collapseDelayMs(true)).toBe(0);
  });
});
