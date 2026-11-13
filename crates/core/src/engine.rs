//! The state engine: applies file observations and produces events plus an up-to-date snapshot
//! (docs/data-contract.md, sections 2 to 4). It reads files only through [`FileView`], so it is the
//! same for live and replay.
//!
//! Design: the engine keeps its own internal model (what it has read) and the published snapshot.
//! Tool and file events are emitted while transcript lines are read. After a batch of changes every
//! dirty session is recomputed from the internal model and compared with the snapshot; differences
//! become `sessionUpserted`, `agentUpdated` and `runUpdated` events. Every event goes through
//! [`Snapshot::apply`], so the snapshot and the stream cannot disagree.

use crate::adapter::bash;
use crate::adapter::records::{
    complete_lines, parse_journal_line, parse_meta, parse_notice, parse_record, parse_run_file,
    parse_session_file, AgentMeta, JournalLine, Record, RunFile, RunFileAgent, SessionFile, ToolUse, WorkflowLaunch,
};
use crate::adapter::script::{extract_meta, DeclaredPhase};
use crate::adapter::text::{self, cut};
use crate::adapter::{classify, normalise, ClaudePath};
use crate::contract::*;
use crate::source::{ChangeOp, FileChange, Source};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

/// Largest slice read from a file in one call. Each slice's complete lines are parsed before the next
/// is read, so memory stays bounded by this (plus one partial line) however large the file is.
const READ_CHUNK: u64 = 4 * 1024 * 1024;
/// Default for [`EngineConfig::quiet_after_ms`]: 30 minutes.
pub const QUIET_AFTER_MS: i64 = 30 * 60 * 1000;
const NOTICE_GRACE_MS: i64 = 1000;
const STATE_REASON_MAX: usize = 200;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub mode: SourceMode,
    /// Live only: a stale session whose files are older than this is not shown. `None` shows all.
    pub recent_window_ms: Option<i64>,
    /// A subagent or workflow agent that is not terminal and has had no transcript record (its own
    /// or a descendant's) for this long, by the source clock, is `stopped` with reason `noActivity`
    /// (contract 2.3). `None` turns the rule off.
    pub quiet_after_ms: Option<i64>,
}

impl EngineConfig {
    pub fn live() -> EngineConfig {
        EngineConfig { mode: SourceMode::Live, recent_window_ms: Some(24 * 3600 * 1000), quiet_after_ms: Some(QUIET_AFTER_MS) }
    }
    pub fn replay() -> EngineConfig {
        EngineConfig { mode: SourceMode::Replay, recent_window_ms: None, quiet_after_ms: Some(QUIET_AFTER_MS) }
    }
}

/// Counters for the Problems popover and for tests.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStats {
    pub lines_read: u64,
    pub bad_lines: u64,
    pub bad_files: u64,
    pub bytes_read: u64,
    pub truncations: u64,
}

struct QueueItem {
    change: FileChange,
    /// Re-read a `.json` file even when its size and mtime look unchanged.
    force: bool,
}

#[derive(Default)]
struct Track {
    read_to: u64,
    carry: Vec<u8>,
    stamp: Option<(Option<u64>, Option<EpochMs>)>,
}

/// A background command a session started and has not heard back from.
#[derive(Debug, Clone, PartialEq)]
pub struct BackgroundTask {
    pub summary: String,
    pub started_at: EpochMs,
    /// The agent that started it (`main` for the main thread).
    pub agent_id: String,
}

/// See [`Engine::session_extras`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SessionExtras {
    pub last_text: Option<String>,
    pub last_stop: Option<String>,
    pub background: Vec<BackgroundTask>,
}

#[derive(Clone)]
struct NoticeInt {
    status: Option<String>,
    ts: Option<EpochMs>,
    still_working: bool,
}

struct AgentInt {
    id: String,
    kind: AgentKind,
    run_id: Option<String>,
    meta: Option<AgentMeta>,
    journal_label: Option<String>,
    journal_phase: Option<String>,
    // task rule
    first_text_seen: bool,
    awaiting_computed: bool,
    task: Option<String>,
    // transcript-derived
    model: Option<String>,
    started_at: Option<EpochMs>,
    last_activity_at: Option<EpochMs>,
    newest_assistant_at: Option<EpochMs>,
    last_stop: Option<String>,
    /// Text of the newest assistant message that had any.
    last_text: Option<String>,
    msg_outputs: HashMap<String, u64>,
    output_total: u64,
    context: u64,
    tool_ids: HashSet<String>,
    tool_calls: u32,
    open_tools: HashMap<String, String>,
    recent: Vec<ToolCall>,
    /// Up to TOOL_HISTORY_MAX calls, oldest first; `history_base` calls were dropped from the front.
    history: VecDeque<ToolCall>,
    history_base: u64,
    history_index: HashMap<String, u64>,
    last_tool: Option<LastTool>,
    ended_at: Option<EpochMs>,
    /// Set by the quiet sweep: no activity for `quiet_after_ms` (contract 2.3).
    quiet: bool,
    // finalisation
    entry: Option<RunFileAgent>,
    synthetic_task: Option<String>,
}

impl AgentInt {
    fn new(id: &str, kind: AgentKind, run_id: Option<String>) -> AgentInt {
        AgentInt {
            id: id.to_string(),
            kind,
            run_id,
            meta: None,
            journal_label: None,
            journal_phase: None,
            first_text_seen: false,
            awaiting_computed: false,
            task: None,
            model: None,
            started_at: None,
            last_activity_at: None,
            newest_assistant_at: None,
            last_stop: None,
            last_text: None,
            msg_outputs: HashMap::new(),
            output_total: 0,
            context: 0,
            tool_ids: HashSet::new(),
            tool_calls: 0,
            open_tools: HashMap::new(),
            recent: Vec::new(),
            history: VecDeque::new(),
            history_base: 0,
            history_index: HashMap::new(),
            last_tool: None,
            ended_at: None,
            quiet: false,
            entry: None,
            synthetic_task: None,
        }
    }

    /// Forget everything read from the transcript (it was truncated).
    fn reset_transcript(&mut self) {
        let fresh = AgentInt::new(&self.id, self.kind, self.run_id.clone());
        let keep_meta = self.meta.take();
        let keep_label = self.journal_label.take();
        let keep_phase = self.journal_phase.take();
        let keep_entry = self.entry.take();
        let keep_synth = self.synthetic_task.take();
        *self = fresh;
        self.meta = keep_meta;
        self.journal_label = keep_label;
        self.journal_phase = keep_phase;
        self.entry = keep_entry;
        self.synthetic_task = keep_synth;
    }

    fn shown_tokens(&self) -> u64 {
        match &self.entry {
            Some(e) => e.tokens.unwrap_or(self.context),
            None => self.context,
        }
    }

    fn touch_time(&mut self, ts: Option<EpochMs>) {
        if let Some(ts) = ts {
            if self.started_at.is_none() {
                self.started_at = Some(ts);
            }
            self.last_activity_at = Some(self.last_activity_at.map_or(ts, |x| x.max(ts)));
        }
    }

    fn push_history(&mut self, call: ToolCall) {
        self.history_index.insert(call.tool_use_id.clone(), self.history_base + self.history.len() as u64);
        self.history.push_back(call);
        while self.history.len() > TOOL_HISTORY_MAX {
            if let Some(old) = self.history.pop_front() {
                self.history_index.remove(&old.tool_use_id);
            }
            self.history_base += 1;
        }
    }

    fn history_call(&mut self, tool_use_id: &str) -> Option<&mut ToolCall> {
        let abs = *self.history_index.get(tool_use_id)?;
        self.history.get_mut(abs.checked_sub(self.history_base)? as usize)
    }

    fn phase_title(&self) -> Option<String> {
        self.entry
            .as_ref()
            .and_then(|e| e.phase_title.clone())
            .or_else(|| self.meta.as_ref().and_then(|m| m.workflow_phase.clone()))
            .or_else(|| self.journal_phase.clone())
    }
}

#[derive(Default)]
struct RunInt {
    id: String,
    observed_at: EpochMs,
    task_id: Option<String>,
    name: Option<String>,
    summary: Option<String>,
    script_path: Option<String>,
    tool_use_id: Option<String>,
    launch_at: Option<EpochMs>,
    parent: Option<String>,
    declared: Option<Vec<DeclaredPhase>>,
    script_name: Option<String>,
    agent_ids: Vec<String>,
    started: Vec<(String, String)>,
    results: HashSet<String>,
    failed: HashSet<String>,
    file: Option<RunFile>,
}

impl RunInt {
    fn superseded(&self, agent_id: &str) -> bool {
        let Some(pos) = self.started.iter().position(|(_, a)| a == agent_id) else { return false };
        let key = &self.started[pos].0;
        self.started[pos + 1..].iter().any(|(k, a)| k == key && a != agent_id)
    }
}

struct SessionInt {
    id: String,
    pid: Option<u32>,
    file: Option<SessionFile>,
    visible: bool,
    liveness: Liveness,
    slug: Option<String>,
    has_transcript: bool,
    title: Option<String>,
    agents: BTreeMap<String, AgentInt>,
    runs: BTreeMap<String, RunInt>,
    notices: HashMap<String, NoticeInt>,
    /// Background commands still running: task id -> what it is and when it started.
    background: BTreeMap<String, BackgroundTask>,
    /// Task ids that ended (a notice or a TaskStop). Files are read in no particular order, so the
    /// end of a command can be seen before its start.
    ended_tasks: HashSet<String>,
    /// Workflow tool_use id -> (inline script, meta.name).
    workflow_inputs: HashMap<String, (Option<String>, Option<String>)>,
}

impl SessionInt {
    fn new(id: &str) -> SessionInt {
        SessionInt {
            id: id.to_string(),
            pid: None,
            file: None,
            visible: false,
            liveness: Liveness::Unknown,
            slug: None,
            has_transcript: false,
            title: None,
            agents: BTreeMap::new(),
            runs: BTreeMap::new(),
            notices: HashMap::new(),
            background: BTreeMap::new(),
            ended_tasks: HashSet::new(),
            workflow_inputs: HashMap::new(),
        }
    }

    fn end_task(&mut self, task_id: &str) {
        self.background.remove(task_id);
        self.ended_tasks.insert(task_id.to_string());
    }

    fn base(&self) -> Option<String> {
        self.slug.as_ref().map(|slug| format!("projects/{}/{}", slug, self.id))
    }

    fn agent_transcript_path(&self, agent_id: &str, run_id: Option<&str>) -> Option<String> {
        let base = self.base()?;
        Some(match run_id {
            Some(r) => format!("{base}/subagents/workflows/{r}/agent-{agent_id}.jsonl"),
            None => format!("{base}/subagents/agent-{agent_id}.jsonl"),
        })
    }

    fn stale(&self) -> bool {
        self.liveness == Liveness::Stale
    }

    /// The agent that spawned `agent_id` (contract 2.4 `parentAgentId`); `None` for main.
    fn parent_of(&self, agent_id: &str) -> Option<String> {
        let a = self.agents.get(agent_id)?;
        match a.kind {
            AgentKind::Main => None,
            AgentKind::Subagent => {
                Some(a.meta.as_ref().and_then(|m| m.parent_agent_id.clone()).unwrap_or_else(|| MAIN_AGENT_ID.into()))
            }
            AgentKind::Workflow => Some(
                a.run_id
                    .as_ref()
                    .and_then(|r| self.runs.get(r))
                    .and_then(|r| r.parent.clone())
                    .unwrap_or_else(|| MAIN_AGENT_ID.into()),
            ),
        }
    }

    fn info(&self) -> SessionInfo {
        let f = self.file.clone().unwrap_or_default();
        SessionInfo {
            session_id: self.id.clone(),
            pid: f.pid.or(self.pid),
            liveness: self.liveness,
            cwd: f.cwd,
            project_slug: if self.has_transcript { self.slug.clone() } else { None },
            name: f.name,
            title: self.title.clone(),
            status: f.status,
            kind: f.kind,
            version: f.version,
            started_at: f.started_at,
            updated_at: f.updated_at,
            status_updated_at: f.status_updated_at,
            has_transcript: self.has_transcript,
        }
    }

    // ---------- derived values (sections 2.3, 2.4, 2.6) ----------

    fn run_status(&self, r: &RunInt) -> (RunStatus, Option<EpochMs>, Option<String>) {
        if let Some(f) = &r.file {
            let ended = match (f.start_time, f.duration_ms) {
                (Some(s), Some(d)) => Some(s + d),
                _ => None,
            };
            return match f.status.as_deref() {
                Some("completed") => (RunStatus::Completed, ended, None),
                Some("failed") | Some("error") => (RunStatus::Failed, ended, None),
                Some("stopped") | Some("killed") => (RunStatus::Stopped, ended, None),
                other => (RunStatus::Stopped, ended, Some(other.unwrap_or("unknown").to_string())),
            };
        }
        if let Some(n) = r.task_id.as_ref().and_then(|t| self.notices.get(t)) {
            match n.status.as_deref() {
                Some("completed") => return (RunStatus::Completed, n.ts, None),
                Some("failed") => return (RunStatus::Failed, n.ts, None),
                Some("killed") | Some("stopped") => return (RunStatus::Stopped, n.ts, None),
                _ => {}
            }
        }
        if self.stale() {
            return (RunStatus::Stopped, None, None);
        }
        (RunStatus::Running, None, None)
    }

    fn agent_state(&self, a: &AgentInt) -> (AgentState, Option<String>) {
        use AgentState::*;
        match a.kind {
            AgentKind::Main => {
                if self.stale() {
                    return (Stopped, Some("sessionGone".into()));
                }
                // `busy` (the model is working) and `shell` (a shell command is running) mean the
                // main thread is working; `idle`, `waiting` and anything unknown mean it waits
                // for the person. No status at all (older versions) counts as running.
                match self.file.as_ref().and_then(|f| f.status.as_deref()) {
                    None | Some("busy") | Some("shell") => (Running, None),
                    Some(_) => (Waiting, None),
                }
            }
            AgentKind::Subagent => {
                let meta = a.meta.as_ref();
                if meta.is_some_and(|m| m.stopped_by_user) {
                    return (Stopped, Some("stoppedByUser".into()));
                }
                let mut resumed = false;
                if let Some(n) = self.notices.get(&a.id) {
                    let fresh = match (a.newest_assistant_at, n.ts) {
                        (Some(at), Some(nt)) => at <= nt + NOTICE_GRACE_MS,
                        _ => true,
                    };
                    if fresh {
                        match n.status.as_deref() {
                            // Stopped with background work of its own: it is expected to resume.
                            Some("completed") if n.still_working && !self.stale() && !a.quiet => {
                                return (Waiting, Some("backgroundWork".into()));
                            }
                            Some("completed") => return (Done, None),
                            Some("failed") => return (Failed, Some("failed".into())),
                            Some(s @ ("killed" | "stopped")) => return (Stopped, Some(s.into())),
                            _ => {}
                        }
                    } else {
                        // It wrote again after its last notice, so it resumed. The meta file
                        // keeps `finished: true` from the earlier stop; ignore it.
                        resumed = true;
                    }
                }
                if !resumed && meta.is_some_and(|m| m.finished) {
                    return (Done, None);
                }
                if self.stale() {
                    return (Stopped, Some("sessionGone".into()));
                }
                if a.quiet {
                    return (Stopped, Some("noActivity".into()));
                }
                (activity(a), None)
            }
            AgentKind::Workflow => {
                let run = a.run_id.as_ref().and_then(|r| self.runs.get(r));
                if let Some(run) = run {
                    if run.file.is_some() {
                        return match &a.entry {
                            Some(e) => match e.state.as_deref() {
                                Some("done") => (Done, None),
                                Some("error") => {
                                    (Failed, Some(cut(e.error.as_deref().unwrap_or("error"), STATE_REASON_MAX)))
                                }
                                Some("queued") => (Queued, None),
                                Some("running") => (Running, None),
                                other => (Stopped, Some(other.unwrap_or("unknown").to_string())),
                            },
                            None if run.superseded(&a.id) => (Failed, Some("retried".into())),
                            None => (Stopped, Some("notInRunFile".into())),
                        };
                    }
                    if run.results.contains(&a.id) {
                        return (Done, None);
                    }
                    if run.failed.contains(&a.id) {
                        return (Failed, Some("failed".into()));
                    }
                    if run.superseded(&a.id) {
                        return (Failed, Some("retried".into()));
                    }
                    if self.run_status(run).0 != RunStatus::Running {
                        return (Stopped, Some("runEnded".into()));
                    }
                }
                if self.stale() {
                    return (Stopped, Some("sessionGone".into()));
                }
                if a.quiet {
                    return (Stopped, Some("noActivity".into()));
                }
                (activity(a), None)
            }
        }
    }

    /// Phases of a run as `(index, title, detail, model)`.
    fn run_phases(&self, r: &RunInt) -> Vec<(u32, String, Option<String>, Option<String>)> {
        let mut out: Vec<(u32, String, Option<String>, Option<String>)> = Vec::new();
        if let Some(f) = &r.file {
            out.extend(f.phases.iter().map(|p| (p.index, p.title.clone(), p.detail.clone(), p.model.clone())));
            for e in &f.agents {
                if let Some(i) = e.phase_index {
                    if !out.iter().any(|p| p.0 == i) {
                        out.push((i, e.phase_title.clone().unwrap_or_default(), None, None));
                    }
                }
            }
            out.sort_by_key(|p| p.0);
            return out;
        }
        if let Some(decl) = &r.declared {
            for (i, p) in decl.iter().enumerate() {
                out.push((i as u32 + 1, p.title.clone(), p.detail.clone(), p.model.clone()));
            }
        }
        for id in &r.agent_ids {
            if let Some(title) = self.agents.get(id).and_then(AgentInt::phase_title) {
                if !out.iter().any(|p| p.1 == title) {
                    let next = out.iter().map(|p| p.0).max().unwrap_or(0) + 1;
                    out.push((next, title, None, None));
                }
            }
        }
        out
    }

    fn phase_index(&self, a: &AgentInt, phases: &[(u32, String, Option<String>, Option<String>)]) -> Option<u32> {
        if let Some(i) = a.entry.as_ref().and_then(|e| e.phase_index) {
            return Some(i);
        }
        let title = a.phase_title()?;
        phases.iter().find(|p| p.1 == title).map(|p| p.0)
    }

    fn build_agent(&self, a: &AgentInt) -> Agent {
        let (state, state_reason) = self.agent_state(a);
        let meta = a.meta.as_ref();
        let run = a.run_id.as_ref().and_then(|r| self.runs.get(r));
        let entry = a.entry.as_ref();
        let (parent, spawn_tool) = match a.kind {
            AgentKind::Main => (None, None),
            AgentKind::Subagent => (
                Some(meta.and_then(|m| m.parent_agent_id.clone()).unwrap_or_else(|| MAIN_AGENT_ID.into())),
                meta.and_then(|m| m.tool_use_id.clone()),
            ),
            AgentKind::Workflow => (
                Some(run.and_then(|r| r.parent.clone()).unwrap_or_else(|| MAIN_AGENT_ID.into())),
                run.and_then(|r| r.tool_use_id.clone()),
            ),
        };
        let label = match a.kind {
            AgentKind::Main => self
                .title
                .clone()
                .or_else(|| self.file.as_ref().and_then(|f| f.name.clone()))
                .unwrap_or_else(|| MAIN_AGENT_ID.into()),
            _ => entry
                .and_then(|e| e.label.clone())
                .or_else(|| meta.and_then(|m| m.description.clone()))
                .or_else(|| a.journal_label.clone())
                .unwrap_or_else(|| a.id.clone()),
        };
        let model = a
            .model
            .clone()
            .or_else(|| entry.and_then(|e| e.model.clone()))
            .or_else(|| meta.and_then(|m| m.model.clone()).filter(|m| m != "inherit"));
        let (phase_index, phase_title) = match run {
            Some(r) if a.kind == AgentKind::Workflow => {
                let phases = self.run_phases(r);
                let idx = self.phase_index(a, &phases);
                let title = a.phase_title().or_else(|| idx.and_then(|i| phases.iter().find(|p| p.0 == i)).map(|p| p.1.clone()));
                (idx, title)
            }
            _ => (None, a.phase_title()),
        };
        let started_at = entry.and_then(|e| e.started_at).or(a.started_at);
        let ended_at = if state.is_terminal() {
            match entry.and_then(|e| e.started_at.zip(e.duration_ms)) {
                Some((s, d)) => Some(s + d),
                None => a.ended_at.or(a.last_activity_at),
            }
        } else {
            None
        };
        let last_tool = match entry.and_then(|e| e.last_tool_name.clone()) {
            Some(name) => Some(LastTool {
                name,
                summary: entry.and_then(|e| e.last_tool_summary.clone()).unwrap_or_default(),
                at: entry
                    .and_then(|e| e.last_progress_at)
                    .or(a.last_tool.as_ref().map(|t| t.at))
                    .unwrap_or(0),
            }),
            None => a.last_tool.clone(),
        };
        Agent {
            agent_id: a.id.clone(),
            session_id: self.id.clone(),
            kind: a.kind,
            parent_agent_id: parent,
            spawn_tool_use_id: spawn_tool,
            spawn_depth: match a.kind {
                AgentKind::Main => 0,
                _ => meta.and_then(|m| m.spawn_depth).unwrap_or(1),
            },
            agent_type: meta.and_then(|m| m.agent_type.clone()),
            name: meta.and_then(|m| m.name.clone()),
            label,
            task: a.task.clone().or_else(|| a.synthetic_task.clone()).unwrap_or_default(),
            model,
            superseded: state_reason.as_deref() == Some("retried") && a.kind == AgentKind::Workflow,
            state,
            state_reason,
            run_id: a.run_id.clone(),
            phase_index,
            phase_title,
            attempt: entry.and_then(|e| e.attempt),
            queued_at: entry.and_then(|e| e.queued_at),
            started_at,
            ended_at,
            last_activity_at: a.last_activity_at,
            tokens: a.shown_tokens(),
            output_tokens: a.output_total,
            tool_calls: entry.and_then(|e| e.tool_calls).unwrap_or(a.tool_calls),
            last_tool,
            recent_tools: a.recent.clone(),
            result_preview: entry.and_then(|e| e.result_preview.clone()),
            worktree_path: meta.and_then(|m| m.worktree_path.clone()),
        }
    }

    fn build_run(&self, r: &RunInt) -> WorkflowRun {
        let (status, ended_at, error) = self.run_status(r);
        let terminal = status != RunStatus::Running;
        let phases_raw = self.run_phases(r);
        let mut phase_agents: HashMap<u32, Vec<AgentState>> = HashMap::new();
        for id in &r.agent_ids {
            let Some(a) = self.agents.get(id) else { continue };
            let (st, reason) = self.agent_state(a);
            if reason.as_deref() == Some("retried") {
                continue;
            }
            if let Some(i) = self.phase_index(a, &phases_raw) {
                phase_agents.entry(i).or_default().push(st);
            }
        }
        let phases = phases_raw
            .into_iter()
            .map(|(index, title, detail, model)| {
                let states = phase_agents.get(&index).map(Vec::as_slice).unwrap_or(&[]);
                let state = if states.is_empty() {
                    if terminal { PhaseState::Done } else { PhaseState::Pending }
                } else if states.iter().any(|s| !s.is_terminal()) {
                    PhaseState::Running
                } else if states.contains(&AgentState::Failed) {
                    PhaseState::Failed
                } else {
                    PhaseState::Done
                };
                Phase { index, title, detail, model, state }
            })
            .collect();
        let f = r.file.as_ref();
        let started_at = f.and_then(|f| f.start_time).or(r.launch_at).or_else(|| {
            r.agent_ids.iter().filter_map(|id| self.agents.get(id).and_then(|a| a.started_at)).min()
        });
        WorkflowRun {
            run_id: r.id.clone(),
            session_id: self.id.clone(),
            name: f
                .and_then(|f| f.workflow_name.clone())
                .or_else(|| r.name.clone())
                .or_else(|| r.script_name.clone())
                .unwrap_or_else(|| r.id.clone()),
            summary: f.and_then(|f| f.summary.clone()).or_else(|| r.summary.clone()),
            status,
            is_final: f.is_some(),
            parent_agent_id: r.parent.clone().unwrap_or_else(|| MAIN_AGENT_ID.into()),
            tool_use_id: r.tool_use_id.clone(),
            task_id: f.and_then(|f| f.task_id.clone()).or_else(|| r.task_id.clone()),
            started_at: started_at.or(Some(r.observed_at)),
            ended_at,
            phases,
            agent_ids: r.agent_ids.clone(),
            total_tokens: f.and_then(|f| f.total_tokens),
            total_tool_calls: f.and_then(|f| f.total_tool_calls),
            declared_agent_count: f.and_then(|f| f.agent_count),
            default_model: f.and_then(|f| f.default_model.clone()),
            error,
        }
    }
}

fn activity(a: &AgentInt) -> AgentState {
    if a.last_stop.as_deref().is_some_and(|s| s != "tool_use") {
        return AgentState::Waiting;
    }
    if !a.open_tools.is_empty() && a.open_tools.values().all(|n| matches!(n.as_str(), "Agent" | "Task" | "Workflow")) {
        return AgentState::Waiting;
    }
    AgentState::Running
}

pub struct Engine {
    cfg: EngineConfig,
    sessions: BTreeMap<String, SessionInt>,
    pid_files: BTreeMap<u32, SessionFile>,
    tracks: HashMap<String, Track>,
    queue: VecDeque<QueueItem>,
    dirty: BTreeSet<String>,
    snapshot: Snapshot,
    out: Vec<Event>,
    next_seq: u64,
    stats: EngineStats,
    problems: Vec<Problem>,
    /// While `run_queue` runs with a budget: when to stop reading and yield.
    deadline: Option<Instant>,
}

/// Whether a tail read everything up to the file's current end.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tail {
    Done,
    /// The budget ran out; the rest is read when the change is handled again.
    Yielded,
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Engine {
        let mode = cfg.mode;
        Engine {
            cfg,
            sessions: BTreeMap::new(),
            pid_files: BTreeMap::new(),
            tracks: HashMap::new(),
            queue: VecDeque::new(),
            dirty: BTreeSet::new(),
            snapshot: Snapshot::empty(mode),
            out: Vec::new(),
            next_seq: 1,
            stats: EngineStats::default(),
            problems: Vec::new(),
            deadline: None,
        }
    }

    /// Parse and read warnings, newest first (contract 3, `SourceStatus.problems`).
    pub fn problems(&self) -> Vec<Problem> {
        let mut out = self.problems.clone();
        out.sort_by(|a, b| b.t.cmp(&a.t).then_with(|| a.path.cmp(&b.path)));
        out
    }

    fn problem(&mut self, path: &str, reason: ProblemReason, t: EpochMs) {
        match self.problems.iter_mut().find(|p| p.path == path && p.reason == reason) {
            Some(p) => {
                p.count += 1;
                p.t = p.t.max(t);
            }
            None => {
                if self.problems.len() >= PROBLEMS_MAX {
                    if let Some(i) = self.problems.iter().enumerate().min_by_key(|(_, p)| p.t).map(|(i, _)| i) {
                        self.problems.remove(i);
                    }
                }
                self.problems.push(Problem { t, path: path.to_string(), reason, count: 1 });
            }
        }
    }

    /// Every tool call kept per agent of the visible sessions (`get_tool_history`).
    pub fn tool_history(&self) -> Vec<AgentToolHistory> {
        let mut out = Vec::new();
        for s in self.sessions.values().filter(|s| s.visible) {
            for a in s.agents.values().filter(|a| !a.history.is_empty()) {
                out.push(AgentToolHistory {
                    session_id: s.id.clone(),
                    agent_id: a.id.clone(),
                    calls: a.history.iter().cloned().collect(),
                });
            }
        }
        out
    }

    fn over_budget(&self) -> bool {
        self.deadline.is_some_and(|d| Instant::now() >= d)
    }

    /// Recomputes which agents are quiet (no activity for `quiet_after_ms` at `now`, counting
    /// descendants) and marks sessions whose flags changed dirty.
    fn sweep_quiet(&mut self, now: EpochMs) {
        let Some(window) = self.cfg.quiet_after_ms else { return };
        for s in self.sessions.values_mut().filter(|s| s.visible) {
            // Newest activity of each agent including its descendants.
            let mut newest: HashMap<String, EpochMs> = HashMap::new();
            for a in s.agents.values() {
                let Some(t) = a.last_activity_at else { continue };
                let mut id = a.id.clone();
                for _ in 0..32 {
                    let e = newest.entry(id.clone()).or_insert(t);
                    *e = (*e).max(t);
                    match s.parent_of(&id) {
                        Some(p) if p != id => id = p,
                        _ => break,
                    }
                }
            }
            let mut changed = false;
            for a in s.agents.values_mut() {
                let quiet = a.kind != AgentKind::Main
                    && newest.get(&a.id).is_some_and(|&t| now.saturating_sub(t) > window);
                if quiet != a.quiet {
                    a.quiet = quiet;
                    changed = true;
                }
            }
            if changed {
                self.dirty.insert(s.id.clone());
            }
        }
    }

    /// Re-evaluates the time-based rules at the source's `now` and emits what changed. The shell
    /// calls this (through the hub) on every tick, so states move even when no file changes.
    pub fn refresh_clock(&mut self, src: &dyn Source) {
        let t = src.now();
        self.sweep_quiet(t);
        self.flush(t);
    }

    /// Per-session facts the snapshot does not carry, for the attention list.
    pub fn session_extras(&self) -> BTreeMap<String, SessionExtras> {
        self.sessions
            .iter()
            .filter(|(_, s)| s.visible)
            .map(|(id, s)| {
                let main = s.agents.get(MAIN_AGENT_ID);
                (
                    id.clone(),
                    SessionExtras {
                        last_text: main.and_then(|a| a.last_text.clone()),
                        last_stop: main.and_then(|a| a.last_stop.clone()),
                        // A command started by an agent that has since ended no longer counts.
                        background: s
                            .background
                            .values()
                            .filter(|b| {
                                b.agent_id == MAIN_AGENT_ID
                                    || s.agents.get(&b.agent_id).is_some_and(|a| {
                                        !matches!(s.agent_state(a).0, AgentState::Done | AgentState::Failed | AgentState::Stopped)
                                    })
                            })
                            .cloned()
                            .collect(),
                    },
                )
            })
            .collect()
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    pub fn stats(&self) -> &EngineStats {
        &self.stats
    }
    pub fn config(&self) -> &EngineConfig {
        &self.cfg
    }
    /// The `seq` the next event will get.
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }
    pub fn set_next_seq(&mut self, seq: u64) {
        self.next_seq = seq.max(1);
    }
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.out)
    }
    pub fn has_pending(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Drops buffered events and emits one `snapshotReset` carrying the current snapshot, numbered `seq`.
    pub fn reset_event(&mut self, seq: u64) -> Event {
        self.out.clear();
        self.snapshot.seq = seq;
        self.next_seq = seq + 1;
        Event { seq, t: self.snapshot.t, kind: EventKind::SnapshotReset { snapshot: Box::new(self.snapshot.clone()) } }
    }

    fn emit(&mut self, t: EpochMs, kind: EventKind) {
        let e = Event { seq: self.next_seq, t, kind };
        self.next_seq += 1;
        self.snapshot.apply(&e);
        self.out.push(e);
    }

    pub fn enqueue(&mut self, changes: impl IntoIterator<Item = FileChange>) {
        self.queue.extend(changes.into_iter().map(|change| QueueItem { change, force: true }));
    }

    /// Queues every session file, live sessions first, then newest first.
    pub fn initial_scan(&mut self, src: &dyn Source) {
        let t = src.now();
        let mut found: Vec<(bool, EpochMs, String)> = Vec::new();
        for e in src.list_dir("sessions") {
            let path = format!("sessions/{}", e.name);
            let Some(ClaudePath::SessionFile { pid }) = classify(&path) else { continue };
            let updated = src
                .read_all(&path)
                .ok()
                .and_then(|b| parse_session_file(&b))
                .and_then(|f| f.updated_at)
                .unwrap_or(0);
            let alive = src.liveness(pid) != Liveness::Stale;
            found.push((alive, updated, path));
        }
        found.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        self.enqueue(found.into_iter().map(|(_, _, path)| FileChange { path, op: ChangeOp::Modify, t }));
    }

    /// Safety net for missed watcher events: re-lists the session files and every visible session's
    /// directories. Unchanged `.json` files are not re-read; `.jsonl` files are tailed (a no-op when
    /// they did not grow).
    pub fn rescan(&mut self, src: &dyn Source) {
        let t = src.now();
        let listed: HashSet<u32> = src
            .list_dir("sessions")
            .into_iter()
            .filter_map(|e| match classify(&format!("sessions/{}", e.name)) {
                Some(ClaudePath::SessionFile { pid }) => Some(pid),
                _ => None,
            })
            .collect();
        let mut items: Vec<QueueItem> = Vec::new();
        for pid in self.pid_files.keys() {
            if !listed.contains(pid) {
                items.push(QueueItem {
                    change: FileChange { path: format!("sessions/{pid}.json"), op: ChangeOp::Delete, t },
                    force: true,
                });
            }
        }
        for pid in &listed {
            items.push(QueueItem {
                change: FileChange { path: format!("sessions/{pid}.json"), op: ChangeOp::Modify, t },
                force: false,
            });
        }
        let visible: Vec<String> = self.sessions.values().filter(|s| s.visible).map(|s| s.id.clone()).collect();
        for sid in visible {
            for path in self.discover_paths(src, &sid) {
                items.push(QueueItem { change: FileChange { path, op: ChangeOp::Modify, t }, force: false });
            }
        }
        self.queue.extend(items);
    }

    /// Re-checks every session's pid (live) and recomputes what depends on it.
    pub fn refresh_liveness(&mut self, src: &dyn Source) {
        let t = src.now();
        let ids: Vec<String> = self.sessions.keys().cloned().collect();
        for sid in ids {
            let Some(s) = self.sessions.get_mut(&sid) else { continue };
            let Some(pid) = s.file.as_ref().and_then(|f| f.pid).or(s.pid) else { continue };
            let l = src.liveness(pid);
            if l != s.liveness {
                s.liveness = l;
                if s.visible {
                    self.dirty.insert(sid);
                } else {
                    self.reconcile_session(src, &sid, t);
                }
            }
        }
        self.flush(t);
    }

    /// Processes queued changes. With a budget, stops after it is used up (returns `false` when work
    /// remains). Dirty sessions are recomputed before returning.
    /// A large file yields part-way when the budget is used up and is read on from there next time,
    /// so one call never holds the caller (and the shell's lock) much longer than the budget.
    pub fn run_queue(&mut self, src: &dyn Source, budget: Option<Duration>) -> bool {
        self.deadline = budget.map(|b| Instant::now() + b);
        let mut last_t = src.now();
        while let Some(item) = self.queue.pop_front() {
            last_t = item.change.t;
            self.handle(src, &item);
            if self.over_budget() {
                break;
            }
        }
        self.deadline = None;
        self.sweep_quiet(src.now());
        self.flush(last_t);
        self.queue.is_empty()
    }

    /// Convenience: queue changes and process all of them.
    pub fn apply_changes(&mut self, src: &dyn Source, changes: impl IntoIterator<Item = FileChange>) {
        self.enqueue(changes);
        self.run_queue(src, None);
    }

    fn handle(&mut self, src: &dyn Source, item: &QueueItem) {
        let change = &item.change;
        let Some(cp) = classify(&change.path) else { return };
        if let ClaudePath::SessionFile { pid } = cp {
            self.on_session_file(src, pid, item);
            return;
        }
        let Some(sid) = cp.session_id().map(str::to_string) else { return };
        if !self.sessions.get(&sid).is_some_and(|s| s.visible) {
            return;
        }
        if change.op == ChangeOp::Delete {
            // Deleted transcripts and metas keep what was read from them.
            return;
        }
        let t = change.t;
        match cp {
            ClaudePath::SessionFile { .. } => unreachable!(),
            ClaudePath::MainTranscript { slug, .. } => {
                if src.size(&change.path).is_none() {
                    return;
                }
                let s = self.sessions.get_mut(&sid).unwrap();
                if s.slug.is_none() || !s.has_transcript {
                    s.slug = Some(slug);
                    s.has_transcript = true;
                }
                if self.tail_transcript(src, &sid, MAIN_AGENT_ID, &change.path, t, true) == Tail::Yielded {
                    self.queue.push_front(QueueItem { change: change.clone(), force: item.force });
                }
            }
            ClaudePath::AgentMeta { slug, run_id, agent_id, .. } => {
                self.note_slug(&sid, slug);
                if !self.json_changed(src, &change.path, item.force) {
                    return;
                }
                let Ok(bytes) = src.read_all(&change.path) else { return };
                let Some(meta) = parse_meta(&bytes) else {
                    self.stats.bad_files += 1;
                    self.problem(&change.path, ProblemReason::BadFile, t);
                    return;
                };
                let kind = if run_id.is_some() { AgentKind::Workflow } else { AgentKind::Subagent };
                self.ensure_agent(&sid, &agent_id, kind, run_id, t, true);
                if let Some(a) = self.sessions.get_mut(&sid).and_then(|s| s.agents.get_mut(&agent_id)) {
                    a.meta = Some(meta);
                }
            }
            ClaudePath::AgentTranscript { slug, run_id, agent_id, .. } => {
                self.note_slug(&sid, slug);
                if src.size(&change.path).is_none() {
                    return;
                }
                let kind = if run_id.is_some() { AgentKind::Workflow } else { AgentKind::Subagent };
                self.ensure_agent(&sid, &agent_id, kind, run_id, t, true);
                if self.tail_transcript(src, &sid, &agent_id, &change.path, t, true) == Tail::Yielded {
                    self.queue.push_front(QueueItem { change: change.clone(), force: item.force });
                }
            }
            ClaudePath::Journal { slug, run_id, .. } => {
                self.note_slug(&sid, slug);
                self.ensure_run(src, &sid, &run_id, t);
                self.tail_journal(src, &sid, &run_id, &change.path, t);
            }
            ClaudePath::RunFile { slug, run_id, .. } => {
                self.note_slug(&sid, slug);
                if !self.json_changed(src, &change.path, item.force) {
                    return;
                }
                let Ok(bytes) = src.read_all(&change.path) else { return };
                let Some(rf) = parse_run_file(&bytes) else {
                    self.stats.bad_files += 1;
                    self.problem(&change.path, ProblemReason::BadFile, t);
                    return;
                };
                self.ensure_run(src, &sid, &run_id, t);
                self.finalize_run(&sid, &run_id, rf, t);
            }
            ClaudePath::Script { slug, file_name, .. } => {
                self.note_slug(&sid, slug);
                let runs: Vec<String> = self.sessions[&sid]
                    .runs
                    .keys()
                    .filter(|r| file_name.ends_with(&format!("-{r}.js")))
                    .cloned()
                    .collect();
                for r in runs {
                    self.try_load_script(src, &sid, &r);
                }
            }
        }
        self.dirty.insert(sid);
    }

    fn note_slug(&mut self, sid: &str, slug: String) {
        if let Some(s) = self.sessions.get_mut(sid) {
            if s.slug.is_none() {
                s.slug = Some(slug);
            }
        }
    }

    /// For `.json` files: whether size or mtime changed since the last read (always true when forced).
    fn json_changed(&mut self, src: &dyn Source, path: &str, force: bool) -> bool {
        let stamp = (src.size(path), src.mtime(path));
        let track = self.tracks.entry(path.to_string()).or_default();
        let changed = force || track.stamp != Some(stamp);
        track.stamp = Some(stamp);
        changed
    }

    // ---------- sessions ----------

    fn on_session_file(&mut self, src: &dyn Source, pid: u32, item: &QueueItem) {
        let path = &item.change.path;
        let t = item.change.t;
        let exists = item.change.op != ChangeOp::Delete && src.size(path).is_some();
        if !exists {
            self.tracks.remove(path);
            if let Some(old) = self.pid_files.remove(&pid) {
                self.reconcile_session(src, &old.session_id, t);
            }
            return;
        }
        if !self.json_changed(src, path, item.force) {
            return;
        }
        let Some(mut f) = src.read_all(path).ok().and_then(|b| parse_session_file(&b)) else {
            self.stats.bad_files += 1;
            self.problem(path, ProblemReason::BadFile, t);
            return;
        };
        if f.pid.is_none() {
            f.pid = Some(pid);
        }
        let sid = f.session_id.clone();
        let old = self.pid_files.insert(pid, f);
        if let Some(old) = old.filter(|o| o.session_id != sid) {
            self.reconcile_session(src, &old.session_id, t);
        }
        self.reconcile_session(src, &sid, t);
    }

    fn reconcile_session(&mut self, src: &dyn Source, sid: &str, t: EpochMs) {
        let winner = self
            .pid_files
            .iter()
            .filter(|(_, f)| f.session_id == sid)
            .max_by_key(|(_, f)| f.updated_at.unwrap_or(0))
            .map(|(pid, f)| (*pid, f.clone()));
        let Some((pid, file)) = winner else {
            if let Some(s) = self.sessions.remove(sid) {
                if s.visible {
                    self.emit(t, EventKind::SessionRemoved { session_id: sid.to_string() });
                }
                if let Some(base) = s.base() {
                    self.tracks.retain(|p, _| !p.starts_with(&base));
                }
                self.dirty.remove(sid);
            }
            return;
        };
        let liveness = src.liveness(pid);
        let s = self.sessions.entry(sid.to_string()).or_insert_with(|| SessionInt::new(sid));
        s.pid = Some(pid);
        s.liveness = liveness;
        s.file = Some(file.clone());
        if s.visible {
            self.dirty.insert(sid.to_string());
            return;
        }
        let show = match self.cfg.recent_window_ms {
            None => true,
            Some(_) if liveness != Liveness::Stale => true,
            Some(w) => {
                let newest = [file.updated_at, file.status_updated_at, src.mtime(&format!("sessions/{pid}.json"))]
                    .into_iter()
                    .flatten()
                    .max()
                    .unwrap_or(0);
                newest >= src.now() - w
            }
        };
        if !show {
            return;
        }
        s.visible = true;
        let info = s.info();
        self.emit(t, EventKind::SessionUpserted { session: info });
        self.ensure_agent(sid, MAIN_AGENT_ID, AgentKind::Main, None, t, true);
        let paths = self.discover_paths(src, sid);
        self.queue.extend(paths.into_iter().map(|path| QueueItem {
            change: FileChange { path, op: ChangeOp::Modify, t },
            force: true,
        }));
    }

    /// Every file of a session, in the order they are best read: main transcript, metas, agent
    /// transcripts, journals, run files.
    fn discover_paths(&mut self, src: &dyn Source, sid: &str) -> Vec<String> {
        let Some(s) = self.sessions.get(sid) else { return Vec::new() };
        let slug = match &s.slug {
            Some(slug) => Some(slug.clone()),
            None => src
                .list_dir("projects")
                .into_iter()
                .filter(|e| e.is_dir)
                .find(|e| src.size(&format!("projects/{}/{}.jsonl", e.name, sid)).is_some())
                .map(|e| e.name),
        };
        let Some(slug) = slug else { return Vec::new() };
        if let Some(s) = self.sessions.get_mut(sid) {
            s.slug = Some(slug.clone());
        }
        let base = format!("projects/{slug}/{sid}");
        let mut main = Vec::new();
        let main_path = format!("projects/{slug}/{sid}.jsonl");
        if src.size(&main_path).is_some() {
            main.push(main_path);
        }
        let (mut metas, mut transcripts, mut journals, mut run_files) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut sort_dir = |dir: &str, metas: &mut Vec<String>, transcripts: &mut Vec<String>| {
            let mut entries = src.list_dir(dir);
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            for e in entries.into_iter().filter(|e| !e.is_dir) {
                let p = format!("{dir}/{}", e.name);
                if e.name.ends_with(".meta.json") {
                    metas.push(p);
                } else if e.name == "journal.jsonl" {
                    journals.push(p);
                } else if e.name.starts_with("agent-") && e.name.ends_with(".jsonl") {
                    transcripts.push(p);
                }
            }
        };
        sort_dir(&format!("{base}/subagents"), &mut metas, &mut transcripts);
        let mut run_dirs: Vec<String> =
            src.list_dir(&format!("{base}/subagents/workflows")).into_iter().filter(|e| e.is_dir).map(|e| e.name).collect();
        run_dirs.sort();
        for r in run_dirs {
            sort_dir(&format!("{base}/subagents/workflows/{r}"), &mut metas, &mut transcripts);
        }
        let mut wf: Vec<String> = src
            .list_dir(&format!("{base}/workflows"))
            .into_iter()
            .filter(|e| !e.is_dir && e.name.ends_with(".json"))
            .map(|e| format!("{base}/workflows/{}", e.name))
            .collect();
        wf.sort();
        run_files.extend(wf);
        main.into_iter().chain(metas).chain(transcripts).chain(journals).chain(run_files).collect()
    }

    // ---------- agents and runs ----------

    fn ensure_run(&mut self, src: &dyn Source, sid: &str, run_id: &str, t: EpochMs) {
        let Some(s) = self.sessions.get_mut(sid) else { return };
        if s.runs.contains_key(run_id) {
            return;
        }
        s.runs.insert(run_id.to_string(), RunInt { id: run_id.to_string(), observed_at: t, ..Default::default() });
        self.try_load_script(src, sid, run_id);
        let s = &self.sessions[sid];
        let run = s.build_run(&s.runs[run_id]);
        self.emit(t, EventKind::RunStarted { session_id: sid.to_string(), run: Box::new(run) });
    }

    fn ensure_agent(&mut self, sid: &str, agent_id: &str, kind: AgentKind, run_id: Option<String>, t: EpochMs, emit: bool) {
        let Some(s) = self.sessions.get_mut(sid) else { return };
        if s.agents.contains_key(agent_id) {
            return;
        }
        if let Some(r) = &run_id {
            if !s.runs.contains_key(r) {
                s.runs.insert(r.clone(), RunInt { id: r.clone(), observed_at: t, ..Default::default() });
                let run = s.build_run(&s.runs[r]);
                self.emit(t, EventKind::RunStarted { session_id: sid.to_string(), run: Box::new(run) });
            }
        }
        let s = self.sessions.get_mut(sid).unwrap();
        if let Some(r) = run_id.as_ref().and_then(|r| s.runs.get_mut(r)) {
            if !r.agent_ids.iter().any(|a| a == agent_id) {
                r.agent_ids.push(agent_id.to_string());
            }
        }
        s.agents.insert(agent_id.to_string(), AgentInt::new(agent_id, kind, run_id));
        if emit {
            let agent = s.build_agent(&s.agents[agent_id]);
            self.emit(t, EventKind::AgentSpawned { session_id: sid.to_string(), agent: Box::new(agent) });
        }
    }

    fn try_load_script(&mut self, src: &dyn Source, sid: &str, run_id: &str) {
        let Some(s) = self.sessions.get(sid) else { return };
        let Some(r) = s.runs.get(run_id) else { return };
        if r.declared.is_some() || r.file.is_some() {
            return;
        }
        let mut text: Option<String> = r
            .tool_use_id
            .as_ref()
            .and_then(|id| s.workflow_inputs.get(id))
            .and_then(|(script, _)| script.clone());
        if text.is_none() {
            if let Some(p) = r.script_path.as_deref().and_then(reroot) {
                text = src.read_all(&p).ok().map(|b| String::from_utf8_lossy(&b).into_owned());
            }
        }
        if text.is_none() {
            if let Some(base) = s.base() {
                let dir = format!("{base}/workflows/scripts");
                let suffix = format!("-{run_id}.js");
                let hits: Vec<String> =
                    src.list_dir(&dir).into_iter().filter(|e| e.name.ends_with(&suffix)).map(|e| e.name).collect();
                if hits.len() == 1 {
                    text = src.read_all(&format!("{dir}/{}", hits[0])).ok().map(|b| String::from_utf8_lossy(&b).into_owned());
                }
            }
        }
        let Some(text) = text else { return };
        let meta = extract_meta(&text);
        let r = self.sessions.get_mut(sid).unwrap().runs.get_mut(run_id).unwrap();
        if r.script_name.is_none() {
            r.script_name = meta.name;
        }
        if let Some(p) = meta.phases.filter(|p| !p.is_empty()) {
            r.declared = Some(p);
        }
    }

    fn finalize_run(&mut self, sid: &str, run_id: &str, rf: RunFile, t: EpochMs) {
        let s = self.sessions.get_mut(sid).unwrap();
        // Clear entries from a previous finalisation.
        for a in s.agents.values_mut().filter(|a| a.run_id.as_deref() == Some(run_id)) {
            a.entry = None;
        }
        for e in &rf.agents {
            let id = e.agent_id.clone().unwrap_or_else(|| format!("{run_id}#{}", e.index));
            let r = s.runs.get_mut(run_id).unwrap();
            if !r.agent_ids.contains(&id) {
                r.agent_ids.push(id.clone());
            }
            let a = s
                .agents
                .entry(id.clone())
                .or_insert_with(|| AgentInt::new(&id, AgentKind::Workflow, Some(run_id.to_string())));
            if e.agent_id.is_none() {
                a.synthetic_task = e.prompt_preview.as_deref().map(text::task_from_single_prompt);
            }
            a.entry = Some(e.clone());
        }
        let r = s.runs.get_mut(run_id).unwrap();
        r.file = Some(rf);
        let ids = r.agent_ids.clone();
        self.update_ended(sid, t);
        let s = &self.sessions[sid];
        let run = s.build_run(&s.runs[run_id]);
        let agents: Vec<Agent> = ids.iter().filter_map(|id| s.agents.get(id)).map(|a| s.build_agent(a)).collect();
        self.emit(t, EventKind::RunFinalized { session_id: sid.to_string(), run: Box::new(run), agents });
    }

    // ---------- journals ----------

    fn tail_journal(&mut self, src: &dyn Source, sid: &str, run_id: &str, path: &str, t: EpochMs) {
        // Journals are small (a few lines per agent), so they are read whole.
        let Some((size, _)) = self.begin_tail(src, path, t) else { return };
        let mut parsed: Vec<JournalLine> = Vec::new();
        while let Some(lines) = self.next_chunk(src, path, size, t) {
            for l in &lines {
                match parse_journal_line(l) {
                    Some(p) => parsed.push(p),
                    None => {
                        self.stats.bad_lines += 1;
                        self.problem(path, ProblemReason::BadLine, t);
                    }
                }
            }
        }
        if parsed.is_empty() {
            return;
        }
        // Read the agents' transcripts up to their current end first (contract 4, step 5).
        let mut catch_up: Vec<String> = self.sessions[sid]
            .runs
            .get(run_id)
            .map(|r| r.agent_ids.clone())
            .unwrap_or_default();
        for l in &parsed {
            if let JournalLine::Started { agent_id, .. } | JournalLine::Result { agent_id, .. } | JournalLine::Failed { agent_id, .. } = l {
                if !agent_id.is_empty() && !catch_up.contains(agent_id) {
                    catch_up.push(agent_id.clone());
                }
            }
        }
        for aid in catch_up {
            let Some(p) = self.sessions[sid].agent_transcript_path(&aid, Some(run_id)) else { continue };
            if src.size(&p).is_some() {
                self.ensure_agent(sid, &aid, AgentKind::Workflow, Some(run_id.to_string()), t, true);
                self.tail_transcript(src, sid, &aid, &p, t, false);
            }
        }
        for l in parsed {
            match l {
                JournalLine::Started { key, agent_id, label, phase } if !agent_id.is_empty() => {
                    self.ensure_agent(sid, &agent_id, AgentKind::Workflow, Some(run_id.to_string()), t, false);
                    let s = self.sessions.get_mut(sid).unwrap();
                    let a = s.agents.get_mut(&agent_id).unwrap();
                    a.journal_label = label;
                    a.journal_phase = phase;
                    let r = s.runs.get_mut(run_id).unwrap();
                    r.started.push((key, agent_id.clone()));
                    if !self.snapshot.sessions.get(sid).is_some_and(|ss| ss.agents.contains_key(&agent_id)) {
                        let s = &self.sessions[sid];
                        let agent = s.build_agent(&s.agents[&agent_id]);
                        self.emit(t, EventKind::AgentSpawned { session_id: sid.to_string(), agent: Box::new(agent) });
                    }
                }
                JournalLine::Result { agent_id, .. } if !agent_id.is_empty() => {
                    self.sessions.get_mut(sid).unwrap().runs.get_mut(run_id).unwrap().results.insert(agent_id);
                }
                JournalLine::Failed { agent_id, .. } if !agent_id.is_empty() => {
                    self.sessions.get_mut(sid).unwrap().runs.get_mut(run_id).unwrap().failed.insert(agent_id);
                }
                _ => {}
            }
        }
        // Journal lines carry no time: endedAt is the agent's lastActivityAt now.
        self.update_ended(sid, t);
    }

    // ---------- transcripts ----------

    /// Starts reading what was appended to `path`: returns its size now and whether it was truncated
    /// (it got shorter than what was read; it is then read again from zero). `None` when it is gone.
    fn begin_tail(&mut self, src: &dyn Source, path: &str, t: EpochMs) -> Option<(u64, bool)> {
        let size = src.size(path)?;
        let truncated = self.tracks.get(path).is_some_and(|tr| size < tr.read_to);
        if truncated {
            self.stats.truncations += 1;
            self.problem(path, ProblemReason::Truncated, t);
            self.tracks.remove(path);
        }
        Some((size, truncated))
    }

    /// Reads the next slice (at most READ_CHUNK bytes) of `path` up to `size` and returns its complete
    /// lines; a partial last line is carried to the next call. `None` when there is nothing left.
    fn next_chunk(&mut self, src: &dyn Source, path: &str, size: u64, t: EpochMs) -> Option<Vec<Vec<u8>>> {
        let track = self.tracks.entry(path.to_string()).or_default();
        if track.read_to >= size {
            return None;
        }
        let to = (track.read_to + READ_CHUNK).min(size);
        let bytes = match src.read_range(path, track.read_to, to) {
            Ok(b) if !b.is_empty() => b,
            Ok(_) => return None,
            Err(_) => {
                self.problem(path, ProblemReason::ReadError, t);
                return None;
            }
        };
        track.read_to += bytes.len() as u64;
        self.stats.bytes_read += bytes.len() as u64;
        let mut buf = std::mem::take(&mut track.carry);
        buf.extend_from_slice(&bytes);
        drop(bytes);
        let (complete, used) = complete_lines(&buf);
        let lines: Vec<Vec<u8>> = complete.into_iter().map(<[u8]>::to_vec).collect();
        track.carry = buf[used..].to_vec();
        self.stats.lines_read += lines.len() as u64;
        Some(lines)
    }

    /// Reads and applies the records appended to an agent's transcript, one slice at a time. With
    /// `may_yield`, stops between slices once the run budget is used up.
    fn tail_transcript(&mut self, src: &dyn Source, sid: &str, agent_id: &str, path: &str, t: EpochMs, may_yield: bool) -> Tail {
        let Some((size, truncated)) = self.begin_tail(src, path, t) else { return Tail::Done };
        if truncated {
            if let Some(a) = self.sessions.get_mut(sid).and_then(|s| s.agents.get_mut(agent_id)) {
                a.reset_transcript();
            }
        }
        while let Some(lines) = self.next_chunk(src, path, size, t) {
            for line in lines {
                match parse_record(&line) {
                    Ok(Some(rec)) => self.apply_record(src, sid, agent_id, rec, t),
                    Ok(None) => {}
                    Err(()) => {
                        self.stats.bad_lines += 1;
                        self.problem(path, ProblemReason::BadLine, t);
                    }
                }
            }
            if may_yield && self.over_budget() && self.tracks.get(path).is_some_and(|tr| tr.read_to < size) {
                return Tail::Yielded;
            }
        }
        Tail::Done
    }

    fn apply_record(&mut self, src: &dyn Source, sid: &str, agent_id: &str, rec: Record, t_obs: EpochMs) {
        let Some(s) = self.sessions.get_mut(sid) else { return };
        let session_cwd = s.file.as_ref().and_then(|f| f.cwd.clone());
        match rec {
            Record::AiTitle(title) => {
                if agent_id == MAIN_AGENT_ID {
                    s.title = Some(title);
                }
            }
            Record::Notice { ts, notice } => {
                s.end_task(&notice.task_id);
                s.notices.insert(notice.task_id.clone(), NoticeInt { status: notice.status, ts, still_working: notice.still_working });
            }
            Record::User { ts, text, results, workflow_launch, background_task_id, .. } => {
                if let Some(task_id) = background_task_id.filter(|id| !s.ended_tasks.contains(id)) {
                    let summary = results
                        .first()
                        .and_then(|r| s.agents.get(agent_id)?.recent.iter().find(|c| c.tool_use_id == r.tool_use_id))
                        .map(|c| c.summary.clone())
                        .unwrap_or_default();
                    s.background.insert(task_id, BackgroundTask { summary, started_at: ts.unwrap_or(t_obs), agent_id: agent_id.to_string() });
                }
                let Some(a) = s.agents.get_mut(agent_id) else { return };
                a.touch_time(ts);
                if let Some(text) = &text {
                    if let Some(n) = parse_notice(text) {
                        s.background.remove(&n.task_id);
                        s.ended_tasks.insert(n.task_id.clone());
                        s.notices.insert(n.task_id.clone(), NoticeInt { status: n.status, ts, still_working: n.still_working });
                    } else {
                        apply_task_rule(a, text);
                    }
                }
                let t = ts.unwrap_or(t_obs);
                let mut finished = Vec::new();
                for r in &results {
                    if a.open_tools.remove(&r.tool_use_id).is_some() {
                        if let Some(c) = a.recent.iter_mut().find(|c| c.tool_use_id == r.tool_use_id) {
                            c.ended_at = Some(t);
                            c.is_error = Some(r.is_error);
                        }
                        if let Some(c) = a.history_call(&r.tool_use_id) {
                            c.ended_at = Some(t);
                            c.is_error = Some(r.is_error);
                        }
                        finished.push((r.tool_use_id.clone(), r.is_error));
                    }
                }
                for (tool_use_id, is_error) in finished {
                    self.emit(
                        t,
                        EventKind::ToolCallFinished {
                            session_id: sid.to_string(),
                            agent_id: agent_id.to_string(),
                            tool_use_id,
                            is_error,
                        },
                    );
                }
                if let Some(l) = workflow_launch {
                    let tool_use_id = results.first().map(|r| r.tool_use_id.clone());
                    self.on_workflow_launch(src, sid, agent_id, l, tool_use_id, ts, t_obs);
                }
            }
            Record::Assistant { ts, cwd, message_id, model, stop_reason, usage, is_api_error, tool_uses, text } => {
                let Some(a) = s.agents.get_mut(agent_id) else { return };
                a.touch_time(ts);
                if text.is_some() && !is_api_error {
                    a.last_text = text;
                }
                if let Some(ts) = ts {
                    a.newest_assistant_at = Some(a.newest_assistant_at.map_or(ts, |x| x.max(ts)));
                }
                if a.model.is_none() && !is_api_error {
                    a.model = model.filter(|m| !m.starts_with('<') && !m.is_empty());
                }
                a.last_stop = stop_reason;
                if let (Some(u), false) = (usage, is_api_error) {
                    let id = message_id.unwrap_or_default();
                    let prev = a.msg_outputs.insert(id, u.output).unwrap_or(0);
                    a.output_total = a.output_total.saturating_sub(prev) + u.output;
                    a.context = u.context();
                }
                let cwd = cwd.or(session_cwd);
                let t = ts.unwrap_or(t_obs);
                for tu in tool_uses {
                    self.on_tool_use(sid, agent_id, tu, cwd.as_deref(), t);
                }
            }
        }
    }

    fn on_tool_use(&mut self, sid: &str, agent_id: &str, tu: ToolUse, cwd: Option<&str>, t: EpochMs) {
        let s = self.sessions.get_mut(sid).unwrap();
        let mut workflow_name = None;
        if tu.name == "Workflow" {
            let script = tu.input.get("script").and_then(|v| v.as_str()).map(str::to_string);
            workflow_name = script.as_deref().and_then(|sc| extract_meta(sc).name);
            s.workflow_inputs.insert(tu.id.clone(), (script, workflow_name.clone()));
        }
        if tu.name == "TaskStop" {
            if let Some(id) = tu.input.get("task_id").and_then(|v| v.as_str()) {
                s.end_task(id);
            }
        }
        let a = s.agents.get_mut(agent_id).unwrap();
        if !a.tool_ids.insert(tu.id.clone()) {
            return;
        }
        let summary = text::tool_summary(&tu.name, &tu.input, cwd, workflow_name.as_deref());
        let call = ToolCall {
            tool_use_id: tu.id.clone(),
            name: tu.name.clone(),
            summary: summary.clone(),
            started_at: t,
            ended_at: None,
            is_error: None,
        };
        a.tool_calls += 1;
        a.open_tools.insert(tu.id.clone(), tu.name.clone());
        a.recent.push(call.clone());
        a.push_history(call.clone());
        while a.recent.len() > RECENT_TOOLS_MAX {
            a.recent.remove(0);
        }
        a.last_tool = Some(LastTool { name: tu.name.clone(), summary, at: t });
        let (tokens, output_tokens) = (a.shown_tokens(), a.output_total);
        self.emit(
            t,
            EventKind::ToolCallStarted {
                session_id: sid.to_string(),
                agent_id: agent_id.to_string(),
                call,
                tokens,
                output_tokens,
            },
        );
        for (path, op, source) in file_touches(&tu, cwd) {
            self.emit(
                t,
                EventKind::FileTouched {
                    session_id: sid.to_string(),
                    agent_id: agent_id.to_string(),
                    path,
                    op,
                    source,
                    tool_use_id: tu.id.clone(),
                },
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn on_workflow_launch(
        &mut self,
        src: &dyn Source,
        sid: &str,
        agent_id: &str,
        l: WorkflowLaunch,
        tool_use_id: Option<String>,
        ts: Option<EpochMs>,
        t_obs: EpochMs,
    ) {
        let run_id = l.run_id.clone();
        let existed = self.sessions[sid].runs.contains_key(&run_id);
        if !existed {
            let r = RunInt { id: run_id.clone(), observed_at: ts.unwrap_or(t_obs), ..Default::default() };
            self.sessions.get_mut(sid).unwrap().runs.insert(run_id.clone(), r);
        }
        {
            let r = self.sessions.get_mut(sid).unwrap().runs.get_mut(&run_id).unwrap();
            r.task_id = l.task_id.or(r.task_id.take());
            r.name = l.workflow_name.or(r.name.take());
            r.summary = l.summary.or(r.summary.take());
            r.script_path = l.script_path.or(r.script_path.take());
            r.tool_use_id = tool_use_id.or(r.tool_use_id.take());
            r.launch_at = ts.or(r.launch_at);
            r.parent = Some(agent_id.to_string());
        }
        self.try_load_script(src, sid, &run_id);
        if !existed {
            let s = &self.sessions[sid];
            let run = s.build_run(&s.runs[&run_id]);
            self.emit(ts.unwrap_or(t_obs), EventKind::RunStarted { session_id: sid.to_string(), run: Box::new(run) });
        }
        self.dirty.insert(sid.to_string());
    }

    // ---------- recompute and diff ----------

    /// Sets `endedAt` of terminal agents to their `lastActivityAt` (the observation time when there is
    /// none); clears it for revived ones.
    fn update_ended(&mut self, sid: &str, t: EpochMs) {
        let Some(s) = self.sessions.get(sid) else { return };
        let states: Vec<(String, bool)> =
            s.agents.values().map(|a| (a.id.clone(), s.agent_state(a).0.is_terminal())).collect();
        let s = self.sessions.get_mut(sid).unwrap();
        for (id, terminal) in states {
            let a = s.agents.get_mut(&id).unwrap();
            if terminal {
                // lastActivityAt once known: an agent can turn terminal before its transcript
                // has been read (a completion notice), or change terminal reason later.
                a.ended_at = a.last_activity_at.or(a.ended_at).or(Some(t));
            } else {
                a.ended_at = None;
            }
        }
    }

    fn flush(&mut self, t: EpochMs) {
        let dirty: Vec<String> = std::mem::take(&mut self.dirty).into_iter().collect();
        for sid in dirty {
            if !self.sessions.get(&sid).is_some_and(|s| s.visible) {
                continue;
            }
            self.update_ended(&sid, t);
            let s = &self.sessions[&sid];
            let info = s.info();
            let agents: Vec<Agent> = s.agents.values().map(|a| s.build_agent(a)).collect();
            let runs: Vec<WorkflowRun> = s.runs.values().map(|r| s.build_run(r)).collect();
            let snap = self.snapshot.sessions.get(&sid);
            if snap.map(|ss| &ss.info) != Some(&info) {
                self.emit(t, EventKind::SessionUpserted { session: info });
            }
            for agent in agents {
                let old = self.snapshot.sessions.get(&sid).and_then(|ss| ss.agents.get(&agent.agent_id));
                let (known, same) = (old.is_some(), old == Some(&agent));
                let session_id = sid.clone();
                let agent = Box::new(agent);
                match (known, same) {
                    (true, true) => {}
                    (true, false) => self.emit(t, EventKind::AgentUpdated { session_id, agent }),
                    (false, _) => self.emit(t, EventKind::AgentSpawned { session_id, agent }),
                }
            }
            for run in runs {
                let old = self.snapshot.sessions.get(&sid).and_then(|ss| ss.runs.get(&run.run_id));
                let (known, same) = (old.is_some(), old == Some(&run));
                let session_id = sid.clone();
                let run = Box::new(run);
                match (known, same) {
                    (true, true) => {}
                    (true, false) => self.emit(t, EventKind::RunUpdated { session_id, run }),
                    (false, _) => self.emit(t, EventKind::RunStarted { session_id, run }),
                }
            }
        }
    }
}

fn apply_task_rule(a: &mut AgentInt, text: &str) {
    if a.task.is_some() {
        return;
    }
    if a.kind == AgentKind::Main {
        if !text.trim_start().starts_with('<') && !text.trim().is_empty() {
            a.task = Some(text::tidy_task(text));
        }
        return;
    }
    if !a.first_text_seen {
        a.first_text_seen = true;
        if text::is_harness_frame(text) {
            a.awaiting_computed = true;
        } else {
            a.task = Some(text::tidy_task(text));
            return;
        }
    }
    if a.awaiting_computed && text::is_computed_task(text) {
        a.task = Some(text::tidy_task(&text::unframe_computed_task(text)));
        a.awaiting_computed = false;
    }
}

/// File touches of one tool call (contract 2.7).
fn file_touches(tu: &ToolUse, cwd: Option<&str>) -> Vec<(String, FileOp, TouchSource)> {
    let field = |k: &str| tu.input.get(k).and_then(|v| v.as_str());
    let one = |k: &str, op: FileOp| {
        field(k).and_then(|p| normalise(p, cwd)).map(|p| vec![(p, op, TouchSource::Tool)]).unwrap_or_default()
    };
    match tu.name.as_str() {
        "Read" => one("file_path", FileOp::Read),
        "Edit" | "Write" | "MultiEdit" => one("file_path", FileOp::Write),
        "NotebookEdit" => one("notebook_path", FileOp::Write),
        "Bash" => field("command")
            .map(|c| {
                bash::touches(c)
                    .into_iter()
                    .filter_map(|(p, op)| normalise(&p, cwd).map(|p| (p, op, TouchSource::Bash)))
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Re-roots an absolute path inside some machine's Claude dir: keeps the part from `projects/` on.
fn reroot(path: &str) -> Option<String> {
    let p = path.replace('\\', "/");
    let i = p.rfind("/projects/")?;
    Some(p[i + 1..].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{DirEntry, FileView};
    use std::sync::Mutex;

    /// An in-memory Claude dir.
    #[derive(Default)]
    struct MemSource {
        files: Mutex<BTreeMap<String, Vec<u8>>>,
        /// Source clock; 0 means the default 1_000.
        now: std::sync::atomic::AtomicI64,
    }

    impl MemSource {
        fn put(&self, path: &str, bytes: &[u8]) {
            self.files.lock().unwrap().insert(path.to_string(), bytes.to_vec());
        }
        fn append(&self, path: &str, bytes: &[u8]) {
            self.files.lock().unwrap().entry(path.to_string()).or_default().extend_from_slice(bytes);
        }
    }

    impl FileView for MemSource {
        fn list_dir(&self, dir: &str) -> Vec<DirEntry> {
            let prefix = format!("{dir}/");
            let mut out: BTreeMap<String, bool> = BTreeMap::new();
            for p in self.files.lock().unwrap().keys() {
                if let Some(rest) = p.strip_prefix(&prefix) {
                    match rest.find('/') {
                        Some(i) => out.insert(rest[..i].to_string(), true),
                        None => out.insert(rest.to_string(), false),
                    };
                }
            }
            out.into_iter().map(|(name, is_dir)| DirEntry { name, is_dir }).collect()
        }
        fn size(&self, path: &str) -> Option<u64> {
            self.files.lock().unwrap().get(path).map(|b| b.len() as u64)
        }
        fn mtime(&self, _path: &str) -> Option<EpochMs> {
            None
        }
        fn read_range(&self, path: &str, from: u64, to: u64) -> std::io::Result<Vec<u8>> {
            let f = self.files.lock().unwrap();
            let b = f.get(path).ok_or(std::io::ErrorKind::NotFound)?;
            let to = (to as usize).min(b.len());
            Ok(b[(from as usize).min(to)..to].to_vec())
        }
        fn read_all(&self, path: &str) -> std::io::Result<Vec<u8>> {
            self.files.lock().unwrap().get(path).cloned().ok_or_else(|| std::io::ErrorKind::NotFound.into())
        }
    }

    impl Source for MemSource {
        fn mode(&self) -> SourceMode {
            SourceMode::Live
        }
        fn poll(&mut self) -> Vec<FileChange> {
            Vec::new()
        }
        fn now(&self) -> EpochMs {
            match self.now.load(std::sync::atomic::Ordering::SeqCst) {
                0 => 1_000,
                t => t,
            }
        }
        fn liveness(&self, _pid: u32) -> Liveness {
            Liveness::Alive
        }
    }

    const MAIN: &str = "projects/-p/s1.jsonl";

    fn setup() -> (MemSource, Engine) {
        let src = MemSource::default();
        src.put("sessions/7.json", br#"{"pid":7,"sessionId":"s1","cwd":"/p","status":"busy","updatedAt":5}"#);
        src.put(MAIN, b"");
        let mut e = Engine::new(EngineConfig { mode: SourceMode::Live, recent_window_ms: None, quiet_after_ms: Some(QUIET_AFTER_MS) });
        e.initial_scan(&src);
        e.run_queue(&src, None);
        (src, e)
    }

    fn change(e: &mut Engine, src: &MemSource, path: &str) {
        e.apply_changes(src, [FileChange { path: path.into(), op: ChangeOp::Modify, t: 2_000 }]);
    }

    fn main_agent(e: &Engine) -> Agent {
        e.snapshot().sessions["s1"].agents[MAIN_AGENT_ID].clone()
    }

    fn assistant(msg: &str, block: &str, out: u64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"2026-01-01T00:00:01.000Z","cwd":"/p","message":{{"id":"{msg}","model":"claude-x","stop_reason":"tool_use","usage":{{"input_tokens":10,"cache_read_input_tokens":100,"output_tokens":{out}}},"content":[{block}]}}}}"#
        ) + "\n"
    }

    fn read_block(id: &str, path: &str) -> String {
        format!(r#"{{"type":"tool_use","id":"{id}","name":"Read","input":{{"file_path":"{path}"}}}}"#)
    }

    #[test]
    fn tails_incrementally_and_waits_for_partial_lines() {
        let (src, mut e) = setup();
        assert_eq!(main_agent(&e).tool_calls, 0);
        let line = assistant("m1", &read_block("t1", "src/a.ts"), 5);
        let (a, b) = line.as_bytes().split_at(40);
        src.append(MAIN, a);
        change(&mut e, &src, MAIN);
        assert_eq!(main_agent(&e).tool_calls, 0, "a partial line is not parsed");
        assert_eq!(e.stats().bad_lines, 0);
        src.append(MAIN, b);
        change(&mut e, &src, MAIN);
        let m = main_agent(&e);
        assert_eq!(m.tool_calls, 1);
        assert_eq!(m.tokens, 115);
        assert_eq!(m.recent_tools[0].summary, "src/a.ts");
        assert!(e.snapshot().sessions["s1"].files.contains_key("/p/src/a.ts"));
        // Re-observing an unchanged file emits nothing.
        e.take_events();
        change(&mut e, &src, MAIN);
        assert!(e.take_events().is_empty());
        // The result arrives.
        src.append(MAIN, br#"{"type":"user","timestamp":"2026-01-01T00:00:02.000Z","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"x","is_error":true}]}}"#);
        src.append(MAIN, b"\nnot json\n");
        change(&mut e, &src, MAIN);
        let m = main_agent(&e);
        assert_eq!(m.recent_tools[0].is_error, Some(true));
        assert_eq!(m.recent_tools[0].ended_at, Some(1767225602000));
        assert_eq!(e.stats().bad_lines, 1, "a bad line is skipped and counted");
    }

    #[test]
    fn dedupes_usage_by_message_id() {
        let (src, mut e) = setup();
        let thinking = r#"{"type":"thinking","thinking":"","signature":"x"}"#;
        src.append(MAIN, assistant("m1", thinking, 7).as_bytes());
        src.append(MAIN, assistant("m1", &read_block("t1", "/x.md"), 7).as_bytes());
        src.append(MAIN, assistant("m2", &read_block("t2", "/y.md"), 3).as_bytes());
        change(&mut e, &src, MAIN);
        let m = main_agent(&e);
        assert_eq!(m.output_tokens, 10);
        assert_eq!(m.tool_calls, 2);
        assert_eq!(m.tokens, 113);
    }

    #[test]
    fn truncated_file_is_read_again() {
        let (src, mut e) = setup();
        src.append(MAIN, assistant("m1", &read_block("t1", "/x.md"), 1).as_bytes());
        src.append(MAIN, assistant("m2", &read_block("t2", "/y.md"), 1).as_bytes());
        change(&mut e, &src, MAIN);
        assert_eq!(main_agent(&e).tool_calls, 2);
        src.put(MAIN, assistant("m3", &read_block("t3", "/z.md"), 1).as_bytes());
        change(&mut e, &src, MAIN);
        let m = main_agent(&e);
        assert_eq!(m.tool_calls, 1);
        assert_eq!(m.last_tool.unwrap().summary, "/z.md");
        assert_eq!(e.stats().truncations, 1);
    }

    #[test]
    fn session_file_changes_and_removal() {
        let (src, mut e) = setup();
        assert_eq!(main_agent(&e).state, AgentState::Running);
        src.put("sessions/7.json", br#"{"pid":7,"sessionId":"s1","cwd":"/p","status":"idle","updatedAt":6}"#);
        change(&mut e, &src, "sessions/7.json");
        assert_eq!(main_agent(&e).state, AgentState::Waiting);
        src.files.lock().unwrap().remove("sessions/7.json");
        e.apply_changes(&src, [FileChange { path: "sessions/7.json".into(), op: ChangeOp::Delete, t: 3_000 }]);
        assert!(e.snapshot().sessions.is_empty());
        let last = e.take_events().pop().unwrap();
        assert_eq!(last.kind.type_name(), "sessionRemoved");
    }

    #[test]
    fn workflow_agents_without_declared_phases_append_phases_and_retry() {
        let (src, mut e) = setup();
        let dir = "projects/-p/s1/subagents/workflows/wf_1";
        src.put(&format!("{dir}/agent-a1.meta.json"), br#"{"agentType":"workflow-subagent","description":"x:1","workflowPhase":"Gather"}"#);
        src.put(&format!("{dir}/agent-a2.meta.json"), br#"{"agentType":"workflow-subagent","description":"y:1","workflowPhase":"Judge"}"#);
        src.put(&format!("{dir}/agent-a3.meta.json"), br#"{"agentType":"workflow-subagent","description":"x:1","workflowPhase":"Gather"}"#);
        for p in ["agent-a1.meta.json", "agent-a2.meta.json", "agent-a3.meta.json"] {
            change(&mut e, &src, &format!("{dir}/{p}"));
        }
        let run = &e.snapshot().sessions["s1"].runs["wf_1"];
        assert_eq!(run.phases.iter().map(|p| (p.index, p.title.as_str())).collect::<Vec<_>>(), vec![(1, "Gather"), (2, "Judge")]);
        assert_eq!(run.status, RunStatus::Running);
        src.put(
            &format!("{dir}/journal.jsonl"),
            b"{\"type\":\"launched\"}\n{\"type\":\"started\",\"key\":\"k1\",\"agentId\":\"a1\",\"label\":\"x:1\",\"phase\":\"Gather\"}\n{\"type\":\"started\",\"key\":\"k2\",\"agentId\":\"a2\"}\n{\"type\":\"result\",\"key\":\"k2\",\"agentId\":\"a2\"}\n{\"type\":\"started\",\"key\":\"k1\",\"agentId\":\"a3\"}\n",
        );
        change(&mut e, &src, &format!("{dir}/journal.jsonl"));
        let s = &e.snapshot().sessions["s1"];
        assert_eq!(s.agents["a1"].state, AgentState::Failed);
        assert_eq!(s.agents["a1"].state_reason.as_deref(), Some("retried"));
        assert!(s.agents["a1"].superseded);
        assert_eq!(s.agents["a2"].state, AgentState::Done);
        assert!(s.agents["a2"].ended_at.is_some());
        assert_eq!(s.agents["a3"].state, AgentState::Running);
        let run = &s.runs["wf_1"];
        assert_eq!(run.agent_ids, vec!["a1", "a2", "a3"]);
        assert_eq!(run.phases[0].state, PhaseState::Running);
        assert_eq!(run.phases[1].state, PhaseState::Done);
    }

    #[test]
    fn non_busy_session_status_means_waiting() {
        let (src, mut e) = setup();
        for (status, want) in [("waiting", AgentState::Waiting), ("idle", AgentState::Waiting), ("busy", AgentState::Running), ("shell", AgentState::Running), ("something-new", AgentState::Waiting)] {
            src.put("sessions/7.json", format!(r#"{{"pid":7,"sessionId":"s1","cwd":"/p","status":"{status}","updatedAt":6}}"#).as_bytes());
            change(&mut e, &src, "sessions/7.json");
            assert_eq!(main_agent(&e).state, want, "status {status}");
        }
    }

    #[test]
    fn quiet_subagent_stops_with_no_activity_unless_a_child_is_active() {
        use std::sync::atomic::Ordering::SeqCst;
        let (src, mut e) = setup();
        let sub = "projects/-p/s1/subagents";
        // 2026-01-01T00:00:01Z
        let t0: EpochMs = 1_767_225_601_000;
        src.put(&format!("{sub}/agent-a1.meta.json"), br#"{"agentType":"general-purpose","description":"parent"}"#);
        src.put(&format!("{sub}/agent-a1.jsonl"), assistant("m1", &read_block("t1", "/x"), 1).as_bytes());
        src.put(&format!("{sub}/agent-a2.meta.json"), br#"{"agentType":"general-purpose","description":"child","parentAgentId":"a1"}"#);
        src.now.store(t0 + 10 * 60_000, SeqCst);
        for p in ["agent-a1.meta.json", "agent-a1.jsonl", "agent-a2.meta.json"] {
            change(&mut e, &src, &format!("{sub}/{p}"));
        }
        let state = |e: &Engine, id: &str| {
            let a = &e.snapshot().sessions["s1"].agents[id];
            (a.state, a.state_reason.clone(), a.ended_at)
        };
        assert_eq!(state(&e, "a1").0, AgentState::Running);
        // An hour later with no records: stopped, ended at its last activity.
        src.now.store(t0 + 60 * 60_000, SeqCst);
        e.refresh_clock(&src);
        assert_eq!(state(&e, "a1"), (AgentState::Stopped, Some("noActivity".into()), Some(t0)));
        // A child that is still writing keeps its parent alive.
        let late = r#"{"type":"assistant","timestamp":"2026-01-01T00:59:00.000Z","message":{"id":"c1","stop_reason":"tool_use","content":[]}}"#;
        src.put(&format!("{sub}/agent-a2.jsonl"), format!("{late}\n").as_bytes());
        change(&mut e, &src, &format!("{sub}/agent-a2.jsonl"));
        assert_eq!(state(&e, "a1").0, AgentState::Running);
        assert_eq!(state(&e, "a1").2, None, "revived agents lose endedAt");
        assert_eq!(state(&e, "a2").0, AgentState::Running);
        // The main agent never goes quiet.
        src.now.store(t0 + 10 * 3_600_000, SeqCst);
        e.refresh_clock(&src);
        assert_eq!(state(&e, "a2").1.as_deref(), Some("noActivity"));
        assert_eq!(main_agent(&e).state, AgentState::Running);
    }

    #[test]
    fn agent_stopped_with_background_work_waits_and_runs_again_when_it_resumes() {
        use std::sync::atomic::Ordering::SeqCst;
        let (src, mut e) = setup();
        let sub = "projects/-p/s1/subagents";
        let t0: EpochMs = 1_767_225_601_000; // 2026-01-01T00:00:01Z
        let asst = |ts: &str, id: &str, stop: &str| {
            format!(r#"{{"type":"assistant","timestamp":"{ts}","message":{{"id":"{id}","stop_reason":"{stop}","content":[]}}}}"#) + "\n"
        };
        let notice = |ts: &str, extra: &str| {
            let text = format!("<task-notification>\\n<task-id>a1</task-id>\\n<status>completed</status>\\n{extra}</task-notification>");
            format!(r#"{{"type":"user","timestamp":"{ts}","message":{{"role":"user","content":"{text}"}}}}"#) + "\n"
        };
        let state = |e: &Engine| {
            let a = &e.snapshot().sessions["s1"].agents["a1"];
            (a.state, a.state_reason.clone())
        };
        // The meta file says finished, and the notice says completed, but with work still going.
        src.put(&format!("{sub}/agent-a1.meta.json"), br#"{"agentType":"general-purpose","description":"x","finished":true}"#);
        let mut sub_log = asst("2026-01-01T00:00:01.000Z", "m1", "end_turn");
        src.put(&format!("{sub}/agent-a1.jsonl"), sub_log.as_bytes());
        let mut main_log = notice("2026-01-01T00:00:02.000Z", "<summary>Agent \\\"x\\\" is waiting on its own background work</summary>\\n");
        src.put(MAIN, main_log.as_bytes());
        src.now.store(t0 + 10_000, SeqCst);
        for p in [format!("{sub}/agent-a1.meta.json"), format!("{sub}/agent-a1.jsonl"), MAIN.to_string()] {
            change(&mut e, &src, &p);
        }
        assert_eq!(state(&e), (AgentState::Waiting, Some("backgroundWork".into())));
        // It writes again: resumed, so running in spite of `finished: true`.
        sub_log += &asst("2026-01-01T00:05:00.000Z", "m2", "tool_use");
        src.put(&format!("{sub}/agent-a1.jsonl"), sub_log.as_bytes());
        src.now.store(t0 + 300_000, SeqCst);
        change(&mut e, &src, &format!("{sub}/agent-a1.jsonl"));
        assert_eq!(state(&e), (AgentState::Running, None));
        // A plain completed notice after its last message: done.
        sub_log += &asst("2026-01-01T00:05:29.000Z", "m3", "end_turn");
        src.put(&format!("{sub}/agent-a1.jsonl"), sub_log.as_bytes());
        main_log += &notice("2026-01-01T00:05:30.000Z", "");
        src.put(MAIN, main_log.as_bytes());
        src.now.store(t0 + 330_000, SeqCst);
        change(&mut e, &src, &format!("{sub}/agent-a1.jsonl"));
        change(&mut e, &src, MAIN);
        assert_eq!(state(&e), (AgentState::Done, None));
    }

    #[test]
    fn large_transcript_is_read_in_slices_and_yields_to_the_budget() {
        let (src, mut e) = setup();
        let line = assistant("m", &read_block("t", "/x"), 1);
        let n = (READ_CHUNK as usize * 3) / line.len() + 10;
        let mut big = Vec::with_capacity(n * (line.len() + 8));
        for i in 0..n {
            big.extend_from_slice(assistant(&format!("m{i}"), &read_block(&format!("t{i}"), "/x"), 1).as_bytes());
        }
        src.put(MAIN, &big);
        e.enqueue([FileChange { path: MAIN.into(), op: ChangeOp::Modify, t: 2_000 }]);
        let mut rounds = 0;
        while !e.run_queue(&src, Some(Duration::ZERO)) {
            rounds += 1;
            assert!(rounds < 100);
        }
        assert!(rounds >= 2, "a zero budget yields between slices, got {rounds} rounds");
        let m = main_agent(&e);
        assert_eq!(m.tool_calls as usize, n);
        assert_eq!(e.stats().bytes_read, big.len() as u64);
        let h = e.tool_history();
        assert_eq!(h[0].calls.len(), n.min(TOOL_HISTORY_MAX));
        assert_eq!(h[0].calls.last().unwrap().tool_use_id, format!("t{}", n - 1));
    }

    #[test]
    fn problems_are_merged_per_path_and_reason() {
        let (src, mut e) = setup();
        src.append(MAIN, b"not json\n{broken\n");
        change(&mut e, &src, MAIN);
        src.put("projects/-p/s1/subagents/agent-z.meta.json", b"{nope");
        change(&mut e, &src, "projects/-p/s1/subagents/agent-z.meta.json");
        let p = e.problems();
        assert_eq!(p.len(), 2);
        let bad = p.iter().find(|p| p.reason == ProblemReason::BadLine).unwrap();
        assert_eq!((bad.path.as_str(), bad.count), (MAIN, 2));
        assert!(p.iter().any(|p| p.reason == ProblemReason::BadFile && p.path.ends_with("agent-z.meta.json")));
    }

    #[test]
    fn tool_history_keeps_more_than_recent_and_tracks_results() {
        let (src, mut e) = setup();
        for i in 0..20 {
            src.append(MAIN, assistant(&format!("m{i}"), &read_block(&format!("t{i}"), "/x"), 1).as_bytes());
        }
        src.append(MAIN, br#"{"type":"user","timestamp":"2026-01-01T00:00:02.000Z","message":{"content":[{"type":"tool_result","tool_use_id":"t0","content":"x"}]}}"#);
        src.append(MAIN, b"\n");
        change(&mut e, &src, MAIN);
        assert_eq!(main_agent(&e).recent_tools.len(), RECENT_TOOLS_MAX);
        let h = e.tool_history();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].calls.len(), 20);
        assert_eq!(h[0].calls[0].is_error, Some(false));
        assert_eq!(h[0].calls[0].ended_at, Some(1767225602000));
    }

    #[test]
    fn reroots_script_paths() {
        assert_eq!(
            reroot("/Users/x/.claude/projects/-a/s/workflows/scripts/n-wf_1.js").as_deref(),
            Some("projects/-a/s/workflows/scripts/n-wf_1.js")
        );
        assert_eq!(reroot("relative.js"), None);
    }

    #[test]
    fn task_rule_for_workflow_frames() {
        let mut a = AgentInt::new("x", AgentKind::Workflow, Some("wf".into()));
        apply_task_rule(&mut a, "[Workflow harness — user request] stuff");
        assert!(a.task.is_none());
        apply_task_rule(&mut a, "[Workflow harness — computed task] header\n  Do the thing.\n\n  Rest");
        assert_eq!(a.task.as_deref(), Some("Do the thing."));
        let mut m = AgentInt::new("main", AgentKind::Main, None);
        apply_task_rule(&mut m, "<command-name>/clear</command-name>");
        apply_task_rule(&mut m, "Real prompt");
        assert_eq!(m.task.as_deref(), Some("Real prompt"));
    }
}
