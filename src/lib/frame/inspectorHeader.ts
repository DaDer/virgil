// What the Inspector header shows for the current selection: title, glyph and state.

import { type AgentKind } from '../contract';
import { basename } from '../format';
import { sessionTitle } from '../data/derive';
import { AGENT_STATE, PHASE_STATE, RUN_STATUS, SESSION_STATE, sessionState, type StateMeta } from '../states';
import type { Selection } from '../stores/selection.svelte';
import { world } from '../stores/world.svelte';

export interface InspectorHeader {
  title: string;
  glyph: { kind: AgentKind | 'session'; agentType: string | null } | null;
  icon: 'run' | 'workflow' | 'file' | null;
  state: StateMeta | null;
  attempt: number | null;
}

export function inspectorHeader(s: Selection | null): InspectorHeader {
  const none: InspectorHeader = { title: 'Inspector', glyph: null, icon: null, state: null, attempt: null };
  if (!s) return none;
  const session = world.session(s.sessionId ?? (s.kind === 'session' ? s.id : null));
  switch (s.kind) {
    case 'session':
      return session
        ? { title: sessionTitle(session), glyph: { kind: 'session', agentType: null }, icon: null, state: SESSION_STATE[sessionState(session)], attempt: null }
        : { ...none, title: 'Session' };
    case 'agent': {
      const a = session?.agents[s.id];
      return a
        ? { title: a.label, glyph: { kind: a.kind, agentType: a.agentType }, icon: null, state: AGENT_STATE[a.state], attempt: a.attempt }
        : { ...none, title: s.id };
    }
    case 'run': {
      const r = session?.runs[s.id];
      return r ? { title: r.name, glyph: null, icon: 'run', state: RUN_STATUS[r.status], attempt: null } : { ...none, title: s.id };
    }
    case 'phase': {
      const p = world.phaseByKey(s.sessionId, s.id);
      return p ? { title: `${p.index} ${p.title}`, glyph: null, icon: 'workflow', state: PHASE_STATE[p.state], attempt: null } : { ...none, title: 'Phase' };
    }
    case 'file':
      return { title: basename(s.id), glyph: null, icon: 'file', state: null, attempt: null };
  }
}
