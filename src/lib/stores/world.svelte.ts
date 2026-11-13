// The world store: the snapshot from the data source, kept current by the event stream
// (data-contract 5.2 start-up rule), applied once per animation frame (ux-spec 5.3, rule 6).
// Views read `world.snapshot` and the helpers; they never talk to the source.

import { MAIN_AGENT_ID, type Agent, type VirgilEvent, type PickedFolder, type RecordingInfo, type Session, type Snapshot, type SourceStatus, type WorkflowRun } from '../contract';
import { applyEvents, emptySnapshot } from '../data/reducer';
import { errorText, type DataSource } from '../data/source';
import {
  agentsByPhase as deriveAgentsByPhase,
  childrenOf as deriveChildrenOf,
  compareSessionsBusyFirst,
  currentRun,
  fileTouchers as deriveFileTouchers,
  filesByAgent as deriveFilesByAgent,
  parsePhaseKey,
  sharedPairs as deriveSharedPairs,
  type SharedPair,
} from '../data/derive';
import type { FileTouch, Problem } from '../contract';
import { ToolHistory } from '../views/shared/toolHistory';

export type { Problem };

export type WorldPhase = 'connecting' | 'ready' | 'error';

/** Tool-call start times per session, from the core's tool history and the stream. */
type Activity = Record<string, number[]>;
const ACTIVITY_MAX = 50_000;

function raf(fn: () => void): void {
  if (typeof requestAnimationFrame === 'function') requestAnimationFrame(fn);
  else setTimeout(fn, 16);
}

class WorldStore {
  snapshot = $state.raw<Snapshot>(emptySnapshot());
  status = $state.raw<SourceStatus | null>(null);
  phase = $state<WorldPhase>('connecting');
  /** Error from the source itself (get_snapshot failed). Not the status' error. */
  error = $state<string | null>(null);
  /** Wall-clock time of the last received event batch. */
  lastEventAt = $state<number | null>(null);
  activity = $state.raw<Activity>({});
  /** Times of every event seen in this source context. */
  eventTimes = $state.raw<number[]>([]);
  /** Replay: every fs-event time of the recording (prev/next event, quiet gaps). Empty in live. */
  timelineTimes = $state.raw<number[]>([]);
  /** Every tool call known per agent; read it through `history` (views/shared/history.svelte.ts). */
  readonly tools = new ToolHistory();
  /** Bumped whenever `tools` changes. */
  toolsVersion = $state(0);
  source = $state.raw<DataSource | null>(null);
  /** Wall-clock time the current status arrived; anchors replay interpolation. */
  statusAt = $state(Date.now());

  private pending: VirgilEvent[] = [];
  private buffering = true;
  private flushScheduled = false;
  private lastFlushWall = 0;
  private seenTools = new Set<string>();
  private seenTimes = new Set<number>();
  private context = '';
  private historyGen = 0;
  private unsubscribe: (() => void) | null = null;

  /** Parse and read warnings from the core, newest first. */
  get problems(): Problem[] {
    return this.status?.problems ?? [];
  }

  // ---------- derived ----------

  /** All sessions, busy first. */
  sessions = $derived(Object.values(this.snapshot.sessions).sort(compareSessionsBusyFirst));

  get mode(): 'live' | 'replay' {
    return this.status?.mode ?? 'live';
  }

  session(id: string | null | undefined): Session | null {
    return (id && this.snapshot.sessions[id]) || null;
  }

  agent(sessionId: string | null | undefined, agentId: string): Agent | null {
    return this.session(sessionId)?.agents[agentId] ?? null;
  }

  run(sessionId: string | null | undefined, runId: string | null | undefined): WorkflowRun | null {
    return (runId && this.session(sessionId)?.runs[runId]) || null;
  }

  /** The given run, else the run in flight, else the newest. */
  runOrCurrent(sessionId: string | null | undefined, runId: string | null | undefined): WorkflowRun | null {
    const s = this.session(sessionId);
    if (!s) return null;
    return (runId && s.runs[runId]) || currentRun(s);
  }

  phaseByKey(sessionId: string | null | undefined, key: string) {
    const k = parsePhaseKey(key);
    const run = k && this.run(sessionId, k.runId);
    return (run && run.phases.find((p) => p.index === k!.index)) || null;
  }

  // Derived maps, memoised by session object identity (the reducer copies on write).
  private memo = new WeakMap<Session, Map<string, unknown>>();
  private cached<T>(s: Session, key: string, f: () => T): T {
    let m = this.memo.get(s);
    if (!m) this.memo.set(s, (m = new Map()));
    if (!m.has(key)) m.set(key, f());
    return m.get(key) as T;
  }

  childrenOf(s: Session): Map<string, Agent[]> {
    return this.cached(s, 'children', () => deriveChildrenOf(s));
  }
  agentsByPhase(s: Session, run: WorkflowRun): Map<number, Agent[]> {
    return this.cached(s, `phases:${run.runId}`, () => deriveAgentsByPhase(s, run));
  }
  fileTouchers(s: Session): Map<string, FileTouch[]> {
    return this.cached(s, 'touchers', () => deriveFileTouchers(s));
  }
  filesByAgent(s: Session) {
    return this.cached(s, 'filesByAgent', () => deriveFilesByAgent(s));
  }
  sharedPairs(s: Session, mode: 'edits' | 'all' = 'edits'): SharedPair[] {
    return this.cached(s, `pairs:${mode}`, () => deriveSharedPairs(s, mode));
  }

  /** Tool-call times for a session (all known, sorted). */
  activityOf(sessionId: string): number[] {
    return this.activity[sessionId] ?? [];
  }

  /** Counts for the StatusBar. */
  busy = $derived(
    Object.values(this.snapshot.sessions).filter((s) => (s.status === 'busy' || s.status === 'shell') && s.liveness !== 'stale').length,
  );
  runningAgents = $derived(
    Object.values(this.snapshot.sessions).reduce(
      (n, s) => n + Object.values(s.agents).filter((a) => a.agentId !== MAIN_AGENT_ID && a.state === 'running').length,
      0,
    ),
  );

  // ---------- lifecycle ----------

  async start(source: DataSource): Promise<void> {
    this.stop();
    this.source = source;
    this.phase = 'connecting';
    this.error = null;
    this.buffering = true;
    this.pending = [];
    // Diagnostics: a one-line view of what the store holds, read by the Tauri source's reporter.
    (globalThis as { __virgilDiag?: () => string }).__virgilDiag = () => {
      const alive = Object.values(this.snapshot.sessions).filter((x) => x.liveness !== 'stale');
      const rows = alive.map((x) => {
        const main = x.agents[MAIN_AGENT_ID];
        const running = Object.values(x.agents).filter((a) => a.state === 'running').length;
        return `${x.name ?? x.sessionId.slice(0, 8)}[${x.status}; main ${main?.state} tools ${main?.toolCalls}; running ${running}/${Object.keys(x.agents).length}]`;
      });
      return `store ${this.phase} seq ${this.snapshot.seq} pending ${this.pending.length} buffering ${this.buffering} visible ${document.visibilityState} :: ${rows.join(' ')}`;
    };
    try {
      // Listen first, then read the snapshot (data-contract 5.2).
      this.unsubscribe = await source.connect({
        onEvents: (events) => this.receive(events),
        onStatus: (status) => this.setStatus(status),
      });
      this.setStatus(await source.getStatus());
      await this.resync();
    } catch (e) {
      this.phase = 'error';
      this.error = errorText(e);
    }
  }

  stop(): void {
    this.unsubscribe?.();
    this.unsubscribe = null;
  }

  /** Fetch a fresh snapshot and apply the buffered events after it. */
  async resync(): Promise<void> {
    const source = this.source;
    if (!source) return;
    this.buffering = true;
    try {
      const snap = await source.getSnapshot();
      this.noteContext();
      this.indexSnapshot(snap);
      const later = this.pending.filter((e) => e.seq > snap.seq);
      this.pending = [];
      this.snapshot = applyEvents(snap, later);
      this.track(later);
      this.phase = 'ready';
      this.error = null;
    } catch (e) {
      this.phase = 'error';
      this.error = errorText(e);
    } finally {
      this.buffering = false;
    }
  }

  async retry(): Promise<void> {
    if (this.source) await this.start(this.source);
  }

  setStatus(status: SourceStatus): void {
    this.status = status;
    this.statusAt = Date.now();
    this.noteContext();
  }

  private receive(events: VirgilEvent[]): void {
    if (!events.length) return;
    this.lastEventAt = Date.now();
    this.pending.push(...events);
    if (this.buffering) return;
    this.scheduleFlush();
  }

  private scheduleFlush(): void {
    if (this.flushScheduled) return;
    this.flushScheduled = true;
    const fast = (this.status?.replay?.speed ?? 1) > 10 && this.status?.replay?.playing;
    const wait = fast ? Math.max(0, 100 - (Date.now() - this.lastFlushWall)) : 0;
    const run = () => raf(() => this.flush());
    if (wait > 0) setTimeout(run, wait);
    else run();
  }

  private flush(): void {
    this.flushScheduled = false;
    // After a failed snapshot, wait for an explicit retry (the error state stays readable).
    if (this.buffering || this.phase === 'error' || !this.pending.length) return;
    this.lastFlushWall = Date.now();
    let events = this.pending;
    this.pending = [];
    // Drop what the snapshot already holds; resync on a gap.
    events = events.filter((e) => e.seq > this.snapshot.seq);
    if (!events.length) return;
    const resetAt = events.findIndex((e) => e.type === 'snapshotReset');
    const head = resetAt >= 0 ? events.slice(0, resetAt) : events;
    if (head.length && head[0].seq > this.snapshot.seq + 1) {
      this.pending = events;
      void this.resync();
      return;
    }
    this.snapshot = applyEvents(this.snapshot, events);
    if (resetAt >= 0) {
      const reset = events[resetAt];
      if (reset.type === 'snapshotReset') this.indexSnapshot(reset.snapshot);
      this.track(events.slice(resetAt + 1));
    } else {
      this.track(events);
    }
  }

  /** Start fresh activity tracking when the mode or the recording changes. */
  private noteContext(): void {
    const s = this.status;
    const ctx = s ? `${s.mode}:${s.recording?.path ?? ''}` : '';
    if (ctx === this.context) return;
    this.context = ctx;
    this.seenTimes.clear();
    this.eventTimes = [];
    this.timelineTimes = [];
    this.clearTools();
  }

  private clearTools(): void {
    this.historyGen += 1;
    this.seenTools.clear();
    this.activity = {};
    this.tools.clear();
    this.toolsVersion += 1;
  }

  /** A new snapshot (start-up, resync, `snapshotReset`): the tool history starts over from it. The
   *  snapshot's recent calls show at once; the core's full history follows. */
  private indexSnapshot(snap: Snapshot): void {
    this.clearTools();
    const add: Record<string, number[]> = {};
    for (const s of Object.values(snap.sessions)) {
      for (const a of Object.values(s.agents)) {
        this.tools.addCalls(s.sessionId, a.agentId, a.recentTools);
        for (const c of a.recentTools) {
          if (this.seenTools.has(c.toolUseId)) continue;
          this.seenTools.add(c.toolUseId);
          (add[s.sessionId] ??= []).push(c.startedAt);
        }
      }
    }
    this.merge(add, []);
    void this.loadHistory();
  }

  /** Fetch the core's full tool history. Merges with what the stream delivered meanwhile; a
   *  response that a later snapshot made obsolete is dropped. */
  private async loadHistory(): Promise<void> {
    const source = this.source;
    if (!source) return;
    const gen = this.historyGen;
    let h;
    try {
      h = await source.getToolHistory();
    } catch {
      return; // Recent calls from the snapshot stay; the next snapshot tries again.
    }
    if (gen !== this.historyGen || source !== this.source) return;
    const add: Record<string, number[]> = {};
    for (const a of h.agents) {
      this.tools.addCalls(a.sessionId, a.agentId, a.calls);
      for (const c of a.calls) {
        if (this.seenTools.has(c.toolUseId)) continue;
        this.seenTools.add(c.toolUseId);
        (add[a.sessionId] ??= []).push(c.startedAt);
      }
    }
    this.toolsVersion += 1;
    this.merge(add, []);
    if (this.mode === 'replay') this.timelineTimes = h.timelineTimes;
  }

  private track(events: VirgilEvent[]): void {
    if (this.tools.applyEvents(events)) this.toolsVersion += 1;
    const add: Record<string, number[]> = {};
    const times: number[] = [];
    for (const e of events) {
      if (e.type === 'snapshotReset') continue;
      if (!this.seenTimes.has(e.t)) {
        this.seenTimes.add(e.t);
        times.push(e.t);
      }
      if (e.type === 'toolCallStarted' && !this.seenTools.has(e.call.toolUseId)) {
        this.seenTools.add(e.call.toolUseId);
        (add[e.sessionId] ??= []).push(e.call.startedAt);
      }
    }
    this.merge(add, times);
  }

  private merge(add: Record<string, number[]>, times: number[]): void {
    const keys = Object.keys(add);
    if (keys.length) {
      const next = { ...this.activity };
      for (const k of keys) {
        const merged = [...(next[k] ?? []), ...add[k]].sort((a, b) => a - b);
        next[k] = merged.length > ACTIVITY_MAX ? merged.slice(merged.length - ACTIVITY_MAX) : merged;
      }
      this.activity = next;
    }
    if (times.length) {
      const merged = [...this.eventTimes, ...times].sort((a, b) => a - b);
      this.eventTimes = merged.length > ACTIVITY_MAX ? merged.slice(merged.length - ACTIVITY_MAX) : merged;
    }
  }

  // ---------- Claude folder ----------

  get canChooseClaudeDir(): boolean {
    return !!(this.source?.setClaudeDir && this.source.pickClaudeDir);
  }

  /** Native folder dialog; null when cancelled or unsupported. */
  async pickClaudeDir(): Promise<string | null> {
    return this.source?.pickClaudeDir ? (await this.source.pickClaudeDir()).path : null;
  }

  /** Watch `dir` (null: the default). Rejects with the core's error. */
  async setClaudeDir(dir: string | null): Promise<void> {
    if (!this.source?.setClaudeDir) return;
    this.setStatus(await this.source.setClaudeDir(dir));
  }

  // ---------- recordings ----------

  get canPickFolder(): boolean {
    return !!this.source?.pickRecordingFolder;
  }

  async pickRecordingFolder(): Promise<PickedFolder> {
    return this.source?.pickRecordingFolder ? this.source.pickRecordingFolder() : { path: null, recordings: [] };
  }

  async listRecordings(dir?: string): Promise<RecordingInfo[]> {
    return this.source ? this.source.listRecordings(dir) : [];
  }
}

export const world = new WorldStore();
