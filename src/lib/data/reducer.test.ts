import { describe, expect, test } from 'vitest';
import type { VirgilEvent, Snapshot } from '../contract';
import { makeAgent, makeRun, makeSessionInfo } from './factories';
import { apply, applyEvents, emptySnapshot } from './reducer';

const SID = 's1';
let seq = 0;
const ev = <E extends Omit<VirgilEvent, 'seq'>>(e: E) => ({ ...e, seq: ++seq }) as unknown as VirgilEvent;

function base(): Snapshot {
  seq = 0;
  return applyEvents(emptySnapshot(), [
    ev({ type: 'sessionUpserted', t: 100, session: makeSessionInfo({ sessionId: SID, name: 'one' }) }),
    ev({ type: 'agentSpawned', t: 100, sessionId: SID, agent: makeAgent({ agentId: 'main', sessionId: SID }) }),
  ]);
}

const call = (id: string, t: number) => ({ toolUseId: id, name: 'Read', summary: 'a.ts', startedAt: t, endedAt: null, isError: null });

describe('reducer', () => {
  test('sets seq and keeps the max t', () => {
    const s = base();
    expect(s.seq).toBe(2);
    const s2 = apply(s, ev({ type: 'sessionUpserted', t: 50, session: makeSessionInfo({ sessionId: SID, name: 'renamed' }) }));
    expect(s2.seq).toBe(3);
    expect(s2.t).toBe(100);
  });

  test('sessionUpserted keeps agents, runs and files', () => {
    const s = base();
    const s2 = apply(s, ev({ type: 'sessionUpserted', t: 200, session: makeSessionInfo({ sessionId: SID, name: 'renamed' }) }));
    expect(s2.sessions[SID].name).toBe('renamed');
    expect(s2.sessions[SID].agents.main).toBe(s.sessions[SID].agents.main);
  });

  test('does not mutate its input', () => {
    const s = base();
    const frozen = JSON.stringify(s);
    applyEvents(s, [
      ev({ type: 'toolCallStarted', t: 300, sessionId: SID, agentId: 'main', call: call('t1', 300), tokens: 10, outputTokens: 2 }),
      ev({ type: 'fileTouched', t: 300, sessionId: SID, agentId: 'main', path: '/p/a.ts', op: 'read', source: 'tool', toolUseId: 't1' }),
      ev({ type: 'sessionRemoved', t: 400, sessionId: SID }),
    ]);
    expect(JSON.stringify(s)).toBe(frozen);
  });

  test('tool calls: recentTools capped at 8, counters, lastTool, finish', () => {
    let s = base();
    for (let i = 0; i < 10; i++) {
      s = apply(s, ev({ type: 'toolCallStarted', t: 1000 + i, sessionId: SID, agentId: 'main', call: call(`t${i}`, 1000 + i), tokens: i * 10, outputTokens: i }));
    }
    s = apply(s, ev({ type: 'toolCallFinished', t: 2000, sessionId: SID, agentId: 'main', toolUseId: 't9', isError: true }));
    const a = s.sessions[SID].agents.main;
    expect(a.recentTools.map((c) => c.toolUseId)).toEqual(['t2', 't3', 't4', 't5', 't6', 't7', 't8', 't9']);
    expect(a.toolCalls).toBe(10);
    expect(a.tokens).toBe(90);
    expect(a.lastTool).toEqual({ name: 'Read', summary: 'a.ts', at: 1009 });
    expect(a.recentTools[7]).toMatchObject({ endedAt: 2000, isError: true });
    expect(a.lastActivityAt).toBe(2000);
  });

  test('file touches aggregate per agent', () => {
    let s = base();
    s = apply(s, ev({ type: 'agentSpawned', t: 1, sessionId: SID, agent: makeAgent({ agentId: 'a1', sessionId: SID }) }));
    const touch = (agentId: string, t: number, op: 'read' | 'write', source: 'tool' | 'bash' = 'tool') =>
      ev({ type: 'fileTouched', t, sessionId: SID, agentId, path: '/p/a.ts', op, source, toolUseId: 'x' });
    s = applyEvents(s, [touch('main', 50, 'read'), touch('main', 30, 'write', 'bash'), touch('a1', 60, 'write')]);
    const f = s.sessions[SID].files['/p/a.ts'];
    expect(f.touches.main).toEqual({ agentId: 'main', reads: 1, writes: 1, firstAt: 30, lastAt: 50, viaBash: true });
    expect(Object.keys(f.touches)).toEqual(['main', 'a1']);
  });

  test('agentSpawned appends to the run, runFinalized replaces agents', () => {
    let s = base();
    s = apply(s, ev({ type: 'runStarted', t: 1, sessionId: SID, run: makeRun({ runId: 'r', sessionId: SID }) }));
    s = apply(s, ev({ type: 'agentSpawned', t: 2, sessionId: SID, agent: makeAgent({ agentId: 'w1', sessionId: SID, kind: 'workflow', runId: 'r' }) }));
    s = apply(s, ev({ type: 'agentSpawned', t: 2, sessionId: SID, agent: makeAgent({ agentId: 'w1', sessionId: SID, kind: 'workflow', runId: 'r' }) }));
    expect(s.sessions[SID].runs.r.agentIds).toEqual(['w1']);
    s = apply(s, ev({
      type: 'runFinalized', t: 3, sessionId: SID,
      run: makeRun({ runId: 'r', sessionId: SID, final: true, status: 'completed', agentIds: ['w1', 'r#2'] }),
      agents: [makeAgent({ agentId: 'w1', sessionId: SID, state: 'done' }), makeAgent({ agentId: 'r#2', sessionId: SID, state: 'failed' })],
    }));
    expect(s.sessions[SID].runs.r.final).toBe(true);
    expect(s.sessions[SID].agents['r#2'].state).toBe('failed');
    expect(s.sessions[SID].agents.w1.state).toBe('done');
  });

  test('events naming missing things are ignored', () => {
    const s = base();
    const s2 = applyEvents(s, [
      ev({ type: 'agentUpdated', t: 5, sessionId: SID, agent: makeAgent({ agentId: 'ghost', sessionId: SID }) }),
      ev({ type: 'toolCallStarted', t: 5, sessionId: 'nope', agentId: 'main', call: call('x', 5), tokens: 1, outputTokens: 1 }),
      ev({ type: 'runUpdated', t: 5, sessionId: SID, run: makeRun({ runId: 'r', sessionId: SID }) }),
    ]);
    expect(s2.sessions[SID].agents.ghost).toBeUndefined();
    expect(s2.sessions[SID].runs.r).toBeUndefined();
    expect(s2.seq).toBe(5);
  });

  test('snapshotReset replaces everything', () => {
    const s = base();
    const reset = emptySnapshot('replay');
    const s2 = apply(s, ev({ type: 'snapshotReset', t: 9, snapshot: { ...reset, seq: 99 } }));
    expect(s2.sessions).toEqual({});
    expect(s2.mode).toBe('replay');
    expect(s2.seq).toBe(seq);
  });

  test('applying a batch equals applying one by one', () => {
    const s = base();
    const events = [
      ev({ type: 'toolCallStarted', t: 10, sessionId: SID, agentId: 'main', call: call('a', 10), tokens: 5, outputTokens: 1 }),
      ev({ type: 'toolCallFinished', t: 11, sessionId: SID, agentId: 'main', toolUseId: 'a', isError: false }),
      ev({ type: 'fileTouched', t: 10, sessionId: SID, agentId: 'main', path: '/x', op: 'write', source: 'tool', toolUseId: 'a' }),
    ];
    const one = events.reduce(apply, s);
    expect(applyEvents(s, events)).toEqual(one);
  });
});
