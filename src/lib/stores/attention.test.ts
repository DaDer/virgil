import { describe, expect, it } from 'vitest';
import { buildScenario, scenarioAt, SESSION_BILLING, SESSION_DOCS, SESSION_ORCHARD } from '../data/scenario';
import { deriveDemoAttention, groupAttention, type AttentionItem, type AttentionKind } from './attention.svelte';

const item = (sessionId: string, kind: AttentionKind): AttentionItem => ({
  sessionId,
  name: sessionId,
  project: null,
  title: null,
  kind,
  since: 0,
  line: '',
  runningAgents: 0,
  backgroundTasks: 0,
});

describe('groupAttention', () => {
  it('orders groups by urgency and drops idle sessions and empty groups', () => {
    const groups = groupAttention([item('a', 'working'), item('b', 'idle'), item('c', 'needsYou'), item('d', 'working')]);
    expect(groups.map((g) => [g.kind, g.items.map((i) => i.sessionId)])).toEqual([
      ['needsYou', ['c']],
      ['working', ['a', 'd']],
    ]);
    expect(groupAttention([item('b', 'idle')])).toEqual([]);
  });
});

describe('deriveDemoAttention', () => {
  it('puts the mock sessions in the expected groups and leaves the stale one out', () => {
    const base = 1_800_000_000_000;
    const items = deriveDemoAttention(scenarioAt(buildScenario(base), base + 20_000), base + 20_000, { [SESSION_BILLING]: 'A question?' });
    const kind = (id: string) => items.find((i) => i.sessionId === id)?.kind;
    expect(items).toHaveLength(4);
    expect(items[0]).toMatchObject({ sessionId: SESSION_BILLING, kind: 'needsYou', line: 'A question?' });
    expect(kind(SESSION_DOCS)).toBe('finished');
    expect(kind(SESSION_ORCHARD)).toBe('working');
  });
});
