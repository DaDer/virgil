// Fixed labels, icons and colour-token families for states and agent types (ux-spec 1.2, 1.3).
// A "tone" names a token family: --state-<tone>, --state-<tone>-text, --state-<tone>-soft,
// and the CSS helper class .st-<tone>.

import type { AgentState, PhaseState, RunStatus, Session } from './contract';

export type Tone = 'queued' | 'running' | 'done' | 'error' | 'stopped' | 'idle';
export type StateIconName = 'queued' | 'running' | 'waiting' | 'done' | 'error' | 'stopped' | 'unknown' | 'idle' | 'pending';

export interface StateMeta {
  label: string;
  tone: Tone;
  icon: StateIconName;
}

/**
 * Agent states. The contract has both `queued` (not started) and `waiting` (started, paused on
 * children or messages); the spec's single "Waiting" row is split: queued reads "Queued" with the
 * clock icon, waiting reads "Waiting" with an hourglass. Both use the queued (amber) tone.
 */
export const AGENT_STATE: Record<AgentState, StateMeta> = {
  queued: { label: 'Queued', tone: 'queued', icon: 'queued' },
  running: { label: 'Running', tone: 'running', icon: 'running' },
  waiting: { label: 'Waiting', tone: 'queued', icon: 'waiting' },
  done: { label: 'Done', tone: 'done', icon: 'done' },
  failed: { label: 'Failed', tone: 'error', icon: 'error' },
  stopped: { label: 'Stopped', tone: 'stopped', icon: 'stopped' },
};

export const RUN_STATUS: Record<RunStatus, StateMeta> = {
  running: { label: 'Running', tone: 'running', icon: 'running' },
  completed: { label: 'Done', tone: 'done', icon: 'done' },
  failed: { label: 'Failed', tone: 'error', icon: 'error' },
  stopped: { label: 'Stopped', tone: 'stopped', icon: 'stopped' },
};

export const PHASE_STATE: Record<PhaseState, StateMeta> = {
  pending: { label: 'Not started', tone: 'idle', icon: 'pending' },
  running: { label: 'Running', tone: 'running', icon: 'running' },
  done: { label: 'Done', tone: 'done', icon: 'done' },
  failed: { label: 'Failed', tone: 'error', icon: 'error' },
};

export type SessionState = 'busy' | 'waiting' | 'idle' | 'stale' | 'ended';

export const SESSION_STATE: Record<SessionState, StateMeta> = {
  busy: { label: 'Busy', tone: 'running', icon: 'running' },
  waiting: { label: 'Waiting', tone: 'queued', icon: 'waiting' },
  idle: { label: 'Idle', tone: 'idle', icon: 'idle' },
  ended: { label: 'Ended', tone: 'idle', icon: 'done' },
  stale: { label: 'Not running', tone: 'idle', icon: 'unknown' },
};

export const UNKNOWN_STATE: StateMeta = { label: 'Unknown', tone: 'idle', icon: 'unknown' };

export function sessionState(s: Pick<Session, 'liveness' | 'status'>): SessionState {
  if (s.liveness === 'stale') return 'stale';
  // `shell`: a shell command is running, which is work too.
  if (s.status === 'busy' || s.status === 'shell') return 'busy';
  // `waiting`: Claude Code asks the person something (a permission, a question).
  if (s.status === 'waiting') return 'waiting';
  return 'idle';
}

export function isActive(state: AgentState): boolean {
  return state === 'queued' || state === 'running' || state === 'waiting';
}

// ---------- agent types ----------

export type TypeKey = 'session' | 'workflow' | 'general' | 'fork' | 'explore' | 'plan' | 'custom';
export type GlyphName = 'type-session' | 'type-workflow' | 'type-general' | 'type-fork' | 'type-explore' | 'type-plan' | 'type-custom';

export interface TypeMeta {
  key: TypeKey;
  label: string;
  glyph: GlyphName;
}

export function agentTypeMeta(agentType: string | null | undefined, kind?: 'main' | 'subagent' | 'workflow' | 'session'): TypeMeta {
  if (kind === 'main' || kind === 'session') return { key: 'session', label: 'Session', glyph: 'type-session' };
  if (kind === 'workflow' || agentType === 'workflow-subagent') return { key: 'workflow', label: 'Workflow agent', glyph: 'type-workflow' };
  switch (agentType) {
    case 'general-purpose':
      return { key: 'general', label: 'General agent', glyph: 'type-general' };
    case 'fork':
      return { key: 'fork', label: 'Fork', glyph: 'type-fork' };
    case 'Explore':
      return { key: 'explore', label: 'Explorer', glyph: 'type-explore' };
    case 'Plan':
      return { key: 'plan', label: 'Planner', glyph: 'type-plan' };
    case null:
    case undefined:
    case '':
      return { key: 'general', label: 'General agent', glyph: 'type-general' };
    default:
      return { key: 'custom', label: agentType, glyph: 'type-custom' };
  }
}
