//! The types that cross the Rust / UI boundary (docs/data-contract.md, section 7) and the reducer
//! of section 3.3. Serialised field names are camelCase; optional values serialise as `null`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type EpochMs = i64;
pub const MAIN_AGENT_ID: &str = "main";
pub const RECENT_TOOLS_MAX: usize = 8;
/// Tool calls kept per agent for `get_tool_history` (oldest dropped first).
pub const TOOL_HISTORY_MAX: usize = 2000;
/// Distinct problems kept in `SourceStatus.problems` (oldest dropped first).
pub const PROBLEMS_MAX: usize = 50;
pub const EVENT_CHANNEL: &str = "virgil:events";
pub const STATUS_CHANNEL: &str = "virgil:status";

macro_rules! string_enum {
    ($(#[$m:meta])* pub enum $name:ident { $($variant:ident),* $(,)? }) => {
        $(#[$m])*
        #[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[serde(rename_all = "camelCase")]
        pub enum $name { $($variant),* }
    };
}

string_enum!(pub enum Liveness { Alive, Stale, Unknown });
string_enum!(pub enum AgentKind { Main, Subagent, Workflow });
string_enum!(pub enum AgentState { Queued, Running, Waiting, Done, Failed, Stopped });
string_enum!(pub enum RunStatus { Running, Completed, Failed, Stopped });
string_enum!(pub enum PhaseState { Pending, Running, Done, Failed });
string_enum!(pub enum FileOp { Read, Write });
string_enum!(pub enum TouchSource { Tool, Bash });
string_enum!(pub enum SourceMode { Live, Replay });
string_enum!(pub enum CommandErrorCode { NotReplaying, NotFound, InvalidRecording, Io, Internal });

impl AgentState {
    pub fn is_terminal(self) -> bool {
        matches!(self, AgentState::Done | AgentState::Failed | AgentState::Stopped)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            AgentState::Queued => "queued",
            AgentState::Running => "running",
            AgentState::Waiting => "waiting",
            AgentState::Done => "done",
            AgentState::Failed => "failed",
            AgentState::Stopped => "stopped",
        }
    }
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Completed => "completed",
            RunStatus::Failed => "failed",
            RunStatus::Stopped => "stopped",
        }
    }
}

impl PhaseState {
    pub fn as_str(self) -> &'static str {
        match self {
            PhaseState::Pending => "pending",
            PhaseState::Running => "running",
            PhaseState::Done => "done",
            PhaseState::Failed => "failed",
        }
    }
}

impl Liveness {
    pub fn as_str(self) -> &'static str {
        match self {
            Liveness::Alive => "alive",
            Liveness::Stale => "stale",
            Liveness::Unknown => "unknown",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: String,
    pub pid: Option<u32>,
    pub liveness: Liveness,
    pub cwd: Option<String>,
    pub project_slug: Option<String>,
    pub name: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub kind: Option<String>,
    pub version: Option<String>,
    pub started_at: Option<EpochMs>,
    pub updated_at: Option<EpochMs>,
    pub status_updated_at: Option<EpochMs>,
    pub has_transcript: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    #[serde(flatten)]
    pub info: SessionInfo,
    pub agents: BTreeMap<String, Agent>,
    pub runs: BTreeMap<String, WorkflowRun>,
    pub files: BTreeMap<String, FileEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub tool_use_id: String,
    pub name: String,
    pub summary: String,
    pub started_at: EpochMs,
    pub ended_at: Option<EpochMs>,
    pub is_error: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LastTool {
    pub name: String,
    pub summary: String,
    pub at: EpochMs,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub agent_id: String,
    pub session_id: String,
    pub kind: AgentKind,
    pub parent_agent_id: Option<String>,
    pub spawn_tool_use_id: Option<String>,
    pub spawn_depth: u32,
    pub agent_type: Option<String>,
    pub name: Option<String>,
    pub label: String,
    pub task: String,
    pub model: Option<String>,
    pub state: AgentState,
    pub state_reason: Option<String>,
    pub run_id: Option<String>,
    pub phase_index: Option<u32>,
    pub phase_title: Option<String>,
    pub attempt: Option<u32>,
    pub superseded: bool,
    pub queued_at: Option<EpochMs>,
    pub started_at: Option<EpochMs>,
    pub ended_at: Option<EpochMs>,
    pub last_activity_at: Option<EpochMs>,
    pub tokens: u64,
    pub output_tokens: u64,
    pub tool_calls: u32,
    pub last_tool: Option<LastTool>,
    pub recent_tools: Vec<ToolCall>,
    pub result_preview: Option<String>,
    pub worktree_path: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Phase {
    pub index: u32,
    pub title: String,
    pub detail: Option<String>,
    pub model: Option<String>,
    pub state: PhaseState,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRun {
    pub run_id: String,
    pub session_id: String,
    pub name: String,
    pub summary: Option<String>,
    pub status: RunStatus,
    #[serde(rename = "final")]
    pub is_final: bool,
    pub parent_agent_id: String,
    pub tool_use_id: Option<String>,
    pub task_id: Option<String>,
    pub started_at: Option<EpochMs>,
    pub ended_at: Option<EpochMs>,
    pub phases: Vec<Phase>,
    pub agent_ids: Vec<String>,
    pub total_tokens: Option<u64>,
    pub total_tool_calls: Option<u64>,
    pub declared_agent_count: Option<u32>,
    pub default_model: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileTouch {
    pub agent_id: String,
    pub reads: u32,
    pub writes: u32,
    pub first_at: EpochMs,
    pub last_at: EpochMs,
    pub via_bash: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub touches: BTreeMap<String, FileTouch>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub seq: u64,
    pub t: EpochMs,
    pub mode: SourceMode,
    pub sessions: BTreeMap<String, Session>,
}

/// Serialises as `{"type": "...", "seq": 1, "t": 0, ...payload}`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Event {
    pub seq: u64,
    pub t: EpochMs,
    #[serde(flatten)]
    pub kind: EventKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum EventKind {
    SnapshotReset {
        snapshot: Box<Snapshot>,
    },
    SessionUpserted {
        session: SessionInfo,
    },
    SessionRemoved {
        session_id: String,
    },
    AgentSpawned {
        session_id: String,
        agent: Box<Agent>,
    },
    AgentUpdated {
        session_id: String,
        agent: Box<Agent>,
    },
    ToolCallStarted {
        session_id: String,
        agent_id: String,
        call: ToolCall,
        tokens: u64,
        output_tokens: u64,
    },
    ToolCallFinished {
        session_id: String,
        agent_id: String,
        tool_use_id: String,
        is_error: bool,
    },
    FileTouched {
        session_id: String,
        agent_id: String,
        path: String,
        op: FileOp,
        source: TouchSource,
        tool_use_id: String,
    },
    RunStarted {
        session_id: String,
        run: Box<WorkflowRun>,
    },
    RunUpdated {
        session_id: String,
        run: Box<WorkflowRun>,
    },
    RunFinalized {
        session_id: String,
        run: Box<WorkflowRun>,
        agents: Vec<Agent>,
    },
}

impl EventKind {
    pub fn type_name(&self) -> &'static str {
        match self {
            EventKind::SnapshotReset { .. } => "snapshotReset",
            EventKind::SessionUpserted { .. } => "sessionUpserted",
            EventKind::SessionRemoved { .. } => "sessionRemoved",
            EventKind::AgentSpawned { .. } => "agentSpawned",
            EventKind::AgentUpdated { .. } => "agentUpdated",
            EventKind::ToolCallStarted { .. } => "toolCallStarted",
            EventKind::ToolCallFinished { .. } => "toolCallFinished",
            EventKind::FileTouched { .. } => "fileTouched",
            EventKind::RunStarted { .. } => "runStarted",
            EventKind::RunUpdated { .. } => "runUpdated",
            EventKind::RunFinalized { .. } => "runFinalized",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct EventBatch {
    pub events: Vec<Event>,
}

impl Snapshot {
    pub fn empty(mode: SourceMode) -> Snapshot {
        Snapshot { seq: 0, t: 0, mode, sessions: BTreeMap::new() }
    }

    /// The reducer of section 3.3. The core and the tests use this one function.
    pub fn apply(&mut self, event: &Event) {
        if let EventKind::SnapshotReset { snapshot } = &event.kind {
            *self = (**snapshot).clone();
            self.seq = event.seq;
            return;
        }
        self.seq = event.seq;
        self.t = self.t.max(event.t);
        let t = event.t;
        match &event.kind {
            EventKind::SnapshotReset { .. } => unreachable!(),
            EventKind::SessionUpserted { session } => match self.sessions.get_mut(&session.session_id) {
                Some(s) => s.info = session.clone(),
                None => {
                    self.sessions.insert(
                        session.session_id.clone(),
                        Session {
                            info: session.clone(),
                            agents: BTreeMap::new(),
                            runs: BTreeMap::new(),
                            files: BTreeMap::new(),
                        },
                    );
                }
            },
            EventKind::SessionRemoved { session_id } => {
                self.sessions.remove(session_id);
            }
            EventKind::AgentSpawned { session_id, agent } => {
                let Some(s) = self.sessions.get_mut(session_id) else { return };
                if let Some(run_id) = &agent.run_id {
                    if let Some(run) = s.runs.get_mut(run_id) {
                        if !run.agent_ids.iter().any(|a| a == &agent.agent_id) {
                            run.agent_ids.push(agent.agent_id.clone());
                        }
                    }
                }
                s.agents.insert(agent.agent_id.clone(), (**agent).clone());
            }
            EventKind::AgentUpdated { session_id, agent } => {
                let Some(s) = self.sessions.get_mut(session_id) else { return };
                if s.agents.contains_key(&agent.agent_id) {
                    s.agents.insert(agent.agent_id.clone(), (**agent).clone());
                }
            }
            EventKind::ToolCallStarted { session_id, agent_id, call, tokens, output_tokens } => {
                let Some(a) = self.sessions.get_mut(session_id).and_then(|s| s.agents.get_mut(agent_id)) else {
                    return;
                };
                a.recent_tools.push(call.clone());
                while a.recent_tools.len() > RECENT_TOOLS_MAX {
                    a.recent_tools.remove(0);
                }
                a.tool_calls += 1;
                a.last_tool = Some(LastTool { name: call.name.clone(), summary: call.summary.clone(), at: call.started_at });
                a.tokens = *tokens;
                a.output_tokens = *output_tokens;
                a.last_activity_at = Some(a.last_activity_at.unwrap_or(0).max(t));
            }
            EventKind::ToolCallFinished { session_id, agent_id, tool_use_id, is_error } => {
                let Some(a) = self.sessions.get_mut(session_id).and_then(|s| s.agents.get_mut(agent_id)) else {
                    return;
                };
                if let Some(c) = a.recent_tools.iter_mut().find(|c| &c.tool_use_id == tool_use_id) {
                    c.ended_at = Some(t);
                    c.is_error = Some(*is_error);
                }
                a.last_activity_at = Some(a.last_activity_at.unwrap_or(0).max(t));
            }
            EventKind::FileTouched { session_id, agent_id, path, op, source, .. } => {
                let Some(s) = self.sessions.get_mut(session_id) else { return };
                if !s.agents.contains_key(agent_id) {
                    return;
                }
                let f = s
                    .files
                    .entry(path.clone())
                    .or_insert_with(|| FileEntry { path: path.clone(), touches: BTreeMap::new() });
                let touch = f.touches.entry(agent_id.clone()).or_insert_with(|| FileTouch {
                    agent_id: agent_id.clone(),
                    reads: 0,
                    writes: 0,
                    first_at: t,
                    last_at: t,
                    via_bash: false,
                });
                match op {
                    FileOp::Read => touch.reads += 1,
                    FileOp::Write => touch.writes += 1,
                }
                touch.first_at = touch.first_at.min(t);
                touch.last_at = touch.last_at.max(t);
                if *source == TouchSource::Bash {
                    touch.via_bash = true;
                }
            }
            EventKind::RunStarted { session_id, run } | EventKind::RunUpdated { session_id, run } => {
                let Some(s) = self.sessions.get_mut(session_id) else { return };
                s.runs.insert(run.run_id.clone(), (**run).clone());
            }
            EventKind::RunFinalized { session_id, run, agents } => {
                let Some(s) = self.sessions.get_mut(session_id) else { return };
                s.runs.insert(run.run_id.clone(), (**run).clone());
                for a in agents {
                    s.agents.insert(a.agent_id.clone(), a.clone());
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecordingInfo {
    pub path: String,
    pub name: String,
    pub format: u32,
    pub session_id: Option<String>,
    pub project_slug: Option<String>,
    pub started_at: EpochMs,
    pub ended_at: EpochMs,
    pub event_count: u64,
    pub complete: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayState {
    pub playing: bool,
    pub speed: f64,
    pub position: EpochMs,
    pub start: EpochMs,
    pub end: EpochMs,
    /// Quiet stretches of more than 30 s play in 1 s (contract 6.2).
    pub skip_gaps: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub mode: SourceMode,
    pub claude_dir: Option<String>,
    pub recording: Option<RecordingInfo>,
    pub replay: Option<ReplayState>,
    pub error: Option<String>,
    /// Parse and read warnings of the current source, newest first, at most [`PROBLEMS_MAX`].
    pub problems: Vec<Problem>,
}

string_enum!(pub enum ProblemReason { BadLine, BadFile, Truncated, ReadError });

/// One parse or read warning, merged per `(path, reason)`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// Source time of the latest occurrence.
    pub t: EpochMs,
    /// Relative to the Claude dir, forward slashes.
    pub path: String,
    pub reason: ProblemReason,
    pub count: u64,
}

/// Every tool call the core knows for one agent, oldest first.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolHistory {
    pub session_id: String,
    pub agent_id: String,
    pub calls: Vec<ToolCall>,
}

/// Result of `get_tool_history`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolHistory {
    /// The `seq` of the snapshot this history belongs to.
    pub seq: u64,
    pub agents: Vec<AgentToolHistory>,
    /// Replay: the time of every fs-event of the recording, ascending (the whole timeline, not only up
    /// to the position). Live: empty.
    pub timeline_times: Vec<EpochMs>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct CommandError {
    pub code: CommandErrorCode,
    pub message: String,
}

impl CommandError {
    pub fn new(code: CommandErrorCode, message: impl Into<String>) -> CommandError {
        CommandError { code, message: message.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_serialise_tagged_and_camel_case() {
        let e = Event {
            seq: 3,
            t: 10,
            kind: EventKind::ToolCallFinished {
                session_id: "s".into(),
                agent_id: "main".into(),
                tool_use_id: "toolu_1".into(),
                is_error: false,
            },
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"seq":3,"t":10,"type":"toolCallFinished","sessionId":"s","agentId":"main","toolUseId":"toolu_1","isError":false})
        );
        let back: Event = serde_json::from_value(v).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn run_serialises_final_and_nulls() {
        let run = WorkflowRun {
            run_id: "wf".into(),
            session_id: "s".into(),
            name: "n".into(),
            summary: None,
            status: RunStatus::Running,
            is_final: false,
            parent_agent_id: "main".into(),
            tool_use_id: None,
            task_id: None,
            started_at: None,
            ended_at: None,
            phases: vec![],
            agent_ids: vec![],
            total_tokens: None,
            total_tool_calls: None,
            declared_agent_count: None,
            default_model: None,
            error: None,
        };
        let v = serde_json::to_value(&run).unwrap();
        assert_eq!(v["final"], serde_json::json!(false));
        assert!(v.get("summary").unwrap().is_null());
        assert_eq!(v["agentIds"], serde_json::json!([]));
    }
}
