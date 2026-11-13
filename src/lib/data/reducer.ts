// The reducer of data-contract section 3.3, as a pure function.
// `applyEvents` never mutates its input: every object it changes is copied once per call
// (copy on write), everything else is shared, so derived values can compare by reference.

import {
  RECENT_TOOLS_MAX,
  type Agent,
  type VirgilEvent,
  type FileEntry,
  type Session,
  type Snapshot,
  type SourceMode,
} from '../contract';

export function emptySnapshot(mode: SourceMode = 'live'): Snapshot {
  return { seq: 0, t: 0, mode, sessions: {} };
}

class Draft {
  private owned = new WeakSet<object>();
  snap: Snapshot;

  constructor(base: Snapshot) {
    this.snap = base;
  }

  private own<T extends object>(o: T): T {
    if (this.owned.has(o)) return o;
    const c = (Array.isArray(o) ? [...o] : { ...o }) as T;
    this.owned.add(c);
    return c;
  }

  root(): Snapshot {
    return (this.snap = this.own(this.snap));
  }

  sessions(): Record<string, Session> {
    const r = this.root();
    return (r.sessions = this.own(r.sessions));
  }

  session(id: string): Session | null {
    if (!this.snap.sessions[id]) return null;
    const all = this.sessions();
    return (all[id] = this.own(all[id]));
  }

  agents(s: Session): Record<string, Agent> {
    return (s.agents = this.own(s.agents));
  }

  agent(s: Session, id: string): Agent | null {
    if (!s.agents[id]) return null;
    const all = this.agents(s);
    return (all[id] = this.own(all[id]));
  }

  runs(s: Session) {
    return (s.runs = this.own(s.runs));
  }

  files(s: Session): Record<string, FileEntry> {
    return (s.files = this.own(s.files));
  }

  adopt<T extends object>(o: T): T {
    this.owned.add(o);
    return o;
  }

  isOwned(o: object): boolean {
    return this.owned.has(o);
  }
}

function applyOne(d: Draft, e: VirgilEvent): void {
  if (e.type === 'snapshotReset') {
    // The reset snapshot is taken as is; its seq equals e.seq.
    d.snap = { ...e.snapshot, seq: e.seq };
    return;
  }
  const r = d.root();
  r.seq = e.seq;
  r.t = Math.max(r.t, e.t);

  switch (e.type) {
    case 'sessionUpserted': {
      const all = d.sessions();
      const prev = all[e.session.sessionId];
      all[e.session.sessionId] = d.adopt(
        prev
          ? { ...e.session, agents: prev.agents, runs: prev.runs, files: prev.files }
          : { ...e.session, agents: {}, runs: {}, files: {} },
      );
      return;
    }
    case 'sessionRemoved': {
      if (!r.sessions[e.sessionId]) return;
      delete d.sessions()[e.sessionId];
      return;
    }
    case 'agentSpawned': {
      const s = d.session(e.sessionId);
      if (!s) return;
      d.agents(s)[e.agent.agentId] = e.agent;
      const runId = e.agent.runId;
      if (runId && s.runs[runId] && !s.runs[runId].agentIds.includes(e.agent.agentId)) {
        const runs = d.runs(s);
        runs[runId] = { ...runs[runId], agentIds: [...runs[runId].agentIds, e.agent.agentId] };
      }
      return;
    }
    case 'agentUpdated': {
      const s = d.session(e.sessionId);
      if (!s || !s.agents[e.agent.agentId]) return;
      d.agents(s)[e.agent.agentId] = e.agent;
      return;
    }
    case 'toolCallStarted': {
      const s = d.session(e.sessionId);
      const a = s && d.agent(s, e.agentId);
      if (!a) return;
      const recent = [...a.recentTools, e.call];
      a.recentTools = recent.length > RECENT_TOOLS_MAX ? recent.slice(recent.length - RECENT_TOOLS_MAX) : recent;
      a.toolCalls += 1;
      a.lastTool = { name: e.call.name, summary: e.call.summary, at: e.call.startedAt };
      a.tokens = e.tokens;
      a.outputTokens = e.outputTokens;
      a.lastActivityAt = Math.max(a.lastActivityAt ?? 0, e.t);
      return;
    }
    case 'toolCallFinished': {
      const s = d.session(e.sessionId);
      const a = s && d.agent(s, e.agentId);
      if (!a) return;
      const i = a.recentTools.findIndex((c) => c.toolUseId === e.toolUseId);
      if (i >= 0) {
        const recent = [...a.recentTools];
        recent[i] = { ...recent[i], endedAt: e.t, isError: e.isError };
        a.recentTools = recent;
      }
      a.lastActivityAt = Math.max(a.lastActivityAt ?? 0, e.t);
      return;
    }
    case 'fileTouched': {
      const s = d.session(e.sessionId);
      if (!s || !s.agents[e.agentId]) return;
      const files = d.files(s);
      const prev = files[e.path];
      const f: FileEntry = { path: e.path, touches: prev ? { ...prev.touches } : {} };
      const old = f.touches[e.agentId];
      const t = old
        ? { ...old }
        : { agentId: e.agentId, reads: 0, writes: 0, firstAt: e.t, lastAt: e.t, viaBash: false };
      if (e.op === 'read') t.reads += 1;
      else t.writes += 1;
      t.firstAt = Math.min(t.firstAt, e.t);
      t.lastAt = Math.max(t.lastAt, e.t);
      if (e.source === 'bash') t.viaBash = true;
      f.touches[e.agentId] = t;
      files[e.path] = f;
      return;
    }
    case 'runStarted':
    case 'runUpdated': {
      const s = d.session(e.sessionId);
      if (!s) return;
      if (e.type === 'runUpdated' && !s.runs[e.run.runId]) return;
      d.runs(s)[e.run.runId] = e.run;
      return;
    }
    case 'runFinalized': {
      const s = d.session(e.sessionId);
      if (!s) return;
      d.runs(s)[e.run.runId] = e.run;
      if (e.agents.length) {
        const agents = d.agents(s);
        for (const a of e.agents) agents[a.agentId] = a;
      }
      return;
    }
  }
}

/** Apply events in order. Returns `snap` itself when nothing changed. */
export function applyEvents(snap: Snapshot, events: readonly VirgilEvent[]): Snapshot {
  if (events.length === 0) return snap;
  const d = new Draft(snap);
  for (const e of events) applyOne(d, e);
  return d.snap;
}

/** `apply(s, e)` of the contract. */
export function apply(snap: Snapshot, event: VirgilEvent): Snapshot {
  return applyEvents(snap, [event]);
}
