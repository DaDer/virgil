// The graph's structure (ux-spec 3.2), as a pure function of one session and the view options.
//
// Only the top level goes to ELK: the session node, plain agents and run containers. Inside a
// run, phases sit left to right and agents are packed in a grid of ceil(sqrt(n)) columns (at
// most 4); that packing is computed here, so a run is a fixed-size box for ELK.
//
// The output `key` changes only when the structure changes (nodes, parents, sizes, edges).
// State, token, tool and time updates never change it, so they never trigger a layout.

import { MAIN_AGENT_ID, type Agent, type Phase, type Session, type WorkflowRun } from '../../contract';
import { agentsByPhase } from '../../data/derive';
import { startsAfter } from '../shared/motion';

export type Detail = 'full' | 'compact' | 'dot';
export type DetailSetting = 'auto' | Detail;

export const SIZE = {
  full: { w: 248, h: 104 },
  compact: { w: 168, h: 44 },
  dot: { w: 28, h: 28 },
  session: { w: 248, h: 88 },
  runCollapsed: { w: 248, h: 72 },
  emptyPhase: { w: 168, h: 72 },
} as const;

export const RUN_HEADER = 36;
export const PHASE_HEADER = 28;
export const PAD = 12;
export const CELL_GAP = 10;
export const PHASE_GAP = 40;
export const MAX_COLS = 4;
/** A container with more agents than this uses compact nodes. */
export const FULL_MAX = 8;

export interface GraphOptions {
  detail: DetailSetting;
  /** The zoom used to pick the automatic level of detail. */
  zoom: number;
  /** Explicit collapse choices by runId; others take the default rule. */
  collapsed: Record<string, boolean>;
  viewTime: number;
  /** Replay: hide agents that start after viewTime (ux-spec 1.4). */
  hideFuture: boolean;
}

export type NodeKind = 'session' | 'agent' | 'run' | 'phase';

export interface GNode {
  id: string;
  kind: NodeKind;
  /** Container node id; positions of children are relative to it. */
  parent: string | null;
  width: number;
  height: number;
  /** Relative position inside the parent (children only; top level comes from ELK). */
  x: number;
  y: number;
  agentId?: string;
  runId?: string;
  phaseIndex?: number;
  detail?: Detail;
  collapsed?: boolean;
}

export type EdgeKind = 'spawn' | 'run' | 'sequence';

export interface GEdge {
  id: string;
  kind: EdgeKind;
  source: string;
  target: string;
  /** The agent at the child end (spawn edges) or the run (run edges), for "active" state. */
  agentId?: string;
  runId?: string;
}

export interface LayoutInput {
  nodes: { id: string; width: number; height: number }[];
  edges: { id: string; source: string; target: string }[];
}

export interface GraphModel {
  nodes: GNode[];
  edges: GEdge[];
  layout: LayoutInput;
  key: string;
  /** Agent id to node id, for agents that are drawn. */
  agentNode: Map<string, string>;
  /** Number of agent nodes drawn. */
  agentCount: number;
}

export const SESSION_NODE = 'session';
export const agentNodeId = (id: string) => `a:${id}`;
export const runNodeId = (id: string) => `r:${id}`;
export const phaseNodeId = (runId: string, index: number) => `p:${runId}:${index}`;

/** Level of detail for agents in a container of `count` agents (ux-spec 3.2 table). */
export function detailFor(setting: DetailSetting, zoom: number, count: number): Detail {
  if (setting !== 'auto') return setting;
  if (zoom < 0.45) return 'dot';
  if (zoom < 0.8 || count > FULL_MAX) return 'compact';
  return 'full';
}

/** Zoom band used for the automatic level: a change of band is a structure change. */
export function zoomBand(zoom: number): number {
  return zoom < 0.45 ? 0.3 : zoom < 0.8 ? 0.6 : 1;
}

/** Grid packing: ceil(sqrt(n)) columns, at most MAX_COLS. */
export function gridFor(n: number): { cols: number; rows: number } {
  if (n <= 0) return { cols: 0, rows: 0 };
  const cols = Math.min(MAX_COLS, Math.ceil(Math.sqrt(n)));
  return { cols, rows: Math.ceil(n / cols) };
}

/**
 * Default collapse: runs that finished more than 10 minutes before viewTime start collapsed when
 * the session has more than one run; the newest run is always expanded.
 */
export function defaultCollapsed(runs: WorkflowRun[], viewTime: number): Set<string> {
  const out = new Set<string>();
  if (runs.length < 2) return out;
  const newest = [...runs].sort((a, b) => (b.startedAt ?? 0) - (a.startedAt ?? 0))[0];
  for (const r of runs) {
    if (r === newest || r.status === 'running') continue;
    if (r.endedAt != null && r.endedAt < viewTime - 10 * 60_000) out.add(r.runId);
  }
  return out;
}

/** Phases of a run, plus any phase index an agent uses that the run does not declare. */
export function runPhases(run: WorkflowRun, byPhase: Map<number, Agent[]>): Phase[] {
  const phases = [...run.phases];
  for (const [idx, agents] of byPhase) {
    if (phases.some((p) => p.index === idx) || !agents.length) continue;
    phases.push({ index: idx, title: agents[0].phaseTitle ?? (idx === 0 ? 'Unassigned' : `Phase ${idx}`), detail: null, model: null, state: 'running' });
  }
  return phases.sort((a, b) => a.index - b.index);
}

export function buildGraph(s: Session, opts: GraphOptions): GraphModel {
  const nodes: GNode[] = [];
  const edges: GEdge[] = [];
  const layoutEdges: LayoutInput['edges'] = [];
  const agentNode = new Map<string, string>();
  const visible = (a: Agent) => !(opts.hideFuture && startsAfter(a, opts.viewTime));

  nodes.push({ id: SESSION_NODE, kind: 'session', parent: null, width: SIZE.session.w, height: SIZE.session.h, x: 0, y: 0, agentId: MAIN_AGENT_ID });
  agentNode.set(MAIN_AGENT_ID, SESSION_NODE);

  const runs = Object.values(s.runs)
    .filter((r) => !(opts.hideFuture && r.startedAt != null && r.startedAt > opts.viewTime))
    .sort((a, b) => (a.startedAt ?? 0) - (b.startedAt ?? 0) || a.runId.localeCompare(b.runId));
  const defaults = defaultCollapsed(runs, opts.viewTime);

  // Runs first, so agents inside them are known before plain agents look up their parents.
  const topOf = new Map<string, string>(); // node id -> top-level node id
  topOf.set(SESSION_NODE, SESSION_NODE);
  for (const run of runs) {
    const rid = runNodeId(run.runId);
    const collapsed = opts.collapsed[run.runId] ?? defaults.has(run.runId);
    const byPhase = agentsByPhase(s, run);
    for (const [k, list] of byPhase) byPhase.set(k, list.filter(visible));
    const phases = runPhases(run, byPhase);
    const runNode: GNode = { id: rid, kind: 'run', parent: null, width: SIZE.runCollapsed.w, height: SIZE.runCollapsed.h, x: 0, y: 0, runId: run.runId, collapsed };
    nodes.push(runNode);
    topOf.set(rid, rid);
    if (collapsed) {
      for (const list of byPhase.values()) for (const a of list) {
        agentNode.set(a.agentId, rid);
        topOf.set(agentNodeId(a.agentId), rid);
      }
      continue;
    }
    let x = PAD;
    let maxH = 0;
    let prevPhase: string | null = null;
    const phaseNodes: GNode[] = [];
    const children: GNode[] = [];
    for (const ph of phases) {
      const pid = phaseNodeId(run.runId, ph.index);
      const agents = byPhase.get(ph.index) ?? [];
      const detail = detailFor(opts.detail, opts.zoom, agents.length);
      const cell = SIZE[detail];
      const { cols, rows } = gridFor(agents.length);
      const w = agents.length ? Math.max(SIZE.emptyPhase.w, PAD * 2 + cols * cell.w + (cols - 1) * CELL_GAP) : SIZE.emptyPhase.w;
      const h = agents.length ? PHASE_HEADER + PAD + rows * cell.h + (rows - 1) * CELL_GAP + PAD : SIZE.emptyPhase.h;
      const pnode: GNode = { id: pid, kind: 'phase', parent: rid, width: w, height: h, x, y: RUN_HEADER + PAD, runId: run.runId, phaseIndex: ph.index };
      phaseNodes.push(pnode);
      topOf.set(pid, rid);
      agents.forEach((a, i) => {
        const id = agentNodeId(a.agentId);
        children.push({
          id,
          kind: 'agent',
          parent: pid,
          width: cell.w,
          height: cell.h,
          x: PAD + (i % cols) * (cell.w + CELL_GAP),
          y: PHASE_HEADER + PAD + Math.floor(i / cols) * (cell.h + CELL_GAP),
          agentId: a.agentId,
          runId: run.runId,
          phaseIndex: ph.index,
          detail,
        });
        agentNode.set(a.agentId, id);
        topOf.set(id, rid);
      });
      if (prevPhase) edges.push({ id: `seq:${prevPhase}>${pid}`, kind: 'sequence', source: prevPhase, target: pid, runId: run.runId });
      prevPhase = pid;
      x += w + PHASE_GAP;
      maxH = Math.max(maxH, h);
    }
    runNode.width = Math.max(SIZE.runCollapsed.w, x - PHASE_GAP + PAD);
    runNode.height = RUN_HEADER + PAD + Math.max(maxH, SIZE.emptyPhase.h) + PAD;
    nodes.push(...phaseNodes, ...children);
  }

  // Plain agents, in start order, hanging off whoever started them.
  const plain = Object.values(s.agents)
    .filter((a) => a.agentId !== MAIN_AGENT_ID && !a.runId && visible(a))
    .sort((a, b) => (a.startedAt ?? 0) - (b.startedAt ?? 0) || a.agentId.localeCompare(b.agentId));
  const plainCount = plain.length;
  const plainDetail = detailFor(opts.detail, opts.zoom, plainCount > 40 ? FULL_MAX + 1 : 1);
  for (const a of plain) {
    const id = agentNodeId(a.agentId);
    nodes.push({ id, kind: 'agent', parent: null, width: SIZE[plainDetail].w, height: SIZE[plainDetail].h, x: 0, y: 0, agentId: a.agentId, detail: plainDetail });
    agentNode.set(a.agentId, id);
    topOf.set(id, id);
  }
  const plainIds = new Set(plain.map((a) => a.agentId));
  const parentNodeOf = (agentId: string | null): string => {
    if (!agentId || agentId === MAIN_AGENT_ID) return SESSION_NODE;
    if (plainIds.has(agentId)) return agentNodeId(agentId);
    return agentNode.get(agentId) ?? SESSION_NODE;
  };

  const seenLayout = new Set<string>();
  const addLayoutEdge = (source: string, target: string) => {
    const a = topOf.get(source) ?? SESSION_NODE;
    const b = topOf.get(target) ?? target;
    if (a === b) return;
    const id = `${a}>${b}`;
    if (seenLayout.has(id)) return;
    seenLayout.add(id);
    layoutEdges.push({ id, source: a, target: b });
  };

  for (const a of plain) {
    const src = parentNodeOf(a.parentAgentId);
    const tgt = agentNodeId(a.agentId);
    edges.push({ id: `spawn:${src}>${tgt}`, kind: 'spawn', source: src, target: tgt, agentId: a.agentId });
    addLayoutEdge(src, tgt);
  }
  for (const run of runs) {
    const src = parentNodeOf(run.parentAgentId);
    const tgt = runNodeId(run.runId);
    edges.push({ id: `run:${src}>${tgt}`, kind: 'run', source: src, target: tgt, runId: run.runId });
    addLayoutEdge(src, tgt);
  }

  const top = nodes.filter((n) => n.parent === null);
  const layout: LayoutInput = {
    nodes: top.map((n) => ({ id: n.id, width: n.width, height: n.height })),
    edges: layoutEdges,
  };
  const key =
    nodes.map((n) => `${n.id}|${n.parent ?? ''}|${n.width}x${n.height}|${n.x},${n.y}`).join(';') +
    '#' +
    edges.map((e) => e.id).join(';');
  return { nodes, edges, layout, key, agentNode, agentCount: plainCount + runs.reduce((n, r) => n + r.agentIds.length, 0) };
}

/** Edge ids on the parent chain of an agent, up to the session (selection highlight). */
export function chainEdges(model: GraphModel, agentId: string): Set<string> {
  const out = new Set<string>();
  const byTarget = new Map<string, GEdge>();
  for (const e of model.edges) if (e.kind !== 'sequence') byTarget.set(e.target, e);
  let node = model.agentNode.get(agentId) ?? null;
  // Workflow agents link through their run container.
  const guard = new Set<string>();
  while (node && node !== SESSION_NODE && !guard.has(node)) {
    guard.add(node);
    let e = byTarget.get(node);
    if (!e) {
      const n = model.nodes.find((x) => x.id === node);
      if (n?.runId && n.kind !== 'run') e = byTarget.get(runNodeId(n.runId));
    }
    if (!e) break;
    out.add(e.id);
    node = e.source;
  }
  return out;
}
