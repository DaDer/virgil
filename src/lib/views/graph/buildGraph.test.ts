import ELK from 'elkjs/lib/elk.bundled.js';
import { describe, expect, it } from 'vitest';
import { oneAgent, sixtyAgents, SID, T0, tenAgents } from '../shared/fixtures';
import {
  buildGraph,
  chainEdges,
  defaultCollapsed,
  detailFor,
  gridFor,
  inFocus,
  jobNodeId,
  runNodeId,
  stackNodeId,
  agentNodeId,
  phaseNodeId,
  SESSION_NODE,
  SIZE,
  zoomBand,
  type GraphOptions,
  type GNode,
} from './buildGraph';
import { fallbackLayout, fromElk, LayoutEngine, toElk } from './layout';
import { makeBackgroundTask, makeRun } from '../../data/factories';
import type { BackgroundTask, Session } from '../../contract';

function addJob(s: Session, taskId: string, patch: Partial<BackgroundTask> = {}): Session {
  s.backgroundTasks[taskId] = makeBackgroundTask({ taskId, sessionId: s.sessionId, summary: taskId, startedAt: T0 + 1000, ...patch });
  return s;
}

const opts = (o: Partial<GraphOptions> = {}): GraphOptions => ({ detail: 'auto', zoom: 1, collapsed: {}, viewTime: T0 + 3_600_000, hideFuture: false, ...o });

function overlaps(a: { x: number; y: number; w: number; h: number }, b: { x: number; y: number; w: number; h: number }) {
  return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
}

describe('detail and packing rules', () => {
  it('picks the level of detail from zoom and container size', () => {
    expect(detailFor('auto', 1, 6)).toBe('full');
    expect(detailFor('auto', 1, 9)).toBe('compact');
    expect(detailFor('auto', 0.6, 2)).toBe('compact');
    expect(detailFor('auto', 0.3, 2)).toBe('dot');
    expect(detailFor('full', 0.3, 40)).toBe('full');
    expect(zoomBand(0.9)).toBe(zoomBand(1.8));
    expect(zoomBand(0.5)).not.toBe(zoomBand(0.9));
  });

  it('packs ceil(sqrt(n)) columns, at most 4', () => {
    expect(gridFor(1)).toEqual({ cols: 1, rows: 1 });
    expect(gridFor(6)).toEqual({ cols: 3, rows: 2 });
    expect(gridFor(19)).toEqual({ cols: 4, rows: 5 });
    expect(gridFor(0)).toEqual({ cols: 0, rows: 0 });
  });

  it('collapses runs that ended long ago, never the newest', () => {
    const a = makeRun({ runId: 'r1', sessionId: SID, startedAt: T0, endedAt: T0 + 1000, status: 'completed' });
    const b = makeRun({ runId: 'r2', sessionId: SID, startedAt: T0 + 5000, endedAt: T0 + 6000, status: 'completed' });
    expect([...defaultCollapsed([a, b], T0 + 3_600_000)]).toEqual(['r1']);
    expect(defaultCollapsed([a], T0 + 3_600_000).size).toBe(0);
  });
});

describe('buildGraph', () => {
  it('1 agent: session, one edge, one full node', () => {
    const g = buildGraph(oneAgent(), opts());
    expect(g.nodes.map((n) => n.kind)).toEqual(['session', 'agent']);
    expect(g.edges).toHaveLength(1);
    expect(g.edges[0]).toMatchObject({ kind: 'spawn', source: SESSION_NODE, target: 'a:a1' });
    expect(g.nodes[1].detail).toBe('full');
  });

  it('10 agents: nested agents hang off their parent', () => {
    const g = buildGraph(tenAgents(), opts());
    expect(g.nodes.filter((n) => n.kind === 'agent')).toHaveLength(10);
    expect(g.edges.find((e) => e.target === 'a:a8')?.source).toBe('a:a0');
    expect(g.layout.edges.find((e) => e.target === 'a:a9')?.source).toBe('a:a0');
  });

  it('60 agents: one run box with six phases, big phases compact, 6 edges for the run', () => {
    const s = sixtyAgents();
    const g = buildGraph(s, opts());
    const phases = g.nodes.filter((n) => n.kind === 'phase');
    expect(phases).toHaveLength(6);
    const agentsIn = (p: GNode) => g.nodes.filter((n) => n.parent === p.id);
    expect(agentsIn(phases[0])[0].detail).toBe('full');
    expect(agentsIn(phases[2])[0].detail).toBe('compact');
    expect(agentsIn(phases[3])).toHaveLength(19);
    // 5 sequence edges + 1 run edge + 2 spawn edges; no edge per workflow agent.
    expect(g.edges.filter((e) => e.kind === 'sequence')).toHaveLength(5);
    expect(g.edges.filter((e) => e.kind === 'run')).toHaveLength(1);
    expect(g.edges).toHaveLength(8);
    // ELK sees only the top level: session, the run and two plain agents.
    expect(g.layout.nodes.map((n) => n.id).sort()).toEqual(['a:p1', 'a:p2', 'r:wf_00000000-001', 'session']);
  });

  it('children fit inside their container without overlapping', () => {
    const g = buildGraph(sixtyAgents(), opts());
    const byId = new Map(g.nodes.map((n) => [n.id, n]));
    for (const n of g.nodes) {
      if (!n.parent) continue;
      const p = byId.get(n.parent)!;
      expect(n.x).toBeGreaterThanOrEqual(0);
      expect(n.y).toBeGreaterThanOrEqual(0);
      expect(n.x + n.width).toBeLessThanOrEqual(p.width);
      expect(n.y + n.height).toBeLessThanOrEqual(p.height);
      for (const m of g.nodes) {
        if (m === n || m.parent !== n.parent) continue;
        expect(overlaps({ x: n.x, y: n.y, w: n.width, h: n.height }, { x: m.x, y: m.y, w: m.width, h: m.height })).toBe(false);
      }
    }
  });

  it('the key ignores state and token changes but follows structure', () => {
    const s = sixtyAgents();
    const k1 = buildGraph(s, opts()).key;
    const changed = { ...s, agents: { ...s.agents, w20: { ...s.agents.w20, state: 'done' as const, tokens: 999_999, toolCalls: 80 } } };
    expect(buildGraph(changed, opts()).key).toBe(k1);
    expect(buildGraph(s, opts({ zoom: 0.3 })).key).not.toBe(k1);
    expect(buildGraph(s, opts({ collapsed: { 'wf_00000000-001': true } })).key).not.toBe(k1);
  });

  it('a collapsed run is one 248 x 72 node; its agents map to it', () => {
    const g = buildGraph(sixtyAgents(), opts({ collapsed: { 'wf_00000000-001': true } }));
    const run = g.nodes.find((n) => n.kind === 'run')!;
    expect([run.width, run.height]).toEqual([SIZE.runCollapsed.w, SIZE.runCollapsed.h]);
    expect(g.nodes.some((n) => n.kind === 'phase')).toBe(false);
    expect(g.agentNode.get('w3')).toBe(run.id);
  });

  it('replay hides agents that start after viewTime', () => {
    const g = buildGraph(tenAgents(), opts({ viewTime: T0 + 25_000, hideFuture: true }));
    expect(g.nodes.filter((n) => n.kind === 'agent').map((n) => n.agentId)).toEqual(['a0', 'a1', 'a2']);
  });

  it('highlights the parent chain up to the session', () => {
    const g = buildGraph(tenAgents(), opts());
    expect([...chainEdges(g, 'a8')].sort()).toEqual(['spawn:a:a0>a:a8', 'spawn:session>a:a0']);
    const w = buildGraph(sixtyAgents(), opts());
    expect([...chainEdges(w, 'w3')]).toEqual(['run:session>r:wf_00000000-001']);
  });
});

describe('layout', () => {
  const engine = () => new LayoutEngine(async () => new ELK());

  for (const [name, make] of [
    ['1', oneAgent],
    ['10', tenAgents],
    ['60', sixtyAgents],
  ] as const) {
    it(`ELK lays out ${name} agent(s) left to right without overlaps`, async () => {
      const g = buildGraph(make(), opts());
      const res = (await engine().request(g.layout))!;
      const boxes = g.layout.nodes.map((n) => ({ id: n.id, ...res.positions[n.id], w: n.width, h: n.height }));
      for (const b of boxes) expect(Number.isFinite(b.x) && Number.isFinite(b.y)).toBe(true);
      for (let i = 0; i < boxes.length; i++) for (let j = i + 1; j < boxes.length; j++) expect(overlaps(boxes[i], boxes[j])).toBe(false);
      const session = boxes.find((b) => b.id === SESSION_NODE)!;
      for (const b of boxes) if (b.id !== SESSION_NODE) expect(b.x).toBeGreaterThan(session.x);
    });
  }

  it('drops the result of an older request', async () => {
    const e = engine();
    const g = buildGraph(tenAgents(), opts());
    const [a, b] = await Promise.all([e.request(g.layout), e.request(g.layout)]);
    expect(a).toBeNull();
    expect(b).not.toBeNull();
  });

  it('toElk skips edges to unknown nodes; fromElk rounds', () => {
    const elk = toElk({ nodes: [{ id: 'x', width: 10, height: 10 }], edges: [{ id: 'e', source: 'x', target: 'y' }] });
    expect(elk.edges).toHaveLength(0);
    expect(fromElk({ id: 'root', children: [{ id: 'x', x: 1.4, y: 2.6, width: 10, height: 10 }] }).positions.x).toEqual({ x: 1, y: 3 });
  });

  it('falls back to a ranked layout when ELK fails', async () => {
    const e = new LayoutEngine(async () => {
      throw new Error('no worker');
    });
    const g = buildGraph(tenAgents(), opts());
    const res = (await e.request(g.layout))!;
    expect(res.positions['a:a8'].x).toBeGreaterThan(res.positions['a:a0'].x);
    expect(fallbackLayout(g.layout).positions[SESSION_NODE]).toEqual({ x: 0, y: 0 });
  });
});

describe('active focus', () => {
  const now = T0 + 3_600_000;
  const base = { sessionId: SID, kind: 'subagent' as const, parentAgentId: 'main', spawnDepth: 1, startedAt: T0 };
  function session() {
    const s = oneAgent();
    const add = (agentId: string, patch: Record<string, unknown>) => {
      s.agents[agentId] = { ...Object.values(s.agents).find((a) => a.agentId !== 'main')!, ...base, agentId, label: agentId, runId: null, phaseIndex: null, ...patch } as never;
    };
    for (const id of Object.keys(s.agents)) if (id !== 'main') delete s.agents[id];
    add('live', { state: 'running', endedAt: null, lastActivityAt: now });
    add('old-parent', { state: 'done', endedAt: now - 50 * 60_000 });
    add('live-child', { state: 'running', endedAt: null, parentAgentId: 'old-parent', spawnDepth: 2 });
    add('recent', { state: 'done', endedAt: now - 2 * 60_000 });
    add('failed', { state: 'failed', endedAt: now - 30 * 60_000 });
    for (let i = 0; i < 6; i++) add(`old-${i}`, { state: i === 0 ? 'stopped' : 'done', endedAt: now - 45 * 60_000 });
    add('old-grandchild', { state: 'done', endedAt: now - 45 * 60_000, parentAgentId: 'old-0', spawnDepth: 2 });
    return s;
  }
  const drawn = (m: ReturnType<typeof buildGraph>) => m.nodes.filter((n) => n.kind === 'agent').map((n) => n.agentId).sort();

  it('keeps working agents, their ancestors, recent ones and failures; folds the rest into one stack per owner', () => {
    const s = session();
    expect([...inFocus(s, now)].sort()).toEqual(['failed', 'live', 'live-child', 'old-parent', 'recent']);
    const m = buildGraph(s, opts({ viewTime: now, focus: 'active' }));
    expect(drawn(m)).toEqual(['failed', 'live', 'live-child', 'old-parent', 'recent']);
    const stacks = m.nodes.filter((n) => n.kind === 'stack');
    expect(stacks).toHaveLength(1);
    expect(stacks[0].id).toBe(stackNodeId(SESSION_NODE));
    // The grandchild of a folded agent goes into the same stack, under the nearest drawn ancestor.
    expect(stacks[0].agentIds).toHaveLength(7);
    expect(m.agentNode.get('old-3')).toBe(stacks[0].id);
    expect(m.edges.some((e) => e.source === SESSION_NODE && e.target === stacks[0].id)).toBe(true);
    expect(m.layout.nodes.some((n) => n.id === stacks[0].id)).toBe(true);
    // Everything is drawn in the default view.
    expect(drawn(buildGraph(s, opts({ viewTime: now })))).toHaveLength(12);
  });

  it('draws a stack\'s agents again when it is opened, and keeps the selected agent in view', () => {
    const s = session();
    const sid = stackNodeId(SESSION_NODE);
    const open = buildGraph(s, opts({ viewTime: now, focus: 'active', expanded: { [sid]: true } }));
    expect(drawn(open)).toHaveLength(12);
    expect(open.nodes.find((n) => n.id === sid)?.expanded).toBe(true);
    expect(open.agentNode.get('old-3')).toBe(agentNodeId('old-3'));
    expect(open.edges.some((e) => e.source === agentNodeId('old-0') && e.target === agentNodeId('old-grandchild'))).toBe(true);
    const kept = buildGraph(s, opts({ viewTime: now, focus: 'active', keep: 'old-grandchild' }));
    expect(drawn(kept)).toEqual(expect.arrayContaining(['old-grandchild', 'old-0']));
    expect(buildGraph(s, opts({ viewTime: now, focus: 'active' })).key).not.toBe(open.key);
  });

  it('folds the finished agents of a workflow phase into a stack cell inside the phase', () => {
    const s = sixtyAgents();
    const run = Object.values(s.runs)[0];
    // The agents the full view draws inside the run; make the first one work and the rest old.
    const ids = buildGraph(s, opts({ viewTime: now })).nodes.filter((n) => n.kind === 'agent' && n.runId === run.runId).map((n) => n.agentId!);
    ids.forEach((id, i) => Object.assign(s.agents[id], i === 0 ? { state: 'running', endedAt: null } : { state: 'done', endedAt: now - 30 * 60_000, lastActivityAt: now - 30 * 60_000 }));
    const m = buildGraph(s, opts({ viewTime: now, focus: 'active' }));
    const inRun = m.nodes.filter((n) => n.kind === 'agent' && n.runId === run.runId);
    expect(inRun.map((n) => n.agentId)).toEqual([ids[0]]);
    const stacks = m.nodes.filter((n) => n.kind === 'stack' && n.runId === run.runId);
    expect(stacks.length).toBeGreaterThan(0);
    expect([...stacks.flatMap((st) => st.agentIds!), ids[0]].sort()).toEqual([...ids].sort());
    for (const st of stacks) {
      expect(st.parent).toBe(phaseNodeId(run.runId, st.phaseIndex!));
      const phase = m.nodes.find((n) => n.id === st.parent)!;
      expect(st.x + st.width).toBeLessThanOrEqual(phase.width);
      expect(st.y + st.height).toBeLessThanOrEqual(phase.height);
    }
  });
});

describe('background commands', () => {
  const now = T0 + 3_600_000;
  const base = { sessionId: SID, kind: 'subagent' as const, parentAgentId: 'main', spawnDepth: 1, startedAt: T0 };
  // Same shape as the active-focus fixture: live and recent agents, a failure, old ones that fold.
  function session() {
    const s = oneAgent();
    const add = (agentId: string, patch: Record<string, unknown>) => {
      s.agents[agentId] = { ...Object.values(s.agents).find((a) => a.agentId !== 'main')!, ...base, agentId, label: agentId, runId: null, phaseIndex: null, ...patch } as never;
    };
    for (const id of Object.keys(s.agents)) if (id !== 'main') delete s.agents[id];
    add('live', { state: 'running', endedAt: null, lastActivityAt: now });
    add('recent', { state: 'done', endedAt: now - 2 * 60_000 });
    for (let i = 0; i < 4; i++) add(`old-${i}`, { state: 'done', endedAt: now - 45 * 60_000 });
    add('old-grandchild', { state: 'done', endedAt: now - 45 * 60_000, parentAgentId: 'old-0', spawnDepth: 2 });
    return s;
  }
  const active = (o: Partial<GraphOptions> = {}) => opts({ viewTime: now, focus: 'active', ...o });
  const jobs = (m: ReturnType<typeof buildGraph>) => m.nodes.filter((n) => n.kind === 'job').map((n) => n.taskId).sort();
  const drawn = (m: ReturnType<typeof buildGraph>) => m.nodes.filter((n) => n.kind === 'agent').map((n) => n.agentId).sort();

  it('hangs a command off the agent that started it with a job edge; main-thread commands off the session', () => {
    const s = addJob(addJob(session(), 't-live', { agentId: 'live' }), 't-main', { agentId: 'main', status: 'completed', endedAt: T0 + 5000 });
    const g = buildGraph(s, opts({ viewTime: now }));
    expect(g.nodes.find((n) => n.id === jobNodeId('t-live'))).toMatchObject({ kind: 'job', parent: null, width: SIZE.job.w, height: SIZE.job.h });
    expect(g.edges.find((e) => e.target === jobNodeId('t-live'))).toMatchObject({ kind: 'job', source: agentNodeId('live'), taskId: 't-live' });
    expect(g.edges.find((e) => e.target === jobNodeId('t-main'))).toMatchObject({ kind: 'job', source: SESSION_NODE });
    expect(g.layout.edges.some((e) => e.source === agentNodeId('live') && e.target === jobNodeId('t-live'))).toBe(true);
    expect(g.layout.nodes.some((n) => n.id === jobNodeId('t-main'))).toBe(true);
  });

  it("a workflow agent's command gets its layout edge from the run node", () => {
    const s = addJob(sixtyAgents(), 't-w', { agentId: 'w3' });
    const g = buildGraph(s, opts());
    const rid = runNodeId('wf_00000000-001');
    expect(g.edges.find((e) => e.kind === 'job')).toMatchObject({ source: agentNodeId('w3'), target: jobNodeId('t-w') });
    expect(g.layout.edges).toContainEqual({ id: `${rid}>${jobNodeId('t-w')}`, source: rid, target: jobNodeId('t-w') });
    // Collapsed, the run itself is the starter's node.
    const c = buildGraph(s, opts({ collapsed: { 'wf_00000000-001': true } }));
    expect(c.edges.find((e) => e.kind === 'job')?.source).toBe(rid);
  });

  it('active: a running command is drawn and keeps its finished starter in view', () => {
    const s = addJob(session(), 't-run', { agentId: 'old-1' });
    expect(inFocus(s, now)).toContain('old-1');
    const m = buildGraph(s, active());
    expect(jobs(m)).toEqual(['t-run']);
    expect(drawn(m)).toContain('old-1');
    expect(m.edges.find((e) => e.kind === 'job')?.source).toBe(agentNodeId('old-1'));
  });

  it('active: an old finished command folds into its owner\'s stack, shared with the folded agents', () => {
    const s = addJob(session(), 't-old', { agentId: 'main', status: 'completed', endedAt: now - 40 * 60_000 });
    const m = buildGraph(s, active());
    expect(jobs(m)).toEqual([]);
    const stacks = m.nodes.filter((n) => n.kind === 'stack');
    expect(stacks).toHaveLength(1);
    expect(stacks[0]).toMatchObject({ id: stackNodeId(SESSION_NODE), taskIds: ['t-old'] });
    expect(stacks[0].agentIds).toHaveLength(5);
    // A command that just ended is still drawn.
    const recent = buildGraph(addJob(s, 't-recent', { status: 'completed', endedAt: now - 60_000 }), active());
    expect(jobs(recent)).toEqual(['t-recent']);
  });

  it('active: a command on a drawn agent with nothing else folded gets a stack of its own', () => {
    const s = addJob(session(), 't-old', { agentId: 'live', status: 'stopped', endedAt: now - 40 * 60_000 });
    const m = buildGraph(s, active());
    const st = m.nodes.find((n) => n.id === stackNodeId(agentNodeId('live')))!;
    expect(st).toMatchObject({ kind: 'stack', agentIds: [], taskIds: ['t-old'] });
    expect(m.edges.find((e) => e.target === st.id)).toMatchObject({ kind: 'job', source: agentNodeId('live') });
    const open = buildGraph(s, active({ expanded: { [st.id]: true } }));
    expect(jobs(open)).toEqual(['t-old']);
  });

  it('a failed command stays drawn for FAILED_KEEP_MS, then folds', () => {
    const s = addJob(session(), 't-failed', { agentId: 'old-2', status: 'failed', endedAt: now - 30 * 60_000 });
    const m = buildGraph(s, active());
    expect(jobs(m)).toEqual(['t-failed']);
    expect(m.edges.find((e) => e.kind === 'job')?.source).toBe(agentNodeId('old-2'));
    s.backgroundTasks['t-failed'].endedAt = now - 90 * 60_000;
    expect(jobs(buildGraph(s, active()))).toEqual([]);
  });

  it("a command whose starter is folded lands in the starter's stack; opening it draws the command", () => {
    const s = addJob(session(), 't-gc', { agentId: 'old-grandchild', status: 'completed', endedAt: now - 40 * 60_000 });
    const sid = stackNodeId(SESSION_NODE);
    const m = buildGraph(s, active());
    expect(m.nodes.filter((n) => n.kind === 'stack').map((n) => [n.id, n.taskIds])).toEqual([[sid, ['t-gc']]]);
    const open = buildGraph(s, active({ expanded: { [sid]: true } }));
    expect(jobs(open)).toEqual(['t-gc']);
    expect(open.edges.find((e) => e.kind === 'job')?.source).toBe(agentNodeId('old-grandchild'));
  });

  it('a command of a folded workflow agent goes into that phase\'s stack cell', () => {
    const s = sixtyAgents();
    const run = Object.values(s.runs)[0];
    for (const id of run.agentIds) Object.assign(s.agents[id], { state: 'done', endedAt: now - 30 * 60_000, lastActivityAt: now - 30 * 60_000 });
    for (const id of ['p1', 'p2']) Object.assign(s.agents[id], { state: 'done', endedAt: now - 30 * 60_000, lastActivityAt: now - 30 * 60_000 });
    const w = run.agentIds[0];
    addJob(s, 't-w', { agentId: w, status: 'completed', endedAt: now - 20 * 60_000 });
    const m = buildGraph(s, active());
    const st = m.nodes.find((n) => n.kind === 'stack' && n.agentIds?.includes(w))!;
    expect(st.parent).toBe(phaseNodeId(run.runId, s.agents[w].phaseIndex!));
    expect(st.taskIds).toEqual(['t-w']);
    expect(jobs(m)).toEqual([]);
    expect(jobs(buildGraph(s, active({ expanded: { [st.id]: true } })))).toEqual(['t-w']);
  });

  it('a status change does not change the key; a new command does', () => {
    const s = addJob(session(), 't1', { agentId: 'live' });
    const k = buildGraph(s, opts({ viewTime: now })).key;
    const ended = { ...s, backgroundTasks: { t1: { ...s.backgroundTasks.t1, status: 'failed' as const, endedAt: now - 1000, outputPath: '/tmp/x.output' } } };
    expect(buildGraph(ended, opts({ viewTime: now })).key).toBe(k);
    expect(buildGraph(addJob(structuredClone(s), 't2', { agentId: 'live' }), opts({ viewTime: now })).key).not.toBe(k);
  });

  it('replay hides commands that start after the view time', () => {
    const s = addJob(addJob(session(), 'early', { startedAt: T0 + 1000 }), 'late', { startedAt: T0 + 60_000 });
    expect(jobs(buildGraph(s, opts({ viewTime: T0 + 30_000, hideFuture: true })))).toEqual(['early']);
    expect(jobs(buildGraph(s, opts({ viewTime: T0 + 30_000 })))).toEqual(['early', 'late']);
  });

  it('chainEdges ignores job edges', () => {
    const s = addJob(addJob(session(), 't1', { agentId: 'live' }), 't-old', { agentId: 'live', status: 'completed', endedAt: now - 40 * 60_000 });
    const g = buildGraph(s, active());
    expect(g.edges.some((e) => e.kind === 'job')).toBe(true);
    expect([...chainEdges(g, 'live')]).toEqual(['spawn:session>a:live']);
  });
});
