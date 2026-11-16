import { describe, expect, it } from 'vitest';
import { buildScenario, scenarioAt, SESSION_BILLING, SESSION_DOCS, SESSION_ORCHARD } from '../data/scenario';
import { deriveDemoAttention, groupAttention, visibleBackground, type AttentionBackground, type AttentionItem, type AttentionKind } from './attention.svelte';

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
  background: [],
});

const command = (taskId: string): AttentionBackground => ({ taskId, summary: taskId, agentId: 'main', agentLabel: null, startedAt: 0 });

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

describe('demo background commands', () => {
  const base = 1_800_000_000_000;
  const at = (rel: number) => deriveDemoAttention(scenarioAt(buildScenario(base), base + rel), base + rel).find((i) => i.sessionId === SESSION_ORCHARD)!;

  it('lists the running ones, oldest first, with the agent that started them', () => {
    const i = at(120_000);
    expect(i.background.map((b) => [b.taskId, b.summary, b.agentLabel === null])).toEqual([
      ['b7dev2k4q', 'Start the dev server', true],
      ['b5api0t9r', 'Run the API tests in watch mode', false],
    ]);
    expect(i.backgroundTasks).toBe(2);
    expect(i.background[1].agentLabel).toBeTruthy();
  });

  it('drops a command once it has ended', () => {
    const ids = at(200_000).background.map((b) => b.taskId);
    expect(ids).toContain('b7dev2k4q');
    expect(ids).not.toContain('b5api0t9r');
    expect(ids).not.toContain('b3idx8m1w');
  });
});

describe('visibleBackground', () => {
  it('shows the first few and counts the rest', () => {
    const i = { ...item('a', 'working'), background: ['b1', 'b2', 'b3', 'b4', 'b5'].map(command) };
    const v = visibleBackground(i);
    expect(v.shown.map((b) => b.taskId)).toEqual(['b1', 'b2', 'b3']);
    expect(v.more).toBe(2);
    expect(visibleBackground({ ...i, background: i.background.slice(0, 3) }).more).toBe(0);
    expect(visibleBackground(item('a', 'working'))).toEqual({ shown: [], more: 0 });
  });
});
