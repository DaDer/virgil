// Timeline rows (ux-spec 3.4) from one session, a grouping and collapse state. Pure.

import { MAIN_AGENT_ID, type Agent, type AgentState, type Session, type WorkflowRun } from '../../contract';
import { agentsByPhase, childrenOf } from '../../data/derive';
import { runPhases } from '../graph/buildGraph';
import { startsAfter } from '../shared/motion';

export type Grouping = 'phase' | 'start' | 'state';

export type TimelineRow =
  | { kind: 'session'; id: string; depth: 0 }
  | { kind: 'agent'; id: string; agentId: string; depth: number; phaseKey: string | null }
  | { kind: 'run'; id: string; runId: string; depth: number; collapsed: boolean; count: number; start: number | null; end: number | null }
  | {
      kind: 'phase';
      id: string;
      runId: string;
      index: number;
      title: string;
      depth: number;
      collapsed: boolean;
      done: number;
      total: number;
      start: number | null;
      end: number | null;
    };

export interface RowOptions {
  group: Grouping;
  /** Explicit collapse choices by row id. */
  collapsed: Record<string, boolean>;
  /** The scoped run: finished runs other than this one start collapsed. */
  scopedRunId: string | null;
  viewTime: number;
  /** Replay: hide agents that start after viewTime. */
  hideFuture: boolean;
}

const STATE_ORDER: Record<AgentState, number> = { running: 0, waiting: 1, queued: 2, failed: 3, stopped: 4, done: 5 };

export const firstTime = (a: Agent): number => a.queuedAt ?? a.startedAt ?? Number.POSITIVE_INFINITY;
const byStart = (a: Agent, b: Agent) => firstTime(a) - firstTime(b) || a.agentId.localeCompare(b.agentId);

/** End of an agent's bar: its end, else viewTime while it is active. */
export function agentEnd(a: Agent, viewTime: number): number | null {
  if (a.endedAt != null) return a.endedAt;
  return a.startedAt != null ? viewTime : null;
}

/** Phase span: first queue (or start) time to the last end among its agents. */
export function span(agents: readonly Agent[], viewTime: number): { start: number | null; end: number | null } {
  let start: number | null = null;
  let end: number | null = null;
  for (const a of agents) {
    const s = a.queuedAt ?? a.startedAt;
    if (s != null && (start == null || s < start)) start = s;
    const e = agentEnd(a, viewTime);
    if (e != null && (end == null || e > end)) end = e;
  }
  return { start, end };
}

export function buildRows(s: Session, opts: RowOptions): TimelineRow[] {
  const visible = (a: Agent) => a.agentId !== MAIN_AGENT_ID && !(opts.hideFuture && startsAfter(a, opts.viewTime));
  const rows: TimelineRow[] = [{ kind: 'session', id: 'session', depth: 0 }];
  const agentRow = (a: Agent, depth: number, phaseKey: string | null): TimelineRow => ({ kind: 'agent', id: `a:${a.agentId}`, agentId: a.agentId, depth, phaseKey });

  if (opts.group !== 'phase') {
    const all = Object.values(s.agents).filter(visible);
    all.sort(opts.group === 'state' ? (a, b) => STATE_ORDER[a.state] - STATE_ORDER[b.state] || byStart(a, b) : byStart);
    for (const a of all) rows.push(agentRow(a, 1, null));
    return rows;
  }

  const kids = childrenOf(s);
  const seen = new Set<string>();
  const walk = (parent: string, depth: number, phaseKey: string | null) => {
    for (const c of [...(kids.get(parent) ?? [])].sort(byStart)) {
      if (!visible(c) || seen.has(c.agentId)) continue;
      seen.add(c.agentId);
      rows.push(agentRow(c, depth, phaseKey));
      walk(c.agentId, depth + 1, phaseKey);
    }
  };
  walk(MAIN_AGENT_ID, 1, null);

  const runs = Object.values(s.runs)
    .filter((r) => !(opts.hideFuture && r.startedAt != null && r.startedAt > opts.viewTime))
    .sort((a, b) => (a.startedAt ?? 0) - (b.startedAt ?? 0) || a.runId.localeCompare(b.runId));
  for (const run of runs) addRun(s, run, rows, opts, visible, walk, seen);

  // Agents whose parent never arrived and that no walk reached.
  for (const a of Object.values(s.agents).filter(visible).sort(byStart)) {
    if (!seen.has(a.agentId) && !a.runId) {
      seen.add(a.agentId);
      rows.push(agentRow(a, 1, null));
    }
  }
  return rows;
}

function addRun(
  s: Session,
  run: WorkflowRun,
  rows: TimelineRow[],
  opts: RowOptions,
  visible: (a: Agent) => boolean,
  walk: (parent: string, depth: number, phaseKey: string | null) => void,
  seen: Set<string>,
): void {
  const id = `r:${run.runId}`;
  const finished = run.status !== 'running';
  const defaultCollapsed = finished && opts.scopedRunId != null && run.runId !== opts.scopedRunId;
  const collapsed = opts.collapsed[id] ?? defaultCollapsed;
  const byPhase = agentsByPhase(s, run);
  const all = run.agentIds.map((x) => s.agents[x]).filter((a): a is Agent => !!a && visible(a));
  const runSpan = span(all, opts.viewTime);
  rows.push({
    kind: 'run',
    id,
    runId: run.runId,
    depth: 1,
    collapsed,
    count: all.filter((a) => !a.superseded).length,
    start: run.startedAt ?? runSpan.start,
    end: run.endedAt ?? (run.status === 'running' ? opts.viewTime : runSpan.end),
  });
  if (collapsed) {
    for (const a of all) seen.add(a.agentId);
    return;
  }
  for (const ph of runPhases(run, byPhase)) {
    const pid = `p:${run.runId}:${ph.index}`;
    const agents = (byPhase.get(ph.index) ?? []).filter(visible).sort(byStart);
    const live = agents.filter((a) => !a.superseded);
    const pcollapsed = opts.collapsed[pid] ?? false;
    rows.push({
      kind: 'phase',
      id: pid,
      runId: run.runId,
      index: ph.index,
      title: ph.title,
      depth: 2,
      collapsed: pcollapsed,
      done: live.filter((a) => a.state === 'done').length,
      total: live.length,
      ...span(agents, opts.viewTime),
    });
    for (const a of agents) {
      seen.add(a.agentId);
      if (pcollapsed) continue;
      rows.push({ kind: 'agent', id: `a:${a.agentId}`, agentId: a.agentId, depth: 3, phaseKey: pid });
      walk(a.agentId, 4, pid);
    }
  }
}

/** Visible slice for virtualisation: rows in the viewport plus `overscan` above and below. */
export function visibleRange(count: number, rowH: number, scrollTop: number, viewH: number, overscan = 6): [number, number] {
  const first = Math.max(0, Math.floor(scrollTop / rowH) - overscan);
  const last = Math.min(count, Math.ceil((scrollTop + viewH) / rowH) + overscan);
  return [first, last];
}

/** Index ranges of agent rows under each phase row, for the phase bands. */
export function phaseBands(rows: readonly TimelineRow[]): { id: string; from: number; to: number }[] {
  const out: { id: string; from: number; to: number }[] = [];
  rows.forEach((r, i) => {
    if (r.kind !== 'agent' || !r.phaseKey) return;
    const last = out[out.length - 1];
    if (last && last.id === r.phaseKey && last.to === i - 1) last.to = i;
    else out.push({ id: r.phaseKey, from: i, to: i });
  });
  return out;
}
