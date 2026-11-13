import { describe, expect, test } from 'vitest';
import type { VirgilEvent, ToolCall } from '../../contract';
import { ToolHistory, ticksUntil, toolBreakdown } from './toolHistory';

const call = (i: number, endedAt: number | null = null): ToolCall => ({
  toolUseId: `t${i}`,
  name: i % 2 ? 'Read' : 'Bash',
  summary: `s${i}`,
  startedAt: 1000 + i,
  endedAt,
  isError: endedAt == null ? null : false,
});

const started = (i: number, seq: number): VirgilEvent => ({
  type: 'toolCallStarted', seq, t: 1000 + i, sessionId: 's', agentId: 'a', call: call(i), tokens: 0, outputTokens: 0,
});

describe('ToolHistory', () => {
  test('keeps every call of one batch, not just the newest eight', () => {
    const h = new ToolHistory();
    const events = Array.from({ length: 30 }, (_, i) => started(i, i + 1));
    expect(h.applyEvents(events)).toBe(true);
    expect(h.ticks('s', 'a')).toHaveLength(30);
    expect(ticksUntil(h.ticks('s', 'a'), 1009)).toHaveLength(10);
  });

  test('a finish event and a late, older history merge without losing the result', () => {
    const h = new ToolHistory();
    h.applyEvents([started(1, 1), { type: 'toolCallFinished', seq: 2, t: 2000, sessionId: 's', agentId: 'a', toolUseId: 't1', isError: true }]);
    // The core's history was taken before the result: it must not undo it.
    expect(h.addCalls('s', 'a', [call(1), call(2)])).toBe(true);
    const ticks = h.ticks('s', 'a');
    expect(ticks.map((t) => [t.toolUseId, t.endedAt, t.isError])).toEqual([
      ['t1', 2000, true],
      ['t2', null, null],
    ]);
    expect(h.addCalls('s', 'a', [call(1), call(2)])).toBe(false);
    expect(toolBreakdown(ticks)).toEqual([{ name: 'Bash', n: 1 }, { name: 'Read', n: 1 }]);
  });

  test('clear forgets everything', () => {
    const h = new ToolHistory();
    h.addCalls('s', 'a', [call(1)]);
    h.clear();
    expect(h.ticks('s', 'a')).toEqual([]);
  });
});
