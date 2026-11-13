import { describe, expect, it } from 'vitest';
import { groupAttention, type AttentionItem, type AttentionKind } from './attention.svelte';

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
