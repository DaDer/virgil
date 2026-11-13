// Synthetic sessions with 1, 10 and 60 agents for view tests (ux-spec 6.3). Invented data only.

import { MAIN_AGENT_ID, type Agent, type AgentState, type Session, type WorkflowRun } from '../../contract';
import { makeAgent, makeRun, makeSessionInfo, withPhaseStates } from '../../data/factories';

export const T0 = Date.UTC(2026, 10, 12, 10, 0, 0);
export const SID = '11111111-2222-4333-8444-555555555555';

function session(agents: Agent[], runs: WorkflowRun[] = []): Session {
  const main = makeAgent({ agentId: MAIN_AGENT_ID, sessionId: SID, kind: 'main', label: 'main', startedAt: T0, state: 'running' });
  const all: Record<string, Agent> = { [MAIN_AGENT_ID]: main };
  for (const a of agents) all[a.agentId] = a;
  const runMap: Record<string, WorkflowRun> = {};
  for (const r of runs) runMap[r.runId] = { ...r, phases: withPhaseStates(r, all) };
  return {
    ...makeSessionInfo({ sessionId: SID, name: 'orchard', cwd: '/home/dev/orchard', status: 'busy', startedAt: T0 }),
    agents: all,
    runs: runMap,
    files: {},
  };
}

/** One plain agent under main. */
export function oneAgent(): Session {
  return session([makeAgent({ agentId: 'a1', sessionId: SID, label: 'explore', startedAt: T0 + 5_000, state: 'running', toolCalls: 3 })]);
}

/** Ten plain agents: eight under main, two nested under the first. */
export function tenAgents(): Session {
  const agents: Agent[] = [];
  for (let i = 0; i < 10; i++) {
    const nested = i >= 8;
    agents.push(
      makeAgent({
        agentId: `a${i}`,
        sessionId: SID,
        label: `agent ${i}`,
        parentAgentId: nested ? 'a0' : MAIN_AGENT_ID,
        spawnDepth: nested ? 2 : 1,
        startedAt: T0 + i * 10_000,
        endedAt: i % 3 === 0 ? T0 + i * 10_000 + 60_000 : null,
        state: i % 3 === 0 ? 'done' : 'running',
      }),
    );
  }
  return session(agents);
}

const PHASES = [
  { title: 'Spec', n: 6 },
  { title: 'Engine', n: 6 },
  { title: 'Screen', n: 18 },
  { title: 'Checks', n: 19 },
  { title: 'Fix', n: 8 },
  { title: 'Docs', n: 1 },
];

/** A six-phase workflow run of 58 agents plus two plain agents: 60 in all. */
export function sixtyAgents(runId = 'wf_00000000-001'): Session {
  const agents: Agent[] = [];
  let k = 0;
  PHASES.forEach((p, pi) => {
    for (let i = 0; i < p.n; i++, k++) {
      const state: AgentState = pi < 2 ? 'done' : pi === 2 ? (i < 12 ? 'done' : 'running') : pi === 3 ? (i < 2 ? 'running' : 'queued') : 'queued';
      const start = T0 + 60_000 + pi * 600_000 + i * 5_000;
      agents.push(
        makeAgent({
          agentId: `w${k}`,
          sessionId: SID,
          kind: 'workflow',
          agentType: 'workflow-subagent',
          label: `${p.title.toLowerCase()}:${i}`,
          runId,
          phaseIndex: pi + 1,
          phaseTitle: p.title,
          queuedAt: T0 + 60_000,
          startedAt: state === 'queued' ? null : start,
          endedAt: state === 'done' ? start + 300_000 : null,
          state,
          tokens: 40_000 + k * 1000,
          toolCalls: k % 20,
        }),
      );
    }
  });
  agents.push(makeAgent({ agentId: 'p1', sessionId: SID, label: 'plain one', startedAt: T0 + 10_000, endedAt: T0 + 40_000, state: 'done' }));
  agents.push(makeAgent({ agentId: 'p2', sessionId: SID, label: 'plain two', startedAt: T0 + 20_000, state: 'running' }));
  const run = makeRun({
    runId,
    sessionId: SID,
    name: 'build',
    startedAt: T0 + 60_000,
    phases: PHASES.map((p, i) => ({ index: i + 1, title: p.title, detail: null, model: null, state: 'pending' as const })),
    agentIds: agents.filter((a) => a.runId === runId).map((a) => a.agentId),
  });
  return session(agents, [run]);
}
