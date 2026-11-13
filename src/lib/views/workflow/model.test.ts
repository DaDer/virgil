import { describe, expect, it } from 'vitest';
import { agentsByPhase, activePhaseIndex } from '../../data/derive';
import { makeAgent } from '../../data/factories';
import { SID, sixtyAgents, T0 } from '../shared/fixtures';
import { activityLines, columns, moveInBoard, orderCards, runBar } from './model';

const RUN = 'wf_00000000-001';

describe('workflow model', () => {
  it('orders cards running, waiting, failed, stopped, done, then by start', () => {
    const a = (id: string, state: 'running' | 'waiting' | 'failed' | 'stopped' | 'done', startedAt: number) => makeAgent({ agentId: id, sessionId: SID, state, startedAt });
    const out = orderCards([a('d', 'done', 1), a('r2', 'running', 5), a('f', 'failed', 1), a('w', 'waiting', 1), a('r1', 'running', 2), a('s', 'stopped', 1)]);
    expect(out.map((x) => x.agentId)).toEqual(['r1', 'r2', 'w', 'f', 's', 'd']);
  });

  it('builds six columns; big ones compact; the active one marked', () => {
    const s = sixtyAgents();
    const run = s.runs[RUN];
    const cols = columns(s, run, agentsByPhase(s, run), activePhaseIndex(s, run));
    expect(cols.map((c) => c.total)).toEqual([6, 6, 18, 19, 8, 1]);
    expect(cols.map((c) => c.compact)).toEqual([false, false, true, true, false, false]);
    expect(cols.find((c) => c.active)?.phase.index).toBe(6);
    expect(cols[2].agents[0].state).toBe('running');
  });

  it('fills the whole-run bar by share, with equal segments per phase', () => {
    const s = sixtyAgents();
    const run = s.runs[RUN];
    const bar = runBar(columns(s, run, agentsByPhase(s, run), null));
    expect(bar).toHaveLength(6);
    expect(bar[0]).toMatchObject({ done: 1, running: 0 });
    expect(bar[2].done).toBeCloseTo(12 / 18);
    expect(bar[2].running).toBeCloseTo(6 / 18);
    expect(bar[5]).toMatchObject({ done: 0, running: 0, failed: 0 });
  });

  it('derives activity lines up to now, oldest first', () => {
    const s = sixtyAgents();
    const run = s.runs[RUN];
    const lines = activityLines(s, run, T0 + 61_000);
    expect(lines[0].text).toBe('Run build started');
    expect(lines.every((l, i) => i === 0 || l.t >= lines[i - 1].t)).toBe(true);
    expect(lines.some((l) => l.text.startsWith('spec:0 started'))).toBe(true);
    expect(lines.some((l) => l.text.includes('done'))).toBe(false);
  });

  it('moves between cards and columns with arrows', () => {
    const cols = [{ agents: [{ agentId: 'a' }, { agentId: 'b' }] }, { agents: [] }, { agents: [{ agentId: 'c' }] }];
    expect(moveInBoard(cols, null, 'ArrowDown')).toBe('a');
    expect(moveInBoard(cols, 'a', 'ArrowDown')).toBe('b');
    expect(moveInBoard(cols, 'b', 'ArrowRight')).toBe('c');
    expect(moveInBoard(cols, 'c', 'ArrowLeft')).toBe('a');
    expect(moveInBoard(cols, 'a', 'ArrowUp')).toBe('a');
  });
});
