// Pure derived values over the snapshot, shared by every view (ux-spec 6.1, `world` derived maps).

import {
  MAIN_AGENT_ID,
  type Agent,
  type AgentState,
  type FileTouch,
  type Session,
  type Snapshot,
  type WorkflowRun,
} from '../contract';
import { isActive, sessionState } from '../states';

export type StateCounts = Record<AgentState, number>;

export function emptyCounts(): StateCounts {
  return { queued: 0, running: 0, waiting: 0, done: 0, failed: 0, stopped: 0 };
}

export function countStates(agents: Iterable<Agent>, opts: { skipSuperseded?: boolean } = {}): StateCounts {
  const c = emptyCounts();
  for (const a of agents) if (!(opts.skipSuperseded && a.superseded)) c[a.state] += 1;
  return c;
}

export function agentList(s: Session): Agent[] {
  return Object.values(s.agents);
}

/** Agents other than main, in start order. */
export function subagents(s: Session): Agent[] {
  return agentList(s)
    .filter((a) => a.agentId !== MAIN_AGENT_ID)
    .sort((a, b) => (a.startedAt ?? 0) - (b.startedAt ?? 0));
}

export interface SessionStats {
  running: number;
  agents: number;
  runs: number;
  tokens: number | null;
  toolCalls: number;
}

export function sessionStats(s: Session): SessionStats {
  let running = 0;
  let agents = 0;
  let tokens = 0;
  let known = false;
  let toolCalls = 0;
  for (const a of Object.values(s.agents)) {
    if (a.agentId !== MAIN_AGENT_ID) {
      agents += 1;
      if (a.state === 'running') running += 1;
    }
    if (a.tokens > 0) known = true;
    tokens += a.tokens;
    toolCalls += a.toolCalls;
  }
  return { running, agents, runs: Object.keys(s.runs).length, tokens: known ? tokens : null, toolCalls };
}

/**
 * Non-workflow children per parent. A parent that is not in the snapshot yet is drawn under main
 * (data-contract 2.1).
 */
export function childrenOf(s: Session): Map<string, Agent[]> {
  const m = new Map<string, Agent[]>();
  for (const a of subagents(s)) {
    if (a.runId) continue;
    const parent = a.parentAgentId && s.agents[a.parentAgentId] ? a.parentAgentId : MAIN_AGENT_ID;
    const list = m.get(parent);
    if (list) list.push(a);
    else m.set(parent, [a]);
  }
  return m;
}

/** Agents of a run per phase index, in first-seen order. */
export function agentsByPhase(s: Session, run: WorkflowRun): Map<number, Agent[]> {
  const m = new Map<number, Agent[]>();
  for (const p of run.phases) m.set(p.index, []);
  for (const id of run.agentIds) {
    const a = s.agents[id];
    if (!a) continue;
    const idx = a.phaseIndex ?? 0;
    const list = m.get(idx);
    if (list) list.push(a);
    else m.set(idx, [a]);
  }
  return m;
}

export function runAgents(s: Session, run: WorkflowRun): Agent[] {
  return run.agentIds.map((id) => s.agents[id]).filter((a): a is Agent => !!a);
}

export interface RunTotals {
  agents: number;
  done: number;
  running: number;
  waiting: number;
  failed: number;
  stopped: number;
  tokens: number;
  toolCalls: number;
  durationMs?: number;
}

export function runTotals(s: Session, run: WorkflowRun): RunTotals {
  const agents = runAgents(s, run).filter((a) => !a.superseded);
  const c = countStates(agents);
  const sumTokens = agents.reduce((n, a) => n + a.tokens, 0);
  const sumTools = agents.reduce((n, a) => n + a.toolCalls, 0);
  return {
    agents: agents.length,
    done: c.done,
    running: c.running,
    waiting: c.waiting + c.queued,
    failed: c.failed,
    stopped: c.stopped,
    tokens: run.totalTokens ?? sumTokens,
    toolCalls: run.totalToolCalls ?? sumTools,
    durationMs: run.final && run.startedAt != null && run.endedAt != null ? run.endedAt - run.startedAt : undefined,
  };
}

/** The latest phase with a running or waiting agent (ux-spec 3.3, "Phase 4 of 6"). */
export function activePhaseIndex(s: Session, run: WorkflowRun): number | null {
  let idx: number | null = null;
  for (const a of runAgents(s, run)) {
    if (!a.superseded && isActive(a.state) && a.phaseIndex != null && (idx == null || a.phaseIndex > idx)) idx = a.phaseIndex;
  }
  return idx;
}

/** The run in flight, else the newest run. */
export function currentRun(s: Session): WorkflowRun | null {
  const runs = Object.values(s.runs).sort((a, b) => (b.startedAt ?? 0) - (a.startedAt ?? 0));
  return runs.find((r) => r.status === 'running') ?? runs[0] ?? null;
}

export interface NowLine {
  phaseTitle: string;
  done: number;
  total: number;
}

/** SessionCard "Now:" line: only when a run is in flight. */
export function nowLine(s: Session): NowLine | undefined {
  const run = Object.values(s.runs).find((r) => r.status === 'running');
  if (!run) return undefined;
  const idx = activePhaseIndex(s, run) ?? [...run.phases].reverse().find((p) => p.state !== 'pending')?.index ?? run.phases[0]?.index;
  const phase = run.phases.find((p) => p.index === idx);
  if (!phase) return undefined;
  const agents = runAgents(s, run).filter((a) => a.phaseIndex === phase.index && !a.superseded);
  return { phaseTitle: phase.title, done: agents.filter((a) => a.state === 'done').length, total: agents.length };
}

// ---------- files ----------

/** Per agent, the files it touched. */
export function filesByAgent(s: Session): Map<string, { path: string; touch: FileTouch }[]> {
  const m = new Map<string, { path: string; touch: FileTouch }[]>();
  for (const f of Object.values(s.files)) {
    for (const t of Object.values(f.touches)) {
      const list = m.get(t.agentId);
      const item = { path: f.path, touch: t };
      if (list) list.push(item);
      else m.set(t.agentId, [item]);
    }
  }
  return m;
}

/** Per path, the agents that touched it. */
export function fileTouchers(s: Session): Map<string, FileTouch[]> {
  const m = new Map<string, FileTouch[]>();
  for (const f of Object.values(s.files)) m.set(f.path, Object.values(f.touches));
  return m;
}

export interface SharedPair {
  a: string;
  b: string;
  files: string[];
  /** Both agents edited at least one of the shared files. */
  bothEdited: boolean;
}

/** One entry per pair of agents that touched the same file (ux-spec 3.2 shared-files overlay). */
export function sharedPairs(s: Session, mode: 'edits' | 'all' = 'all'): SharedPair[] {
  const pairs = new Map<string, SharedPair>();
  for (const f of Object.values(s.files)) {
    const ts = Object.values(f.touches);
    if (ts.length < 2) continue;
    for (let i = 0; i < ts.length; i++) {
      for (let j = i + 1; j < ts.length; j++) {
        const both = ts[i].writes > 0 && ts[j].writes > 0;
        if (mode === 'edits' && !both) continue;
        const [a, b] = ts[i].agentId < ts[j].agentId ? [ts[i].agentId, ts[j].agentId] : [ts[j].agentId, ts[i].agentId];
        const key = `${a}\u0000${b}`;
        const p = pairs.get(key) ?? { a, b, files: [], bothEdited: false };
        p.files.push(f.path);
        p.bothEdited ||= both;
        pairs.set(key, p);
      }
    }
  }
  return [...pairs.values()].sort((x, y) => y.files.length - x.files.length || (x.a + x.b).localeCompare(y.a + y.b));
}

// ---------- chains ----------

export interface ChainPart {
  kind: 'session' | 'agent' | 'run' | 'phase';
  id: string;
  label: string;
}

/** `Session › Run › Phase › Agent` or `Session › Agent › Agent` (ux-spec 3.5). */
export function parentChain(s: Session, agentId: string): ChainPart[] {
  const out: ChainPart[] = [];
  const seen = new Set<string>();
  let a: Agent | undefined = s.agents[agentId];
  while (a && !seen.has(a.agentId)) {
    seen.add(a.agentId);
    if (a.agentId !== MAIN_AGENT_ID) out.unshift({ kind: 'agent', id: a.agentId, label: a.label });
    if (a.runId) {
      const run: WorkflowRun | undefined = s.runs[a.runId];
      if (a.phaseIndex != null) {
        const ph = run?.phases.find((p) => p.index === a!.phaseIndex);
        out.unshift({ kind: 'phase', id: phaseKey(a.runId, a.phaseIndex), label: `${a.phaseIndex} ${ph?.title ?? a.phaseTitle ?? ''}`.trim() });
      }
      out.unshift({ kind: 'run', id: a.runId, label: run?.name ?? a.runId });
      a = run ? s.agents[run.parentAgentId] : s.agents[MAIN_AGENT_ID];
      continue;
    }
    if (a.agentId === MAIN_AGENT_ID) break;
    a = (a.parentAgentId && s.agents[a.parentAgentId]) || s.agents[MAIN_AGENT_ID];
  }
  out.unshift({ kind: 'session', id: s.sessionId, label: sessionTitle(s) });
  return out;
}

export function phaseKey(runId: string, index: number): string {
  return `${runId}:${index}`;
}

export function parsePhaseKey(key: string): { runId: string; index: number } | null {
  const i = key.lastIndexOf(':');
  if (i < 0) return null;
  const index = Number(key.slice(i + 1));
  return Number.isInteger(index) ? { runId: key.slice(0, i), index } : null;
}

/** Session name; falls back to the AI title, then to "Session " + first 8 characters of the id. */
export function sessionTitle(s: Pick<Session, 'name' | 'title' | 'sessionId'>): string {
  return s.name || s.title || `Session ${s.sessionId.slice(0, 8)}`;
}

/** Busy first, then most recently updated. */
export function compareSessionsBusyFirst(a: Session, b: Session): number {
  const rank = (s: Session) => ({ busy: 0, waiting: 1, idle: 2, ended: 3, stale: 4 })[sessionState(s)];
  return rank(a) - rank(b) || lastActivity(b) - lastActivity(a);
}

export function lastActivity(s: Session): number {
  let t = s.updatedAt ?? s.startedAt ?? 0;
  for (const a of Object.values(s.agents)) if (a.lastActivityAt && a.lastActivityAt > t) t = a.lastActivityAt;
  return t;
}

/** Count times into `buckets` equal slices of [from, to). */
export function bucketize(times: Iterable<number>, from: number, to: number, buckets: number): number[] {
  const out = new Array<number>(buckets).fill(0);
  const span = to - from;
  if (span <= 0 || buckets <= 0) return out;
  for (const t of times) {
    if (t < from || t >= to) continue;
    out[Math.min(buckets - 1, Math.floor(((t - from) / span) * buckets))] += 1;
  }
  return out;
}

export function allSessions(snap: Snapshot): Session[] {
  return Object.values(snap.sessions);
}
