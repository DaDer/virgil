// Pure helpers for the Workflow view (ux-spec 3.3).

import type { Agent, AgentState, Phase, Session, WorkflowRun } from '../../contract';
import { countStates, type StateCounts } from '../../data/derive';
import { formatDuration } from '../../format';
import { runPhases } from '../graph/buildGraph';

/** A column switches to compact cards above this many agents. */
export const COMPACT_ABOVE = 8;

const ORDER: Record<AgentState, number> = { running: 0, waiting: 1, queued: 2, failed: 3, stopped: 4, done: 5 };

/** Card order in a column: running, waiting, failed, stopped, done; within a group by start time. */
export function orderCards(agents: readonly Agent[]): Agent[] {
  return [...agents].sort(
    (a, b) =>
      ORDER[a.state] - ORDER[b.state] ||
      (a.startedAt ?? a.queuedAt ?? Infinity) - (b.startedAt ?? b.queuedAt ?? Infinity) ||
      a.agentId.localeCompare(b.agentId),
  );
}

export interface ColumnModel {
  phase: Phase;
  agents: Agent[];
  counts: StateCounts;
  /** Agents that count: not superseded. */
  total: number;
  compact: boolean;
  active: boolean;
}

/** One column per phase (declared, plus any an agent uses), agents ordered. */
export function columns(s: Session, run: WorkflowRun, byPhase: Map<number, Agent[]>, activeIndex: number | null): ColumnModel[] {
  return runPhases(run, byPhase).map((phase) => {
    const agents = orderCards(byPhase.get(phase.index) ?? []);
    const live = agents.filter((a) => !a.superseded);
    return {
      phase,
      agents,
      counts: countStates(live),
      total: live.length,
      compact: agents.length > COMPACT_ABOVE,
      active: activeIndex === phase.index,
    };
  });
}

export interface RunBarSegment {
  index: number;
  title: string;
  /** Shares of the segment, 0..1, in order done, running, failed. */
  done: number;
  running: number;
  failed: number;
}

/** Whole-run bar: one equal-width segment per phase, filled by the share of its agents. */
export function runBar(cols: readonly ColumnModel[]): RunBarSegment[] {
  return cols.map((c) => {
    const n = c.total;
    return {
      index: c.phase.index,
      title: c.phase.title,
      done: n ? c.counts.done / n : 0,
      running: n ? c.counts.running / n : 0,
      failed: n ? (c.counts.failed + c.counts.stopped) / n : 0,
    };
  });
}

export interface ActivityLine {
  t: number;
  text: string;
  tone: 'running' | 'done' | 'error' | 'stopped' | 'idle';
}

/**
 * The run's activity as log lines, oldest first: run start, each agent's start and end. The run
 * file's own log lines are not part of the data contract, so this is what the panel can show.
 */
export function activityLines(s: Session, run: WorkflowRun, now: number): ActivityLine[] {
  const out: ActivityLine[] = [];
  if (run.startedAt != null) out.push({ t: run.startedAt, text: `Run ${run.name} started`, tone: 'running' });
  for (const id of run.agentIds) {
    const a = s.agents[id];
    if (!a) continue;
    const phase = a.phaseIndex != null ? `${a.phaseIndex} ${a.phaseTitle ?? ''}`.trim() : '';
    const where = phase ? ` (phase ${phase})` : '';
    if (a.startedAt != null && a.startedAt <= now) out.push({ t: a.startedAt, text: `${a.label} started${where}`, tone: 'running' });
    if (a.endedAt != null && a.endedAt <= now) {
      const took = a.startedAt != null ? `, took ${formatDuration(a.endedAt - a.startedAt)}` : '';
      if (a.state === 'done') out.push({ t: a.endedAt, text: `${a.label} done${took}`, tone: 'done' });
      else if (a.state === 'failed') out.push({ t: a.endedAt, text: `${a.label} failed${a.superseded ? ' (retried)' : a.stateReason ? `: ${a.stateReason}` : ''}`, tone: 'error' });
      else if (a.state === 'stopped') out.push({ t: a.endedAt, text: `${a.label} stopped${took}`, tone: 'stopped' });
    }
  }
  if (run.endedAt != null && run.endedAt <= now) out.push({ t: run.endedAt, text: `Run ${run.status === 'completed' ? 'finished' : run.status}`, tone: run.status === 'completed' ? 'done' : run.status === 'failed' ? 'error' : 'stopped' });
  return out.sort((a, b) => a.t - b.t);
}

/** Arrow-key movement between cards (ux-spec 3.3): within a column and across columns. */
export function moveInBoard(cols: readonly { agents: readonly { agentId: string }[] }[], current: string | null, key: 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight'): string | null {
  let ci = -1;
  let ri = -1;
  cols.forEach((c, i) => {
    const j = c.agents.findIndex((a) => a.agentId === current);
    if (j >= 0) [ci, ri] = [i, j];
  });
  if (ci < 0) return cols.find((c) => c.agents.length)?.agents[0]?.agentId ?? null;
  if (key === 'ArrowUp') return cols[ci].agents[Math.max(0, ri - 1)].agentId;
  if (key === 'ArrowDown') return cols[ci].agents[Math.min(cols[ci].agents.length - 1, ri + 1)].agentId;
  const dir = key === 'ArrowRight' ? 1 : -1;
  for (let i = ci + dir; i >= 0 && i < cols.length; i += dir) {
    const list = cols[i].agents;
    if (list.length) return list[Math.min(ri, list.length - 1)].agentId;
  }
  return current;
}
