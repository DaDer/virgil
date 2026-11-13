// A DataSource that plays the scripted scenario, so the UI runs in a plain browser.
// Live mode: the scenario plays at wall-clock speed and loops every SCENARIO_DURATION.
// Replay mode: one fake recording of the same scenario with play, pause, speed, seek and
// quiet-gap skipping. Variants (URL ?mock=<variant>) reach the empty, loading and error states.

import type { VirgilEvent, AgentToolHistory, RecordingInfo, Snapshot, SourceMode, SourceStatus, ToolCall, ToolHistory } from '../contract';
import { applyEvents, emptySnapshot } from './reducer';
import { PREHISTORY, SCENARIO_DURATION, SESSION_ORCHARD, buildScenario } from './scenario';
import { SPEED_MAX, SPEED_MIN, type DataSource, type SourceHandlers } from './source';

export type MockVariant = 'demo' | 'empty' | 'missing' | 'error' | 'slow';
export const MOCK_VARIANTS: readonly MockVariant[] = ['demo', 'empty', 'missing', 'error', 'slow'];

export const MOCK_RECORDING_PATH = 'mock://recordings/demo-build';
/** Scenario time 0 of the fake recording: 2026-11-12 10:27:00 UTC. */
export const MOCK_RECORDING_BASE = Date.UTC(2026, 10, 12, 10, 27, 0);
/** Live mode starts this far into the scenario, so there is something to see at once. */
export const LIVE_START_OFFSET = 45_000;
const CLAUDE_DIR = '/home/dev/.claude';
const QUIET_GAP = 30_000;
const TICK_MS = 100;
const STATUS_EVERY_MS = 250;

export interface MockOptions {
  variant?: MockVariant;
  /** Wall clock; injectable for tests. */
  now?: () => number;
  /** Run the internal interval. Tests pass false and call tick(). */
  timers?: boolean;
}

function forReplay(events: VirgilEvent[]): VirgilEvent[] {
  return events.map((e) =>
    e.type === 'sessionUpserted' ? { ...e, session: { ...e.session, liveness: 'unknown' as const } } : e,
  );
}

function commandError(code: string, message: string) {
  return { code, message };
}

export class MockSource implements DataSource {
  readonly kind = 'mock' as const;
  readonly variant: MockVariant;
  private now: () => number;
  private timers: boolean;
  private handlers: SourceHandlers | null = null;
  private interval: ReturnType<typeof setInterval> | null = null;

  private seq = 0;
  private snap: Snapshot = emptySnapshot('live');
  private mode: SourceMode = 'live';
  private script: VirgilEvent[] = [];
  private cursor = 0;

  // live
  private liveBase = 0;
  // replay
  private position = 0;
  private playing = false;
  private speed = 10;
  private skipGaps = true;
  private lastWall = 0;
  private lastStatusWall = 0;
  private failedOnce = false;

  constructor(opts: MockOptions = {}) {
    this.variant = opts.variant ?? 'demo';
    this.now = opts.now ?? (() => Date.now());
    this.timers = opts.timers ?? true;
    this.startLive();
  }

  // ---------- DataSource ----------

  async connect(h: SourceHandlers): Promise<() => void> {
    this.handlers = h;
    this.lastWall = this.now();
    if (this.timers && !this.interval) this.interval = setInterval(() => this.tick(), TICK_MS);
    return () => {
      this.handlers = null;
      if (this.interval) clearInterval(this.interval);
      this.interval = null;
    };
  }

  async getSnapshot(): Promise<Snapshot> {
    if (this.variant === 'slow') await new Promise((r) => setTimeout(r, 1500));
    if (this.variant === 'error' && !this.failedOnce) {
      this.failedOnce = true;
      throw commandError('internal', 'The mock core failed on purpose (variant "error"). Try again works.');
    }
    return this.snap;
  }

  async getStatus(): Promise<SourceStatus> {
    return this.status();
  }

  /** Every tool call applied so far, rebuilt from the script (like the core keeps them). */
  async getToolHistory(): Promise<ToolHistory> {
    const byAgent = new Map<string, AgentToolHistory>();
    const calls = new Map<string, ToolCall>();
    for (let i = 0; i < this.cursor; i++) {
      const e = this.script[i];
      if (e.type === 'toolCallStarted') {
        const key = `${e.sessionId}/${e.agentId}`;
        let h = byAgent.get(key);
        if (!h) byAgent.set(key, (h = { sessionId: e.sessionId, agentId: e.agentId, calls: [] }));
        const c = { ...e.call };
        h.calls.push(c);
        calls.set(`${key}/${c.toolUseId}`, c);
      } else if (e.type === 'toolCallFinished') {
        const c = calls.get(`${e.sessionId}/${e.agentId}/${e.toolUseId}`);
        if (c) {
          c.endedAt = e.t;
          c.isError = e.isError;
        }
      }
    }
    return {
      seq: this.snap.seq,
      agents: [...byAgent.values()],
      timelineTimes: this.mode === 'replay' ? this.script.map((e) => e.t) : [],
    };
  }

  async listRecordings(): Promise<RecordingInfo[]> {
    return this.variant === 'demo' || this.variant === 'slow' ? [this.recording()] : [];
  }

  async openRecording(path: string): Promise<SourceStatus> {
    if (path !== MOCK_RECORDING_PATH) {
      throw commandError('invalidRecording', 'This folder is not an Virgil recording (manifest.json is missing).');
    }
    this.mode = 'replay';
    this.script = forReplay(buildScenario(MOCK_RECORDING_BASE));
    this.playing = false;
    this.speed = 10;
    this.rebuild(MOCK_RECORDING_BASE);
    return this.emitStatus();
  }

  async switchToLive(): Promise<SourceStatus> {
    this.startLive();
    this.emitReset();
    return this.emitStatus();
  }

  async play(): Promise<SourceStatus> {
    this.requireReplay();
    if (this.position >= this.end()) this.rebuild(this.start());
    this.playing = true;
    this.lastWall = this.now();
    return this.emitStatus();
  }

  async pause(): Promise<SourceStatus> {
    this.requireReplay();
    this.playing = false;
    return this.emitStatus();
  }

  async seek(t: number): Promise<SourceStatus> {
    this.requireReplay();
    this.rebuild(Math.min(this.end(), Math.max(this.start(), t)));
    this.lastWall = this.now();
    return this.emitStatus();
  }

  async setSpeed(speed: number): Promise<SourceStatus> {
    this.requireReplay();
    this.speed = Math.min(SPEED_MAX, Math.max(SPEED_MIN, speed));
    return this.emitStatus();
  }

  async setSkipGaps(on: boolean): Promise<SourceStatus> {
    this.skipGaps = on;
    return this.emitStatus();
  }

  // ---------- clock ----------

  /** Advance the clock to now and deliver due events. Called by the interval, or by tests. */
  tick(): void {
    const wall = this.now();
    const dt = Math.max(0, wall - this.lastWall);
    this.lastWall = wall;
    if (this.mode === 'live') {
      if (this.variant !== 'demo' && this.variant !== 'slow' && this.variant !== 'error') return;
      if (wall >= this.liveBase + SCENARIO_DURATION) {
        // Loop: a new copy of the scenario starting now.
        this.liveBase += SCENARIO_DURATION * Math.floor((wall - this.liveBase) / SCENARIO_DURATION);
        this.script = buildScenario(this.liveBase);
        this.cursor = 0;
        this.snap = emptySnapshot('live');
        this.collect(this.liveBase - PREHISTORY);
        this.emitReset();
        return;
      }
      this.emitEvents(this.collect(wall));
      return;
    }
    if (!this.playing) return;
    let rate = this.speed;
    if (this.skipGaps) {
      const prev = this.cursor > 0 ? this.script[this.cursor - 1].t : this.start();
      const next = this.cursor < this.script.length ? this.script[this.cursor].t : this.end();
      if (next - prev > QUIET_GAP) rate = Math.max(rate, (next - prev) / 1000);
    }
    this.position = Math.min(this.end(), this.position + dt * rate);
    this.emitEvents(this.collect(this.position));
    if (this.position >= this.end()) {
      this.playing = false;
      this.emitStatus();
    } else if (wall - this.lastStatusWall >= STATUS_EVERY_MS) {
      this.emitStatus();
    }
  }

  // ---------- internals ----------

  private startLive(): void {
    this.mode = 'live';
    this.playing = false;
    const wall = this.now();
    this.liveBase = wall - LIVE_START_OFFSET;
    this.script = this.variant === 'demo' || this.variant === 'slow' || this.variant === 'error' ? buildScenario(this.liveBase) : [];
    this.cursor = 0;
    this.snap = emptySnapshot('live');
    // The snapshot starts with the sessions only; the first tick streams their history, so the
    // UI sees every tool call (activity sparklines) as it would from a running core.
    this.collect(this.liveBase - PREHISTORY);
  }

  /** Apply scripted events up to `t` to the source's own snapshot; return them renumbered. */
  private collect(t: number): VirgilEvent[] {
    const out: VirgilEvent[] = [];
    while (this.cursor < this.script.length && this.script[this.cursor].t <= t) {
      out.push({ ...this.script[this.cursor], seq: ++this.seq } as VirgilEvent);
      this.cursor += 1;
    }
    if (out.length) this.snap = { ...applyEvents(this.snap, out), mode: this.mode };
    return out;
  }

  private rebuild(position: number): void {
    this.position = position;
    this.cursor = 0;
    this.snap = emptySnapshot(this.mode);
    this.collect(position);
    this.emitReset();
  }

  private emitReset(): void {
    const seq = ++this.seq;
    this.snap = { ...this.snap, seq, mode: this.mode };
    this.handlers?.onEvents([{ type: 'snapshotReset', seq, t: this.mode === 'live' ? this.now() : this.position, snapshot: this.snap }]);
  }

  private emitEvents(events: VirgilEvent[]): void {
    if (events.length) this.handlers?.onEvents(events);
  }

  private emitStatus(): SourceStatus {
    const s = this.status();
    this.lastStatusWall = this.now();
    this.handlers?.onStatus(s);
    return s;
  }

  private requireReplay(): void {
    if (this.mode !== 'replay') throw commandError('notReplaying', 'Not replaying a recording.');
  }

  private start(): number {
    return MOCK_RECORDING_BASE;
  }

  private end(): number {
    return MOCK_RECORDING_BASE + SCENARIO_DURATION;
  }

  private recording(): RecordingInfo {
    return {
      path: MOCK_RECORDING_PATH,
      name: 'demo-build',
      format: 1,
      sessionId: SESSION_ORCHARD,
      projectSlug: '-home-dev-projects-orchard',
      startedAt: this.start(),
      endedAt: this.end(),
      eventCount: 1200,
      complete: true,
    };
  }

  private status(): SourceStatus {
    if (this.mode === 'replay') {
      return {
        mode: 'replay',
        claudeDir: null,
        recording: this.recording(),
        replay: {
          playing: this.playing,
          speed: this.speed,
          position: this.position,
          start: this.start(),
          end: this.end(),
          skipGaps: this.skipGaps,
        },
        error: null,
        problems: [],
      };
    }
    return {
      mode: 'live',
      claudeDir: CLAUDE_DIR,
      recording: null,
      replay: null,
      error: this.variant === 'missing' ? `Claude folder not found: ${CLAUDE_DIR}` : null,
      problems: [],
    };
  }

  /** For tests and the skip-gaps toggle. */
  get skippingGaps(): boolean {
    return this.skipGaps;
  }
}

export function mockVariantFromUrl(): MockVariant {
  if (typeof location === 'undefined') return 'demo';
  const v = new URLSearchParams(location.search).get('mock');
  return (MOCK_VARIANTS as readonly string[]).includes(v ?? '') ? (v as MockVariant) : 'demo';
}
