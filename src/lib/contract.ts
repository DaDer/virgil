// Virgil data contract, version 1. Source of truth: docs/data-contract.md.
// Do not edit by hand without changing the document.

export type EpochMs = number;
export type SessionId = string;
/** Unique within a session. The main thread is the literal "main". */
export type AgentId = string;
export type RunId = string;
export type ToolUseId = string;

export const MAIN_AGENT_ID: AgentId = "main";
export const RECENT_TOOLS_MAX = 8;
export const EVENT_CHANNEL = "virgil:events";
export const STATUS_CHANNEL = "virgil:status";

// ---------- domain model ----------

export type Liveness = "alive" | "stale" | "unknown";
export type AgentKind = "main" | "subagent" | "workflow";
export type AgentState = "queued" | "running" | "waiting" | "done" | "failed" | "stopped";
export type RunStatus = "running" | "completed" | "failed" | "stopped";
export type PhaseState = "pending" | "running" | "done" | "failed";
export type FileOp = "read" | "write";
export type TouchSource = "tool" | "bash";
export type SourceMode = "live" | "replay";

export interface SessionInfo {
  sessionId: SessionId;
  pid: number | null;
  liveness: Liveness;
  cwd: string | null;
  projectSlug: string | null;
  /** Short derived name from the session file. */
  name: string | null;
  /** AI-written title from the transcript. */
  title: string | null;
  /** "busy", "idle", or whatever Claude Code writes. */
  status: string | null;
  kind: string | null;
  version: string | null;
  startedAt: EpochMs | null;
  updatedAt: EpochMs | null;
  statusUpdatedAt: EpochMs | null;
  hasTranscript: boolean;
}

export interface Session extends SessionInfo {
  agents: Record<AgentId, Agent>;
  runs: Record<RunId, WorkflowRun>;
  /** Keyed by normalised absolute path. */
  files: Record<string, FileEntry>;
}

export interface ToolCall {
  toolUseId: ToolUseId;
  name: string;
  /** At most 120 characters; may be empty. */
  summary: string;
  startedAt: EpochMs;
  endedAt: EpochMs | null;
  /** null until the result arrives. */
  isError: boolean | null;
}

export interface LastTool {
  name: string;
  summary: string;
  at: EpochMs;
}

export interface Agent {
  agentId: AgentId;
  sessionId: SessionId;
  kind: AgentKind;
  /** null only for the main agent. May name an agent that is not in the snapshot yet. */
  parentAgentId: AgentId | null;
  spawnToolUseId: ToolUseId | null;
  /** 0 for main. */
  spawnDepth: number;
  agentType: string | null;
  name: string | null;
  /** Short display name. */
  label: string;
  /** High-level task, at most 280 characters; empty when unknown. */
  task: string;
  model: string | null;
  state: AgentState;
  stateReason: string | null;
  runId: RunId | null;
  /** 1-based; null outside workflows. */
  phaseIndex: number | null;
  phaseTitle: string | null;
  attempt: number | null;
  /** An abandoned attempt that a later agent replaced. */
  superseded: boolean;
  queuedAt: EpochMs | null;
  startedAt: EpochMs | null;
  endedAt: EpochMs | null;
  lastActivityAt: EpochMs | null;
  /** Context size at the latest request. */
  tokens: number;
  outputTokens: number;
  toolCalls: number;
  lastTool: LastTool | null;
  /** Newest RECENT_TOOLS_MAX calls, oldest first. */
  recentTools: ToolCall[];
  resultPreview: string | null;
  worktreePath: string | null;
}

export interface Phase {
  /** 1-based. */
  index: number;
  title: string;
  detail: string | null;
  model: string | null;
  state: PhaseState;
}

export interface WorkflowRun {
  runId: RunId;
  sessionId: SessionId;
  name: string;
  summary: string | null;
  status: RunStatus;
  /** true once the run file has been read. */
  final: boolean;
  parentAgentId: AgentId;
  toolUseId: ToolUseId | null;
  taskId: string | null;
  startedAt: EpochMs | null;
  endedAt: EpochMs | null;
  phases: Phase[];
  /** First-seen order. */
  agentIds: AgentId[];
  /** null until final; sum over agentIds meanwhile. */
  totalTokens: number | null;
  totalToolCalls: number | null;
  declaredAgentCount: number | null;
  defaultModel: string | null;
  error: string | null;
}

export interface FileTouch {
  agentId: AgentId;
  reads: number;
  writes: number;
  firstAt: EpochMs;
  lastAt: EpochMs;
  /** At least one touch was guessed from a Bash command. */
  viaBash: boolean;
}

export interface FileEntry {
  path: string;
  touches: Record<AgentId, FileTouch>;
}

export interface Snapshot {
  /** seq of the last event applied; 0 when none. */
  seq: number;
  t: EpochMs;
  mode: SourceMode;
  sessions: Record<SessionId, Session>;
}

// ---------- events ----------

interface EventBase {
  seq: number;
  t: EpochMs;
}

export interface SnapshotResetEvent extends EventBase {
  type: "snapshotReset";
  snapshot: Snapshot;
}
export interface SessionUpsertedEvent extends EventBase {
  type: "sessionUpserted";
  session: SessionInfo;
}
export interface SessionRemovedEvent extends EventBase {
  type: "sessionRemoved";
  sessionId: SessionId;
}
export interface AgentSpawnedEvent extends EventBase {
  type: "agentSpawned";
  sessionId: SessionId;
  agent: Agent;
}
export interface AgentUpdatedEvent extends EventBase {
  type: "agentUpdated";
  sessionId: SessionId;
  agent: Agent;
}
export interface ToolCallStartedEvent extends EventBase {
  type: "toolCallStarted";
  sessionId: SessionId;
  agentId: AgentId;
  call: ToolCall;
  /** The agent's tokens and outputTokens after this call's message. */
  tokens: number;
  outputTokens: number;
}
export interface ToolCallFinishedEvent extends EventBase {
  type: "toolCallFinished";
  sessionId: SessionId;
  agentId: AgentId;
  toolUseId: ToolUseId;
  isError: boolean;
}
export interface FileTouchedEvent extends EventBase {
  type: "fileTouched";
  sessionId: SessionId;
  agentId: AgentId;
  path: string;
  op: FileOp;
  source: TouchSource;
  toolUseId: ToolUseId;
}
export interface RunStartedEvent extends EventBase {
  type: "runStarted";
  sessionId: SessionId;
  run: WorkflowRun;
}
export interface RunUpdatedEvent extends EventBase {
  type: "runUpdated";
  sessionId: SessionId;
  run: WorkflowRun;
}
export interface RunFinalizedEvent extends EventBase {
  type: "runFinalized";
  sessionId: SessionId;
  run: WorkflowRun;
  /** Every agent of the run, as full replacements. */
  agents: Agent[];
}

export type VirgilEvent =
  | SnapshotResetEvent
  | SessionUpsertedEvent
  | SessionRemovedEvent
  | AgentSpawnedEvent
  | AgentUpdatedEvent
  | ToolCallStartedEvent
  | ToolCallFinishedEvent
  | FileTouchedEvent
  | RunStartedEvent
  | RunUpdatedEvent
  | RunFinalizedEvent;

export type VirgilEventType = VirgilEvent["type"];

/** Payload of EVENT_CHANNEL. */
export interface EventBatch {
  events: VirgilEvent[];
}

// ---------- Tauri surface ----------

export interface RecordingInfo {
  /** Absolute path of the recording directory. */
  path: string;
  /** Directory name. */
  name: string;
  format: number;
  sessionId: SessionId | null;
  projectSlug: string | null;
  startedAt: EpochMs;
  /** Timeline end: stoppedAt, else the last event's time. */
  endedAt: EpochMs;
  eventCount: number;
  /** false while still recording, or when final/ is missing. */
  complete: boolean;
}

export interface ReplayState {
  playing: boolean;
  speed: number;
  position: EpochMs;
  start: EpochMs;
  end: EpochMs;
  /** Quiet stretches of more than 30 s play in 1 s (section 6.2). */
  skipGaps: boolean;
}

export type ProblemReason = "badLine" | "badFile" | "truncated" | "readError";

/** A parse or read warning, merged per (path, reason). */
export interface Problem {
  /** Source time of the latest occurrence. */
  t: EpochMs;
  /** Relative to the Claude dir. */
  path: string;
  reason: ProblemReason;
  count: number;
}

/** Payload of STATUS_CHANNEL and the return value of most commands. */
export interface SourceStatus {
  mode: SourceMode;
  /** The Claude dir being watched; null in replay. */
  claudeDir: string | null;
  recording: RecordingInfo | null;
  /** null in live mode. */
  replay: ReplayState | null;
  /** A problem with the source, e.g. the Claude dir is missing. */
  error: string | null;
  /** Parse and read warnings of the current source, newest first, at most 50. */
  problems: Problem[];
}

/** Every tool call the core keeps for one agent (at most 2000, the newest), oldest first. */
export interface AgentToolHistory {
  sessionId: SessionId;
  agentId: AgentId;
  calls: ToolCall[];
}

/** Result of `get_tool_history`. */
export interface ToolHistory {
  /** seq of the snapshot this history belongs to. */
  seq: number;
  agents: AgentToolHistory[];
  /** Replay: the time of every fs-event of the recording, ascending (whole timeline). Live: []. */
  timelineTimes: EpochMs[];
}

export type CommandErrorCode = "notReplaying" | "notFound" | "invalidRecording" | "io" | "internal";

export interface CommandError {
  code: CommandErrorCode;
  message: string;
}

/** Result of `pick_recording_folder`: the chosen folder, canonical (null when cancelled), and the recordings
 *  in it. When the folder is itself a recording, `recordings` is just that one and its `path` equals `path`. */
export interface PickedFolder {
  path: string | null;
  recordings: RecordingInfo[];
}

/** Result of `pick_claude_dir`: the chosen folder, canonical, or null when cancelled. */
export interface PickedPath {
  path: string | null;
}

/** Command name -> argument object and return type, for a typed invoke wrapper. */
export interface Commands {
  get_snapshot: { args: Record<string, never>; returns: Snapshot };
  get_status: { args: Record<string, never>; returns: SourceStatus };
  list_recordings: { args: { dir?: string }; returns: RecordingInfo[] };
  open_recording: { args: { path: string }; returns: SourceStatus };
  replay_play: { args: Record<string, never>; returns: SourceStatus };
  replay_pause: { args: Record<string, never>; returns: SourceStatus };
  replay_seek: { args: { t: EpochMs }; returns: SourceStatus };
  replay_set_speed: { args: { speed: number }; returns: SourceStatus };
  switch_to_live: { args: Record<string, never>; returns: SourceStatus };
  pick_recording_folder: { args: Record<string, never>; returns: PickedFolder };
  get_tool_history: { args: Record<string, never>; returns: ToolHistory };
  set_skip_gaps: { args: { on: boolean }; returns: SourceStatus };
  set_claude_dir: { args: { dir: string | null }; returns: SourceStatus };
  pick_claude_dir: { args: Record<string, never>; returns: PickedPath };
}
