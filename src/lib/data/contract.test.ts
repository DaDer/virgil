// Contract test: fixtures/contract-sample.json is written by the Rust core
// (`npm run contract:sample`, crates/core/examples/contract_sample.rs). Feeding it through the UI
// reducer and checking every object's keys against the TypeScript interfaces catches drift on
// either side of the Tauri boundary.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, test } from 'vitest';
import type {
  Agent,
  AgentToolHistory,
  Problem,
  ToolHistory as ToolHistoryT,
  VirgilEvent,
  CommandError,
  EventBatch,
  FileEntry,
  FileTouch,
  LastTool,
  Phase,
  RecordingInfo,
  ReplayState,
  Session,
  SessionInfo,
  Snapshot,
  SourceStatus,
  ToolCall,
  WorkflowRun,
} from '../contract';
import { applyEvents, emptySnapshot } from './reducer';
import { ToolHistory } from '../views/shared/toolHistory';

interface Sample {
  events: VirgilEvent[];
  batch: EventBatch;
  snapshot: Snapshot;
  statusReplay: SourceStatus;
  statusLive: SourceStatus;
  recordings: RecordingInfo[];
  commandError: CommandError;
  toolHistory: ToolHistoryT;
}

const sample = JSON.parse(
  readFileSync(fileURLToPath(new URL('../../../fixtures/contract-sample.json', import.meta.url)), 'utf8'),
) as Sample;

// Exhaustive key lists: the compiler checks they match the interfaces, the test checks the JSON.
type Keys<T> = Record<keyof T, true>;
const k = <T>(keys: Keys<T>) => Object.keys(keys).sort();

const SESSION_INFO = k<SessionInfo>({
  sessionId: true, pid: true, liveness: true, cwd: true, projectSlug: true, name: true, title: true, status: true,
  kind: true, version: true, startedAt: true, updatedAt: true, statusUpdatedAt: true, hasTranscript: true,
});
const SESSION = k<Session>({
  ...(Object.fromEntries(SESSION_INFO.map((x) => [x, true])) as Keys<SessionInfo>),
  agents: true, runs: true, files: true,
});
const AGENT = k<Agent>({
  agentId: true, sessionId: true, kind: true, parentAgentId: true, spawnToolUseId: true, spawnDepth: true,
  agentType: true, name: true, label: true, task: true, model: true, state: true, stateReason: true, runId: true,
  phaseIndex: true, phaseTitle: true, attempt: true, superseded: true, queuedAt: true, startedAt: true,
  endedAt: true, lastActivityAt: true, tokens: true, outputTokens: true, toolCalls: true, lastTool: true,
  recentTools: true, resultPreview: true, worktreePath: true,
});
const TOOL_CALL = k<ToolCall>({ toolUseId: true, name: true, summary: true, startedAt: true, endedAt: true, isError: true });
const LAST_TOOL = k<LastTool>({ name: true, summary: true, at: true });
const PHASE = k<Phase>({ index: true, title: true, detail: true, model: true, state: true });
const RUN = k<WorkflowRun>({
  runId: true, sessionId: true, name: true, summary: true, status: true, final: true, parentAgentId: true,
  toolUseId: true, taskId: true, startedAt: true, endedAt: true, phases: true, agentIds: true, totalTokens: true,
  totalToolCalls: true, declaredAgentCount: true, defaultModel: true, error: true,
});
const FILE_ENTRY = k<FileEntry>({ path: true, touches: true });
const FILE_TOUCH = k<FileTouch>({ agentId: true, reads: true, writes: true, firstAt: true, lastAt: true, viaBash: true });
const SNAPSHOT = k<Snapshot>({ seq: true, t: true, mode: true, sessions: true });
const STATUS = k<SourceStatus>({ mode: true, claudeDir: true, recording: true, replay: true, error: true, problems: true });
const REPLAY = k<ReplayState>({ playing: true, speed: true, position: true, start: true, end: true, skipGaps: true });
const PROBLEM = k<Problem>({ t: true, path: true, reason: true, count: true });
const TOOL_HISTORY = k<ToolHistoryT>({ seq: true, agents: true, timelineTimes: true });
const AGENT_TOOL_HISTORY = k<AgentToolHistory>({ sessionId: true, agentId: true, calls: true });
const RECORDING = k<RecordingInfo>({
  path: true, name: true, format: true, sessionId: true, projectSlug: true, startedAt: true, endedAt: true,
  eventCount: true, complete: true,
});
const ERROR = k<CommandError>({ code: true, message: true });

/** Payload keys per event type, besides type, seq and t. */
const EVENT: { [T in VirgilEvent['type']]: string[] } = {
  snapshotReset: ['snapshot'],
  sessionUpserted: ['session'],
  sessionRemoved: ['sessionId'],
  agentSpawned: ['agent', 'sessionId'],
  agentUpdated: ['agent', 'sessionId'],
  toolCallStarted: ['agentId', 'call', 'outputTokens', 'sessionId', 'tokens'],
  toolCallFinished: ['agentId', 'isError', 'sessionId', 'toolUseId'],
  fileTouched: ['agentId', 'op', 'path', 'sessionId', 'source', 'toolUseId'],
  runStarted: ['run', 'sessionId'],
  runUpdated: ['run', 'sessionId'],
  runFinalized: ['agents', 'run', 'sessionId'],
};

const keysOf = (o: object) => Object.keys(o).sort();

function checkAgent(a: Agent) {
  expect(keysOf(a)).toEqual(AGENT);
  for (const c of a.recentTools) expect(keysOf(c)).toEqual(TOOL_CALL);
  if (a.lastTool) expect(keysOf(a.lastTool)).toEqual(LAST_TOOL);
}
function checkRun(r: WorkflowRun) {
  expect(keysOf(r)).toEqual(RUN);
  for (const p of r.phases) expect(keysOf(p)).toEqual(PHASE);
}
function checkSnapshot(s: Snapshot) {
  expect(keysOf(s)).toEqual(SNAPSHOT);
  for (const sess of Object.values(s.sessions)) {
    expect(keysOf(sess)).toEqual(SESSION);
    Object.values(sess.agents).forEach(checkAgent);
    Object.values(sess.runs).forEach(checkRun);
    for (const f of Object.values(sess.files)) {
      expect(keysOf(f)).toEqual(FILE_ENTRY);
      for (const t of Object.values(f.touches)) expect(keysOf(t)).toEqual(FILE_TOUCH);
    }
  }
}
function checkStatus(s: SourceStatus) {
  expect(keysOf(s)).toEqual(STATUS);
  if (s.replay) expect(keysOf(s.replay)).toEqual(REPLAY);
  if (s.recording) expect(keysOf(s.recording)).toEqual(RECORDING);
  for (const p of s.problems) expect(keysOf(p)).toEqual(PROBLEM);
}

describe('contract sample from the Rust core', () => {
  test('covers every event type', () => {
    expect(new Set(sample.events.map((e) => e.type))).toEqual(new Set(Object.keys(EVENT)));
  });

  test('every event has the payload keys of its type', () => {
    for (const e of sample.events) {
      const { type, seq, t, ...payload } = e;
      expect(typeof seq).toBe('number');
      expect(typeof t).toBe('number');
      expect(keysOf(payload), type).toEqual(EVENT[type]);
      switch (e.type) {
        case 'snapshotReset': checkSnapshot(e.snapshot); break;
        case 'sessionUpserted': expect(keysOf(e.session)).toEqual(SESSION_INFO); break;
        case 'agentSpawned': case 'agentUpdated': checkAgent(e.agent); break;
        case 'toolCallStarted': expect(keysOf(e.call)).toEqual(TOOL_CALL); break;
        case 'runStarted': case 'runUpdated': checkRun(e.run); break;
        case 'runFinalized': checkRun(e.run); e.agents.forEach(checkAgent); break;
      }
    }
  });

  test('seq numbers have no gaps', () => {
    sample.events.forEach((e, i) => expect(e.seq).toBe(sample.events[0].seq + i));
  });

  test('the UI reducer turns the stream into the core snapshot', () => {
    const out = applyEvents(emptySnapshot('replay'), sample.events);
    checkSnapshot(out);
    expect(out).toEqual(sample.snapshot);
  });

  test('a batch and the status, recording and error payloads match the types', () => {
    expect(keysOf(sample.batch)).toEqual(['events']);
    checkSnapshot(sample.snapshot);
    checkStatus(sample.statusReplay);
    checkStatus(sample.statusLive);
    sample.recordings.forEach((r) => expect(keysOf(r)).toEqual(RECORDING));
    expect(keysOf(sample.commandError)).toEqual(ERROR);
    expect(sample.statusLive.problems.length).toBeGreaterThan(0);
  });

  test('the tool history matches the types and holds every call the stream started', () => {
    const h = sample.toolHistory;
    expect(keysOf(h)).toEqual(TOOL_HISTORY);
    expect(h.seq).toBe(sample.snapshot.seq - 2); // the sample appends two synthetic events after it
    for (const a of h.agents) {
      expect(keysOf(a)).toEqual(AGENT_TOOL_HISTORY);
      a.calls.forEach((c) => expect(keysOf(c)).toEqual(TOOL_CALL));
    }
    expect(h.timelineTimes.length).toBeGreaterThan(0);
    // Fed from the history alone, or from the stream alone, the UI ends with the same calls.
    const fromHistory = new ToolHistory();
    for (const a of h.agents) fromHistory.addCalls(a.sessionId, a.agentId, a.calls);
    const fromStream = new ToolHistory();
    fromStream.applyEvents(sample.events);
    let total = 0;
    for (const a of h.agents) {
      expect(fromStream.ticks(a.sessionId, a.agentId)).toEqual(fromHistory.ticks(a.sessionId, a.agentId));
      total += a.calls.length;
    }
    const started = sample.events.filter((e) => e.type === 'toolCallStarted').length;
    expect(total).toBe(started);
    // The snapshot alone holds fewer: that is why the history exists.
    const recent = Object.values(sample.snapshot.sessions).flatMap((s) => Object.values(s.agents)).reduce((n, a) => n + a.recentTools.length, 0);
    expect(recent).toBeLessThanOrEqual(total);
  });
});
