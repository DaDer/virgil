import { describe, expect, it } from 'vitest';
import { makeRun } from '../../data/factories';
import { oneAgent, SID, sixtyAgents, T0, tenAgents } from '../shared/fixtures';
import { buildRows, phaseBands, span, visibleRange, type RowOptions } from './buildRows';

const opts = (o: Partial<RowOptions> = {}): RowOptions => ({ group: 'phase', collapsed: {}, scopedRunId: null, viewTime: T0 + 3_600_000, hideFuture: false, ...o });

describe('buildRows', () => {
  it('1 agent: session row and one agent row', () => {
    expect(buildRows(oneAgent(), opts()).map((r) => r.kind)).toEqual(['session', 'agent']);
  });

  it('10 agents: nested agents follow their parent, one level deeper', () => {
    const rows = buildRows(tenAgents(), opts());
    const ids = rows.map((r) => (r.kind === 'agent' ? `${r.agentId}@${r.depth}` : r.kind));
    expect(ids.slice(0, 4)).toEqual(['session', 'a0@1', 'a8@2', 'a9@2']);
    expect(rows).toHaveLength(11);
  });

  it('60 agents grouped by phase: run, phases with counts, agents inside', () => {
    const rows = buildRows(sixtyAgents(), opts());
    expect(rows.filter((r) => r.kind === 'run')).toHaveLength(1);
    const phases = rows.filter((r) => r.kind === 'phase');
    expect(phases.map((p) => (p.kind === 'phase' ? `${p.done}/${p.total}` : ''))).toEqual(['6/6', '6/6', '12/18', '0/19', '0/8', '0/1']);
    expect(rows).toHaveLength(1 + 2 + 1 + 6 + 58);
    expect(phaseBands(rows)).toHaveLength(6);
    const band = phaseBands(rows)[0];
    expect(band.to - band.from + 1).toBe(6);
  });

  it('collapses a phase or a run', () => {
    const run = 'wf_00000000-001';
    expect(buildRows(sixtyAgents(), opts({ collapsed: { [`p:${run}:4`]: true } }))).toHaveLength(68 - 19);
    expect(buildRows(sixtyAgents(), opts({ collapsed: { [`r:${run}`]: true } }))).toHaveLength(1 + 2 + 1);
  });

  it('finished runs other than the scoped one start collapsed', () => {
    const s = sixtyAgents();
    s.runs.old = makeRun({ runId: 'old', sessionId: SID, status: 'completed', startedAt: T0, endedAt: T0 + 1000 });
    const rows = buildRows(s, opts({ scopedRunId: 'wf_00000000-001' }));
    const old = rows.find((r) => r.kind === 'run' && r.runId === 'old');
    expect(old && old.kind === 'run' && old.collapsed).toBe(true);
  });

  it('flat groupings: by start, and by state with running first', () => {
    const byStart = buildRows(tenAgents(), opts({ group: 'start' }));
    expect(byStart.map((r) => (r.kind === 'agent' ? r.agentId : r.kind)).slice(0, 3)).toEqual(['session', 'a0', 'a1']);
    const byState = buildRows(tenAgents(), opts({ group: 'state' }));
    expect(byState[1].kind === 'agent' && byState[1].agentId).toBe('a1');
    expect(byState.at(-1)!.kind === 'agent' && byState.at(-1)).toMatchObject({ agentId: 'a9' });
  });

  it('hides agents that start after viewTime in replay', () => {
    expect(buildRows(tenAgents(), opts({ hideFuture: true, viewTime: T0 + 15_000 }))).toHaveLength(3);
  });

  it('spans run from the first queue time to the last end; active ends at viewTime', () => {
    const s = tenAgents();
    expect(span([s.agents.a0, s.agents.a1], T0 + 99_000)).toEqual({ start: T0, end: T0 + 99_000 });
  });

  it('virtualises with overscan', () => {
    expect(visibleRange(70, 28, 0, 280)).toEqual([0, 16]);
    expect(visibleRange(70, 28, 1400, 280)).toEqual([44, 66]);
    expect(visibleRange(5, 28, 0, 280)).toEqual([0, 5]);
  });
});
