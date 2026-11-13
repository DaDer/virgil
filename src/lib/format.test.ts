import { describe, expect, test } from 'vitest';
import { displayPath, formatCount, formatDuration, formatElapsed, formatRelative, middleTruncate, modelFamily, projectName, shortModel, taskSummary } from './format';

describe('format', () => {
  test('counts', () => {
    expect(formatCount(950)).toBe('950');
    expect(formatCount(41_200)).toBe('41k');
    expect(formatCount(375_000)).toBe('375k');
    expect(formatCount(5_240_000)).toBe('5.2M');
    expect(formatCount(7_900_000)).toBe('7.9M');
    expect(formatCount(12_300_000)).toBe('12M');
    expect(formatCount(1500)).toBe('1.5k');
    expect(formatCount(null)).toBe('–');
    expect(formatCount(NaN)).toBe('–');
  });

  test('durations', () => {
    expect(formatDuration(42_000)).toBe('42s');
    expect(formatDuration(17 * 60_000 + 20_000)).toBe('17m 20s');
    expect(formatDuration(9 * 3_600_000 + 6 * 60_000)).toBe('9h 06m');
    expect(formatDuration(420, { allowMs: true })).toBe('420 ms');
    expect(formatDuration(null)).toBe('–');
    expect(formatElapsed(3600_000 + 12 * 60_000 + 40_000)).toBe('01:12:40');
  });

  test('relative time', () => {
    const now = 10_000_000;
    expect(formatRelative(now - 3000, now)).toBe('just now');
    expect(formatRelative(now - 35_000, now)).toBe('35s ago');
    expect(formatRelative(now - 12 * 60_000, now)).toBe('12m ago');
    expect(formatRelative(now - 3 * 3_600_000, now)).toBe('3h ago');
  });

  test('models', () => {
    expect(shortModel('claude-opus-5-5')).toBe('Opus 5.5');
    expect(shortModel('claude-sonnet-5-5[1m]')).toBe('Sonnet 5.5');
    expect(shortModel('claude-haiku-4-5-20251001')).toBe('Haiku 4.5');
    expect(shortModel('claude-fable-5-2')).toBe('Fable 5.2');
    expect(shortModel('gpt-x')).toBe('gpt-x');
    expect(shortModel(null)).toBe('Unknown model');
    expect(modelFamily('claude-opus-5-5')).toBe('opus');
    expect(modelFamily(null)).toBe('other');
  });

  test('paths', () => {
    expect(displayPath('/home/dev/p/src/a.ts', '/home/dev/p')).toBe('src/a.ts');
    expect(displayPath('/Users/me/x/y.ts')).toBe('~/x/y.ts');
    expect(displayPath('C:\\Users\\me\\x.ts')).toBe('~/x.ts');
    expect(middleTruncate('src/lib/components/deep/folder/Card.svelte', 28)).toBe('src/…/folder/Card.svelte');
    expect(middleTruncate('short.ts', 28)).toBe('short.ts');
    expect(projectName('/Users/me/code/shop')).toBe('code/shop');
    expect(taskSummary('\n  First line  \nsecond')).toBe('First line');
    expect(taskSummary('x'.repeat(200)).length).toBe(120);
  });
});
