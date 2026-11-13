// Builders for contract objects with every field filled. Used by the mock scenario and tests.

import {
  MAIN_AGENT_ID,
  type Agent,
  type AgentState,
  type Phase,
  type PhaseState,
  type SessionInfo,
  type WorkflowRun,
} from '../contract';

export function makeSessionInfo(p: Partial<SessionInfo> & { sessionId: string }): SessionInfo {
  return {
    pid: null,
    liveness: 'alive',
    cwd: null,
    projectSlug: null,
    name: null,
    title: null,
    status: 'idle',
    kind: 'interactive',
    version: '2.1.323',
    startedAt: null,
    updatedAt: null,
    statusUpdatedAt: null,
    hasTranscript: true,
    ...p,
  };
}

export function makeAgent(p: Partial<Agent> & { agentId: string; sessionId: string }): Agent {
  const isMain = p.agentId === MAIN_AGENT_ID;
  return {
    kind: isMain ? 'main' : 'subagent',
    parentAgentId: isMain ? null : MAIN_AGENT_ID,
    spawnToolUseId: null,
    spawnDepth: isMain ? 0 : 1,
    agentType: null,
    name: null,
    label: p.agentId,
    task: '',
    model: null,
    state: 'running',
    stateReason: null,
    runId: null,
    phaseIndex: null,
    phaseTitle: null,
    attempt: null,
    superseded: false,
    queuedAt: null,
    startedAt: null,
    endedAt: null,
    lastActivityAt: null,
    tokens: 0,
    outputTokens: 0,
    toolCalls: 0,
    lastTool: null,
    recentTools: [],
    resultPreview: null,
    worktreePath: null,
    ...p,
  };
}

export function makeRun(p: Partial<WorkflowRun> & { runId: string; sessionId: string }): WorkflowRun {
  return {
    name: p.runId,
    summary: null,
    status: 'running',
    final: false,
    parentAgentId: MAIN_AGENT_ID,
    toolUseId: null,
    taskId: null,
    startedAt: null,
    endedAt: null,
    phases: [],
    agentIds: [],
    totalTokens: null,
    totalToolCalls: null,
    declaredAgentCount: null,
    defaultModel: null,
    error: null,
    ...p,
  };
}

const ACTIVE: readonly AgentState[] = ['queued', 'running', 'waiting'];

/** Phase state over its agents that are not superseded (data-contract 2.6). */
export function phaseState(agents: readonly Agent[], runTerminal: boolean): PhaseState {
  const live = agents.filter((a) => !a.superseded);
  if (live.length === 0) return runTerminal ? 'done' : 'pending';
  if (live.some((a) => ACTIVE.includes(a.state))) return 'running';
  if (live.some((a) => a.state === 'failed')) return 'failed';
  return 'done';
}

export function withPhaseStates(run: WorkflowRun, agents: Record<string, Agent>): Phase[] {
  const terminal = run.status !== 'running';
  return run.phases.map((ph) => {
    const own = run.agentIds.map((id) => agents[id]).filter((a): a is Agent => !!a && a.phaseIndex === ph.index);
    return { ...ph, state: phaseState(own, terminal) };
  });
}
