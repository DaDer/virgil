// Selection and hover (ux-spec 1.4). One thing at a time; selecting opens the Inspector.
//
// Ids by kind:
//   session  sessionId
//   agent    agentId (sessionId given separately; agent ids are unique only within a session)
//   run      runId
//   phase    `${runId}:${index}`  (use phaseKey from $lib/data/derive)
//   file     normalised absolute path

export type SelectionKind = 'session' | 'agent' | 'run' | 'phase' | 'file';

export interface Selection {
  kind: SelectionKind;
  id: string;
  /** The session the thing belongs to. Required for agent, run, phase and file. */
  sessionId?: string;
}

export const sel = {
  session: (sessionId: string): Selection => ({ kind: 'session', id: sessionId, sessionId }),
  agent: (sessionId: string, agentId: string): Selection => ({ kind: 'agent', id: agentId, sessionId }),
  run: (sessionId: string, runId: string): Selection => ({ kind: 'run', id: runId, sessionId }),
  phase: (sessionId: string, runId: string, index: number): Selection => ({ kind: 'phase', id: `${runId}:${index}`, sessionId }),
  file: (sessionId: string, path: string): Selection => ({ kind: 'file', id: path, sessionId }),
};

export function sameSelection(a: Selection | null, b: Selection | null): boolean {
  if (!a || !b) return a === b;
  return a.kind === b.kind && a.id === b.id && (a.sessionId ?? null) === (b.sessionId ?? null);
}

class SelectionStore {
  current = $state.raw<Selection | null>(null);

  select(s: Selection | null): void {
    if (!sameSelection(this.current, s)) this.current = s;
  }

  clear(): void {
    this.current = null;
  }

  /** True when `kind`/`id` (and the session, when given) is the current selection. */
  is(kind: SelectionKind, id: string, sessionId?: string): boolean {
    const c = this.current;
    return !!c && c.kind === kind && c.id === id && (sessionId === undefined || c.sessionId === undefined || c.sessionId === sessionId);
  }
}

class HoverStore {
  current = $state.raw<Selection | null>(null);

  set(s: Selection | null): void {
    if (!sameSelection(this.current, s)) this.current = s;
  }

  clear(): void {
    this.current = null;
  }

  is(kind: SelectionKind, id: string, sessionId?: string): boolean {
    const c = this.current;
    return !!c && c.kind === kind && c.id === id && (sessionId === undefined || c.sessionId === undefined || c.sessionId === sessionId);
  }
}

export const selection = new SelectionStore();
export const hover = new HoverStore();
