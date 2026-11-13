import ELK from 'elkjs/lib/elk.bundled.js';
import { describe, expect, it } from 'vitest';
import { oneAgent, sixtyAgents, SID, T0, tenAgents } from '../shared/fixtures';
import {
  buildGraph,
  chainEdges,
  defaultCollapsed,
  detailFor,
  gridFor,
  SESSION_NODE,
  SIZE,
  zoomBand,
  type GraphOptions,
  type GNode,
} from './buildGraph';
import { fallbackLayout, fromElk, LayoutEngine, toElk } from './layout';
import { makeRun } from '../../data/factories';

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
