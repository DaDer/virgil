// A deterministic, scripted scenario for the mock source: three sessions (two alive, one stale),
// nested subagents, a plain subagent in a worktree, and a four-phase workflow run of 21 agent
// attempts (20 agents plus one retry, one failure) that is finalised near the end.
// Everything is invented; no real transcript content. The only randomness is a fixed-seed LCG.

import {
  MAIN_AGENT_ID,
  type Agent,
  type VirgilEvent,
  type FileOp,
  type Phase,
  type SessionInfo,
  type Snapshot,
  type ToolCall,
  type WorkflowRun,
} from '../contract';
import { makeAgent, makeRun, makeSessionInfo, withPhaseStates } from './factories';
import { applyEvents, emptySnapshot } from './reducer';

/** Length of one loop of the scenario. */
export const SCENARIO_DURATION = 6 * 60_000;
/** Sessions exist from this long before scenario time 0, with some earlier tool calls. */
export const PREHISTORY = 4 * 3_600_000;

type DistributiveOmit<T, K extends keyof never> = T extends unknown ? Omit<T, K> : never;
export type UnsequencedEvent = DistributiveOmit<VirgilEvent, 'seq'>;

export const SESSION_ORCHARD = '7c1e4a52-3b9d-4f6e-8a21-5d0c9b7e1f34';
export const SESSION_NOTES = 'b42d9e10-6a7f-4c3b-9e58-2f1a8d6c4b07';
export const SESSION_SCRATCH = 'e9f03b6d-1c2a-4d8e-b7f5-6a4c3e2d1b09';
export const DEMO_RUN_ID = 'wf_3c9a1e70-5b2';

class Lcg {
  private s: number;
  constructor(seed: number) {
    this.s = seed >>> 0;
  }
  next(): number {
    this.s = (Math.imul(this.s, 1664525) + 1013904223) >>> 0;
    return this.s / 0x1_0000_0000;
  }
  int(lo: number, hi: number): number {
    return lo + Math.floor(this.next() * (hi - lo + 1));
  }
  pick<T>(xs: readonly T[]): T {
    return xs[Math.floor(this.next() * xs.length)];
  }
  hex(n: number): string {
    let s = '';
    for (let i = 0; i < n; i++) s += '0123456789abcdef'[Math.floor(this.next() * 16)];
    return s;
  }
}

interface Action {
  t: number;
  order: number;
  run: (snap: Snapshot) => UnsequencedEvent[];
}

const ORCHARD = '/home/dev/projects/orchard';
const NOTES = '/home/dev/sites/notes';

const ORCHARD_FILES = [
  'src/stock/api.ts',
  'src/stock/model.ts',
  'src/stock/store.ts',
  'src/screens/StockScreen.svelte',
  'src/screens/StockRow.svelte',
  'src/lib/format.ts',
  'src/lib/http.ts',
  'tests/stock.test.ts',
  'tests/screen.test.ts',
  'docs/stock.md',
  'package.json',
  'README.md',
].map((p) => `${ORCHARD}/${p}`);

const NOTES_FILES = ['src/feed.ts', 'src/pages/index.astro', 'astro.config.mjs', 'README.md'].map((p) => `${NOTES}/${p}`);

const BASH = [
  'npm test',
  'npm run build',
  'git status --short',
  'cargo check',
  'ls src/stock',
  'npm run lint -- --fix',
  'git diff --stat',
];
const GREP = ['StockRow', 'fetchStock\\(', 'TODO', 'export function', 'useStore'];

class Builder {
  private actions: Action[] = [];
  private order = 0;
  private toolSeq = 0;
  readonly rng = new Lcg(0x5eed_a11);

  constructor(readonly base: number) {}

  at(rel: number, run: (snap: Snapshot) => UnsequencedEvent[]): void {
    this.actions.push({ t: this.base + rel, order: this.order++, run });
  }

  toolId(): string {
    this.toolSeq += 1;
    return `toolu_${Math.abs(this.base).toString(36)}${String(this.toolSeq).padStart(5, '0')}${this.rng.hex(8)}`;
  }

  agentId(): string {
    return `a${this.rng.hex(16)}`;
  }

  session(rel: number, info: SessionInfo): void {
    this.at(rel, () => [{ type: 'sessionUpserted', t: this.base + rel, session: info }]);
  }

  sessionPatch(rel: number, sessionId: string, patch: Partial<SessionInfo>): void {
    this.at(rel, (snap) => {
      const s = snap.sessions[sessionId];
      if (!s) return [];
      const { agents: _a, runs: _r, files: _f, ...info } = s;
      return [{ type: 'sessionUpserted', t: this.base + rel, session: { ...info, ...patch } }];
    });
  }

  spawn(rel: number, agent: Partial<Agent> & { agentId: string; sessionId: string }): void {
    const t = this.base + rel;
    this.at(rel, (snap) => {
      const full = makeAgent({ startedAt: t, lastActivityAt: t, ...agent });
      const out: UnsequencedEvent[] = [{ type: 'agentSpawned', t, sessionId: agent.sessionId, agent: full }];
      const run = full.runId ? snap.sessions[agent.sessionId]?.runs[full.runId] : undefined;
      if (run) {
        const next = { ...run, agentIds: run.agentIds.includes(full.agentId) ? run.agentIds : [...run.agentIds, full.agentId] };
        next.phases = withPhaseStates(next, { ...snap.sessions[agent.sessionId].agents, [full.agentId]: full });
        if (next.phases.some((p, i) => p.state !== run.phases[i]?.state)) {
          out.push({ type: 'runUpdated', t, sessionId: agent.sessionId, run: next });
        }
      }
      return out;
    });
  }

  /** Replace fields of an agent; recompute and emit the run's phases when it is a workflow agent. */
  update(rel: number, sessionId: string, agentId: string, patch: Partial<Agent> | ((a: Agent) => Partial<Agent>)): void {
    const t = this.base + rel;
    this.at(rel, (snap) => {
      const s = snap.sessions[sessionId];
      const a = s?.agents[agentId];
      if (!a) return [];
      const next: Agent = { ...a, ...(typeof patch === 'function' ? patch(a) : patch) };
      const out: UnsequencedEvent[] = [{ type: 'agentUpdated', t, sessionId, agent: next }];
      if (next.runId && s.runs[next.runId]) {
        const run = s.runs[next.runId];
        const phases = withPhaseStates(run, { ...s.agents, [agentId]: next });
        if (phases.some((p, i) => p.state !== run.phases[i]?.state)) {
          out.push({ type: 'runUpdated', t, sessionId, run: { ...run, phases } });
        }
      }
      return out;
    });
  }

  finish(rel: number, sessionId: string, agentId: string, state: Agent['state'], extra: Partial<Agent> = {}): void {
    const t = this.base + rel;
    this.update(rel, sessionId, agentId, (a) => ({ state, endedAt: t, lastActivityAt: Math.max(a.lastActivityAt ?? 0, t), ...extra }));
  }

  tool(rel: number, sessionId: string, agentId: string, name: string, summary: string, opts: { dur?: number; error?: boolean; file?: { path: string; op: FileOp; bash?: boolean } } = {}): void {
    const t = this.base + rel;
    const id = this.toolId();
    const tokensDelta = this.rng.int(1800, 9000);
    const outDelta = this.rng.int(60, 640);
    this.at(rel, (snap) => {
      const a = snap.sessions[sessionId]?.agents[agentId];
      if (!a) return [];
      const call: ToolCall = { toolUseId: id, name, summary, startedAt: t, endedAt: null, isError: null };
      const out: UnsequencedEvent[] = [
        { type: 'toolCallStarted', t, sessionId, agentId, call, tokens: a.tokens + tokensDelta, outputTokens: a.outputTokens + outDelta },
      ];
      if (opts.file) {
        out.push({ type: 'fileTouched', t, sessionId, agentId, path: opts.file.path, op: opts.file.op, source: opts.file.bash ? 'bash' : 'tool', toolUseId: id });
      }
      return out;
    });
    const end = rel + (opts.dur ?? this.rng.int(300, 2600));
    this.at(end, () => [{ type: 'toolCallFinished', t: this.base + end, sessionId, agentId, toolUseId: id, isError: !!opts.error }]);
  }

  /** A run of plausible tool calls between two relative times. */
  work(sessionId: string, agentId: string, from: number, to: number, files: readonly string[], cwd: string, opts: { gap?: [number, number]; writes?: boolean } = {}): void {
    const [g0, g1] = opts.gap ?? [2500, 7000];
    let t = from + this.rng.int(400, 1600);
    while (t < to - 1200) {
      const kind = this.rng.int(0, 9);
      const path = this.rng.pick(files);
      const rel = path.startsWith(cwd + '/') ? path.slice(cwd.length + 1) : path;
      const dur = Math.min(this.rng.int(300, 2400), to - t - 600);
      if (kind <= 3) this.tool(t, sessionId, agentId, 'Bash', this.rng.pick(BASH), { dur });
      else if (kind <= 5) this.tool(t, sessionId, agentId, 'Read', rel, { dur, file: { path, op: 'read' } });
      else if (kind === 6) this.tool(t, sessionId, agentId, 'Grep', this.rng.pick(GREP), { dur });
      else if (kind === 7 && opts.writes !== false) this.tool(t, sessionId, agentId, 'Edit', rel, { dur, file: { path, op: 'write' } });
      else if (kind === 8 && opts.writes !== false) this.tool(t, sessionId, agentId, 'Write', rel, { dur, file: { path, op: 'write' } });
      else this.tool(t, sessionId, agentId, 'Bash', `cat ${rel}`, { dur, file: { path, op: 'read', bash: true } });
      t += this.rng.int(g0, g1);
    }
  }

  run(rel: number, run: WorkflowRun): void {
    this.at(rel, () => [{ type: 'runStarted', t: this.base + rel, sessionId: run.sessionId, run }]);
  }

  finalize(rel: number, sessionId: string, runId: string, patch: (run: WorkflowRun, agents: Agent[]) => { run: Partial<WorkflowRun>; agents: Agent[] }): void {
    const t = this.base + rel;
    this.at(rel, (snap) => {
      const s = snap.sessions[sessionId];
      const run = s?.runs[runId];
      if (!run) return [];
      const agents = run.agentIds.map((id) => s.agents[id]).filter(Boolean);
      const p = patch(run, agents);
      const next = { ...run, ...p.run };
      const map = { ...s.agents };
      for (const a of p.agents) map[a.agentId] = a;
      next.phases = withPhaseStates(next, map);
      return [{ type: 'runFinalized', t, sessionId, run: next, agents: p.agents }];
    });
  }

  build(): VirgilEvent[] {
    const sorted = [...this.actions].sort((a, b) => a.t - b.t || a.order - b.order);
    let snap = emptySnapshot('live');
    const out: VirgilEvent[] = [];
    let seq = 0;
    for (const action of sorted) {
      for (const e of action.run(snap)) {
        const ev = { ...e, seq: ++seq } as VirgilEvent;
        snap = applyEvents(snap, [ev]);
        out.push(ev);
      }
    }
    return out;
  }
}

const S = 1000;

interface WfAgentPlan {
  label: string;
  phase: number;
  model: string;
  start: number;
  end: number;
  task: string;
  fail?: string;
  retryOf?: string;
}

/**
 * Build the scenario's events. `base` is the epoch time of scenario time 0. Event `seq` numbers
 * run 1..n and are renumbered by the source.
 */
export function buildScenario(base: number): VirgilEvent[] {
  const b = new Builder(base);
  const T = (s: number) => base + s * S;

  // ---------- session 1: orchard, busy, with a workflow run ----------
  const s1 = SESSION_ORCHARD;
  const P = -PREHISTORY;
  b.session(P, makeSessionInfo({
    sessionId: s1, pid: 48211, cwd: ORCHARD, projectSlug: '-home-dev-projects-orchard', name: 'orchard',
    title: 'Build the stock screen', status: 'busy', startedAt: T(-1260), updatedAt: T(0), statusUpdatedAt: T(0),
  }));
  b.spawn(P, {
    agentId: MAIN_AGENT_ID, sessionId: s1, kind: 'main', label: 'Build the stock screen', model: 'claude-opus-5-5',
    task: 'Add a stock screen to the orchard app: list every variety with its count, let people edit counts inline, and cover the API with tests.',
    startedAt: T(-1260), lastActivityAt: T(-1260),
  });
  b.work(s1, MAIN_AGENT_ID, -1250 * S, -30 * S, ORCHARD_FILES, ORCHARD, { gap: [12_000, 50_000] });
  b.work(s1, MAIN_AGENT_ID, 1 * S, 29 * S, ORCHARD_FILES, ORCHARD, { gap: [3000, 6000] });

  // Explorer with a nested agent.
  const explore = b.agentId();
  const nested = b.agentId();
  b.tool(4 * S, s1, MAIN_AGENT_ID, 'Agent', 'Map the stock code', { dur: 600 });
  b.spawn(4.6 * S, {
    agentId: explore, sessionId: s1, agentType: 'Explore', label: 'Map the stock code', model: 'claude-haiku-4-5',
    task: 'Find every file that reads or writes stock counts and describe how data flows from the API to the screen.',
    spawnToolUseId: 'toolu_mock_explore',
  });
  b.work(s1, explore, 5 * S, 11 * S, ORCHARD_FILES, ORCHARD, { writes: false, gap: [1200, 2600] });
  b.tool(11 * S, s1, explore, 'Agent', 'Read the data layer', { dur: 500 });
  b.spawn(11.6 * S, {
    agentId: nested, sessionId: s1, parentAgentId: explore, spawnDepth: 2, agentType: 'general-purpose',
    label: 'Read the data layer', model: 'claude-sonnet-5-5',
    task: 'Read src/stock/store.ts and src/stock/model.ts and list the public functions with their argument types.',
  });
  b.update(12 * S, s1, explore, { state: 'waiting' });
  b.work(s1, nested, 12 * S, 34 * S, ORCHARD_FILES.slice(0, 6), ORCHARD, { writes: false, gap: [1800, 4000] });
  b.finish(34 * S, s1, nested, 'done', { resultPreview: 'Six public functions; the store keeps counts in a Map keyed by variety id.' });
  b.update(34.5 * S, s1, explore, { state: 'running' });
  b.work(s1, explore, 35 * S, 41 * S, ORCHARD_FILES, ORCHARD, { writes: false, gap: [1500, 2500] });
  b.finish(41 * S, s1, explore, 'done', { resultPreview: 'Data flows api.ts → store.ts → StockScreen.svelte; StockRow edits go back through store.update().' });

  // The workflow run.
  const runStart = 30 * S;
  const phases: Phase[] = [
    { index: 1, title: 'Plan', detail: 'one outline for everybody', model: 'opus', state: 'pending' },
    { index: 2, title: 'Build', detail: 'one agent per module', model: 'sonnet', state: 'pending' },
    { index: 3, title: 'Review', detail: 'independent reviewers', model: null, state: 'pending' },
    { index: 4, title: 'Fix', detail: 'apply the review findings', model: 'opus', state: 'pending' },
  ];
  b.tool(29.4 * S, s1, MAIN_AGENT_ID, 'Workflow', 'orchard-build', { dur: 500 });
  b.run(runStart, makeRun({
    runId: DEMO_RUN_ID, sessionId: s1, name: 'orchard-build', summary: 'Plan, build, review and fix the stock screen',
    toolUseId: 'toolu_mock_workflow', taskId: 'w4f1c2', startedAt: T(30), phases,
  }));
  b.update(30.2 * S, s1, MAIN_AGENT_ID, { state: 'waiting' });

  const plan: WfAgentPlan[] = [
    { label: 'plan:outline', phase: 1, model: 'claude-opus-5-5', start: 32, end: 72, task: 'Write one outline for the stock screen that every build agent follows: modules, file names, and the public API.' },
    { label: 'plan:risks', phase: 1, model: 'claude-opus-5-5', start: 33, end: 80, task: 'List the risks in changing the stock store and how each build agent should avoid them.' },
    { label: 'build:api', phase: 2, model: 'claude-sonnet-5-5', start: 82, end: 160, task: 'Implement src/stock/api.ts following the outline, with typed errors for network failures.' },
    { label: 'build:model', phase: 2, model: 'claude-sonnet-5-5', start: 84, end: 140, task: 'Implement the stock model types and validation in src/stock/model.ts.' },
    { label: 'build:store', phase: 2, model: 'claude-sonnet-5-5', start: 86, end: 176, task: 'Extend the store with inline count edits and optimistic updates.' },
    { label: 'build:screen', phase: 2, model: 'claude-sonnet-5-5', start: 88, end: 196, task: 'Build StockScreen.svelte: a sortable list of varieties with counts.' },
    { label: 'build:row', phase: 2, model: 'claude-sonnet-5-5', start: 90, end: 150, fail: 'Edit failed: the outline names a component that does not exist', task: 'Build StockRow.svelte with an inline editable count field.' },
    { label: 'build:format', phase: 2, model: 'claude-haiku-4-5', start: 92, end: 122, task: 'Add count and date formatting helpers to src/lib/format.ts.' },
    { label: 'build:tests', phase: 2, model: 'claude-sonnet-5-5', start: 94, end: 190, task: 'Cover the stock API and store with unit tests.' },
    { label: 'build:docs', phase: 2, model: 'claude-haiku-4-5', start: 96, end: 132, task: 'Document the stock screen in docs/stock.md.' },
    { label: 'review:api', phase: 3, model: 'claude-fable-5-2', start: 198, end: 236, task: 'Review the API module for error handling and naming.' },
    { label: 'review:store', phase: 3, model: 'claude-fable-5-2', start: 200, end: 248, task: 'Review the store changes for race conditions in optimistic updates.' },
    { label: 'review:screen', phase: 3, model: 'claude-sonnet-5-5', start: 202, end: 226, task: 'Review the screen for accessibility: focus order, labels and contrast.' },
    { label: 'review:tests', phase: 3, model: 'claude-fable-5-2', start: 204, end: 252, task: 'Check that the tests cover failure paths, not only the happy path.' },
    { label: 'review:docs', phase: 3, model: 'claude-haiku-4-5', start: 206, end: 222, task: 'Check the docs against the code.' },
    { label: 'review:perf', phase: 3, model: 'claude-fable-5-2', start: 208, end: 216, task: 'Look for needless re-renders in the stock list.' },
    { label: 'review:perf (after usage limit)', phase: 3, model: 'claude-fable-5-2', start: 228, end: 258, retryOf: 'review:perf', task: 'Look for needless re-renders in the stock list.' },
    { label: 'fix:store', phase: 4, model: 'claude-opus-5-5', start: 262, end: 304, task: 'Apply the review findings to the store.' },
    { label: 'fix:row', phase: 4, model: 'claude-opus-5-5', start: 264, end: 316, task: 'Rebuild StockRow.svelte, which failed in the build phase.' },
    { label: 'fix:tests', phase: 4, model: 'claude-sonnet-5-5', start: 266, end: 298, task: 'Add the missing failure-path tests.' },
    { label: 'fix:screen', phase: 4, model: 'claude-sonnet-5-5', start: 268, end: 310, task: 'Fix the focus order and labels the reviewer found.' },
  ];

  const byLabel = new Map<string, string>();
  for (const p of plan) {
    const id = b.agentId();
    byLabel.set(p.label, id);
    const ph = phases[p.phase - 1];
    b.spawn(p.start * S, {
      agentId: id, sessionId: s1, kind: 'workflow', agentType: 'workflow-subagent', label: p.label, task: p.task,
      model: p.model, runId: DEMO_RUN_ID, phaseIndex: ph.index, phaseTitle: ph.title, spawnToolUseId: 'toolu_mock_workflow',
    });
    // The workflow agents share a few files, so the shared-files overlay has something to show.
    const files = ORCHARD_FILES.filter((_, i) => (i + p.phase + p.start) % 3 !== 0);
    if (p.retryOf) {
      b.finish(p.start * S, s1, byLabel.get(p.retryOf)!, 'failed', { stateReason: 'retried', superseded: true });
    }
    const stop = p.label === 'review:perf' ? p.start + 7 : p.end;
    b.work(s1, id, p.start * S, stop * S, files, ORCHARD, { gap: [2400, 6400] });
    if (p.label === 'review:perf') continue; // abandoned attempt: ends when the retry starts
    if (p.fail) {
      b.tool((p.end - 1.5) * S, s1, id, 'Edit', 'src/screens/StockRow.svelte', { dur: 400, error: true, file: { path: `${ORCHARD}/src/screens/StockRow.svelte`, op: 'write' } });
      b.finish(p.end * S, s1, id, 'failed', { stateReason: 'failed' });
    } else {
      b.finish(p.end * S, s1, id, 'done', { resultPreview: `${p.label}: finished. ${p.task.split(' ').slice(0, 6).join(' ')}… done.` });
    }
  }

  // A plain subagent in its own git worktree, launched while the reviews run.
  const notesAgent = b.agentId();
  b.tool(199 * S, s1, MAIN_AGENT_ID, 'Agent', 'Write release notes', { dur: 500 });
  b.spawn(199.6 * S, {
    agentId: notesAgent, sessionId: s1, agentType: 'general-purpose', label: 'Write release notes', model: 'claude-sonnet-5-5',
    task: 'Draft release notes for the stock screen in a separate worktree, based on the outline.',
    worktreePath: `${ORCHARD}/.worktrees/release-notes`,
  });
  b.work(s1, notesAgent, 200 * S, 256 * S, [`${ORCHARD}/.worktrees/release-notes/CHANGELOG.md`, `${ORCHARD}/README.md`], ORCHARD);
  b.finish(256 * S, s1, notesAgent, 'done', { resultPreview: 'Drafted CHANGELOG.md with four entries.' });

  // Finalisation: the run file arrives.
  b.finalize(322 * S, s1, DEMO_RUN_ID, (run, agents) => {
    const finalAgents = agents.map((a) => {
      const p = plan.find((x) => byLabel.get(x.label) === a.agentId)!;
      if (a.superseded) return a;
      return {
        ...a,
        queuedAt: T(p.phase === 1 ? 31 : p.start - 2),
        attempt: p.retryOf ? 2 : 1,
        tokens: Math.round(a.tokens * 0.97),
        endedAt: T(p.end),
      };
    });
    const totalTokens = finalAgents.reduce((n, a) => n + a.tokens, 0);
    const totalToolCalls = finalAgents.reduce((n, a) => n + a.toolCalls, 0);
    return {
      run: {
        status: 'completed', final: true, endedAt: T(322), totalTokens, totalToolCalls,
        declaredAgentCount: finalAgents.filter((a) => !a.superseded).length, defaultModel: 'claude-sonnet-5-5',
      },
      agents: finalAgents,
    };
  });
  b.update(323 * S, s1, MAIN_AGENT_ID, { state: 'running' });
  b.work(s1, MAIN_AGENT_ID, 323 * S, 338 * S, ORCHARD_FILES, ORCHARD, { gap: [2500, 4000] });
  b.sessionPatch(340 * S, s1, { status: 'idle', statusUpdatedAt: T(340), updatedAt: T(340) });
  b.update(340 * S, s1, MAIN_AGENT_ID, { state: 'waiting' });

  // ---------- session 2: notes, mostly idle ----------
  const s2 = SESSION_NOTES;
  b.session(P, makeSessionInfo({
    sessionId: s2, pid: 51907, cwd: NOTES, projectSlug: '-home-dev-sites-notes', name: 'notes',
    title: 'Add an RSS feed', status: 'idle', startedAt: T(-5400), updatedAt: T(-600), statusUpdatedAt: T(-600),
  }));
  b.spawn(P, {
    agentId: MAIN_AGENT_ID, sessionId: s2, kind: 'main', label: 'Add an RSS feed', model: 'claude-sonnet-5-5', state: 'waiting',
    task: 'Add an RSS feed to the notes site and link it from the home page.', startedAt: T(-5400), lastActivityAt: T(-5400),
  });
  b.work(s2, MAIN_AGENT_ID, -1700 * S, -620 * S, NOTES_FILES, NOTES, { gap: [25_000, 90_000] });
  b.sessionPatch(60 * S, s2, { status: 'busy', statusUpdatedAt: T(60), updatedAt: T(60) });
  b.update(60 * S, s2, MAIN_AGENT_ID, { state: 'running' });
  b.work(s2, MAIN_AGENT_ID, 61 * S, 70 * S, NOTES_FILES, NOTES);
  const planner = b.agentId();
  b.tool(70 * S, s2, MAIN_AGENT_ID, 'Agent', 'Plan the RSS feed', { dur: 500 });
  b.spawn(70.6 * S, {
    agentId: planner, sessionId: s2, agentType: 'Plan', label: 'Plan the RSS feed', model: 'claude-opus-5-5',
    task: 'Plan how to generate an RSS feed from the notes collection at build time.',
  });
  b.update(71 * S, s2, MAIN_AGENT_ID, { state: 'waiting' });
  b.work(s2, planner, 71 * S, 110 * S, NOTES_FILES, NOTES, { writes: false });
  b.finish(110 * S, s2, planner, 'done', { resultPreview: 'Use the content collection; write /rss.xml in an endpoint.' });
  b.update(110.5 * S, s2, MAIN_AGENT_ID, { state: 'running' });
  b.work(s2, MAIN_AGENT_ID, 111 * S, 124 * S, NOTES_FILES, NOTES);
  b.sessionPatch(125 * S, s2, { status: 'idle', statusUpdatedAt: T(125), updatedAt: T(125) });
  b.update(125 * S, s2, MAIN_AGENT_ID, { state: 'waiting' });
  b.sessionPatch(240 * S, s2, { status: 'busy', statusUpdatedAt: T(240), updatedAt: T(240) });
  b.update(240 * S, s2, MAIN_AGENT_ID, { state: 'running' });
  b.work(s2, MAIN_AGENT_ID, 240 * S, 280 * S, NOTES_FILES, NOTES);
  b.sessionPatch(281 * S, s2, { status: 'idle', statusUpdatedAt: T(281), updatedAt: T(281) });
  b.update(281 * S, s2, MAIN_AGENT_ID, { state: 'waiting' });

  // ---------- session 3: stale pid file ----------
  const s3 = SESSION_SCRATCH;
  b.session(P, makeSessionInfo({
    sessionId: s3, pid: 30122, liveness: 'stale', cwd: '/home/dev/scratch', projectSlug: '-home-dev-scratch', name: 'scratch',
    title: null, status: 'busy', startedAt: T(-14_400), updatedAt: T(-10_800), statusUpdatedAt: T(-10_800),
  }));
  b.spawn(P, {
    agentId: MAIN_AGENT_ID, sessionId: s3, kind: 'main', label: 'scratch', state: 'running',
    model: 'claude-opus-5-5', task: 'Try out a parser for log lines.', startedAt: T(-14_400), lastActivityAt: T(-14_400),
  });
  b.work(s3, MAIN_AGENT_ID, -14_300 * S, -10_800 * S, ['/home/dev/scratch/parse.py', '/home/dev/scratch/sample.log'], '/home/dev/scratch', { gap: [60_000, 240_000] });
  b.finish(-10_800 * S, s3, MAIN_AGENT_ID, 'stopped', { stateReason: 'sessionGone' });

  return b.build();
}

/** The snapshot of the scenario at absolute time `t` (all events with event.t <= t). */
export function scenarioAt(events: readonly VirgilEvent[], t: number, mode: 'live' | 'replay' = 'live'): Snapshot {
  const upto = events.filter((e) => e.t <= t);
  return { ...applyEvents(emptySnapshot(mode), upto), mode };
}
