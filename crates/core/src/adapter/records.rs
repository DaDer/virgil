//! Parsers for every source file format (contract section 1). Each takes bytes or a line and returns
//! a typed value with every field optional; nothing here panics on bad input.

use super::text::xml_tag;
use super::time::parse_iso;
use crate::contract::EpochMs;
use serde_json::Value;

fn s(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}
fn n(v: &Value, key: &str) -> Option<i64> {
    let x = v.get(key)?;
    x.as_i64().or_else(|| x.as_f64().map(|f| f as i64))
}
fn u(v: &Value, key: &str) -> Option<u64> {
    n(v, key).filter(|x| *x >= 0).map(|x| x as u64)
}
fn b(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}
fn ts(v: &Value) -> Option<EpochMs> {
    match v.get("timestamp")? {
        Value::String(t) => parse_iso(t),
        Value::Number(x) => x.as_i64(),
        _ => None,
    }
}

// ---------- sessions/<pid>.json ----------

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SessionFile {
    pub pid: Option<u32>,
    pub session_id: String,
    pub cwd: Option<String>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub kind: Option<String>,
    pub version: Option<String>,
    pub started_at: Option<EpochMs>,
    pub updated_at: Option<EpochMs>,
    pub status_updated_at: Option<EpochMs>,
}

pub fn parse_session_file(bytes: &[u8]) -> Option<SessionFile> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    let session_id = s(&v, "sessionId").filter(|x| !x.is_empty())?;
    Some(SessionFile {
        pid: n(&v, "pid").and_then(|p| u32::try_from(p).ok()),
        session_id,
        cwd: s(&v, "cwd"),
        name: s(&v, "name"),
        status: s(&v, "status"),
        kind: s(&v, "kind"),
        version: s(&v, "version"),
        started_at: n(&v, "startedAt"),
        updated_at: n(&v, "updatedAt"),
        status_updated_at: n(&v, "statusUpdatedAt"),
    })
}

// ---------- agent-<id>.meta.json ----------

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AgentMeta {
    pub agent_type: Option<String>,
    pub description: Option<String>,
    pub tool_use_id: Option<String>,
    pub parent_agent_id: Option<String>,
    pub spawn_depth: Option<u32>,
    pub workflow_phase: Option<String>,
    pub model: Option<String>,
    pub finished: bool,
    pub stopped_by_user: bool,
    pub name: Option<String>,
    pub worktree_path: Option<String>,
}

pub fn parse_meta(bytes: &[u8]) -> Option<AgentMeta> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    if !v.is_object() {
        return None;
    }
    Some(AgentMeta {
        agent_type: s(&v, "agentType"),
        description: s(&v, "description"),
        tool_use_id: s(&v, "toolUseId"),
        parent_agent_id: s(&v, "parentAgentId"),
        spawn_depth: n(&v, "spawnDepth").and_then(|x| u32::try_from(x).ok()),
        workflow_phase: s(&v, "workflowPhase"),
        model: s(&v, "model"),
        finished: b(&v, "finished"),
        stopped_by_user: b(&v, "stoppedByUser"),
        name: s(&v, "name"),
        worktree_path: s(&v, "worktreePath"),
    })
}

// ---------- journal.jsonl ----------

#[derive(Debug, Clone, PartialEq)]
pub enum JournalLine {
    Started { key: String, agent_id: String, label: Option<String>, phase: Option<String> },
    Result { key: String, agent_id: String },
    Failed { key: String, agent_id: String },
    Other,
}

pub fn parse_journal_line(line: &[u8]) -> Option<JournalLine> {
    let v: Value = serde_json::from_slice(line).ok()?;
    let key = s(&v, "key").unwrap_or_default();
    let agent_id = s(&v, "agentId").unwrap_or_default();
    Some(match v.get("type").and_then(Value::as_str) {
        Some("started") => JournalLine::Started { key, agent_id, label: s(&v, "label"), phase: s(&v, "phase") },
        Some("result") => JournalLine::Result { key, agent_id },
        Some("failed") => JournalLine::Failed { key, agent_id },
        Some(_) => JournalLine::Other,
        None => return None,
    })
}

// ---------- workflows/<runId>.json ----------

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RunFilePhase {
    pub index: u32,
    pub title: String,
    pub detail: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RunFileAgent {
    pub index: u32,
    pub label: Option<String>,
    pub phase_index: Option<u32>,
    pub phase_title: Option<String>,
    pub agent_id: Option<String>,
    pub model: Option<String>,
    pub state: Option<String>,
    pub queued_at: Option<EpochMs>,
    pub started_at: Option<EpochMs>,
    pub attempt: Option<u32>,
    pub tokens: Option<u64>,
    pub tool_calls: Option<u32>,
    pub duration_ms: Option<i64>,
    pub last_tool_name: Option<String>,
    pub last_tool_summary: Option<String>,
    pub prompt_preview: Option<String>,
    pub result_preview: Option<String>,
    pub last_progress_at: Option<EpochMs>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RunFile {
    pub run_id: Option<String>,
    pub task_id: Option<String>,
    pub workflow_name: Option<String>,
    pub summary: Option<String>,
    pub status: Option<String>,
    pub start_time: Option<EpochMs>,
    pub duration_ms: Option<i64>,
    pub phases: Vec<RunFilePhase>,
    pub agents: Vec<RunFileAgent>,
    pub agent_count: Option<u32>,
    pub total_tokens: Option<u64>,
    pub total_tool_calls: Option<u64>,
    pub default_model: Option<String>,
}

pub fn parse_run_file(bytes: &[u8]) -> Option<RunFile> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    if !v.is_object() {
        return None;
    }
    let declared: Vec<&Value> = v.get("phases").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    let progress: Vec<&Value> =
        v.get("workflowProgress").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    let mut phases: Vec<RunFilePhase> = Vec::new();
    for p in progress.iter().filter(|p| p.get("type").and_then(Value::as_str) == Some("workflow_phase")) {
        let Some(index) = n(p, "index").and_then(|x| u32::try_from(x).ok()) else { continue };
        let title = s(p, "title").unwrap_or_default();
        let decl = declared
            .iter()
            .find(|d| s(d, "title").as_deref() == Some(title.as_str()))
            .or_else(|| declared.get((index as usize).wrapping_sub(1)));
        phases.push(RunFilePhase {
            index,
            title,
            detail: decl.and_then(|d| s(d, "detail")),
            model: decl.and_then(|d| s(d, "model")),
        });
    }
    if phases.is_empty() {
        for (i, d) in declared.iter().enumerate() {
            phases.push(RunFilePhase {
                index: i as u32 + 1,
                title: s(d, "title").unwrap_or_default(),
                detail: s(d, "detail"),
                model: s(d, "model"),
            });
        }
    }
    let agents = progress
        .iter()
        .filter(|p| p.get("type").and_then(Value::as_str) == Some("workflow_agent"))
        .enumerate()
        .map(|(i, p)| RunFileAgent {
            index: n(p, "index").and_then(|x| u32::try_from(x).ok()).unwrap_or(i as u32),
            label: s(p, "label"),
            phase_index: n(p, "phaseIndex").and_then(|x| u32::try_from(x).ok()),
            phase_title: s(p, "phaseTitle"),
            agent_id: s(p, "agentId").filter(|x| !x.is_empty()),
            model: s(p, "model"),
            state: s(p, "state"),
            queued_at: n(p, "queuedAt"),
            started_at: n(p, "startedAt"),
            attempt: n(p, "attempt").and_then(|x| u32::try_from(x).ok()),
            tokens: u(p, "tokens"),
            tool_calls: n(p, "toolCalls").and_then(|x| u32::try_from(x).ok()),
            duration_ms: n(p, "durationMs"),
            last_tool_name: s(p, "lastToolName"),
            last_tool_summary: s(p, "lastToolSummary"),
            prompt_preview: s(p, "promptPreview"),
            result_preview: s(p, "resultPreview"),
            last_progress_at: n(p, "lastProgressAt"),
            error: match p.get("error") {
                Some(Value::String(e)) => Some(e.clone()),
                Some(Value::Null) | None => None,
                Some(other) => Some(other.to_string()),
            },
        })
        .collect();
    Some(RunFile {
        run_id: s(&v, "runId"),
        task_id: s(&v, "taskId"),
        workflow_name: s(&v, "workflowName"),
        summary: s(&v, "summary"),
        status: s(&v, "status"),
        start_time: n(&v, "startTime"),
        duration_ms: n(&v, "durationMs"),
        phases,
        agents,
        agent_count: n(&v, "agentCount").and_then(|x| u32::try_from(x).ok()),
        total_tokens: u(&v, "totalTokens"),
        total_tool_calls: u(&v, "totalToolCalls"),
        default_model: s(&v, "defaultModel"),
    })
}

// ---------- transcripts ----------

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Usage {
    pub input: u64,
    pub cache_creation: u64,
    pub cache_read: u64,
    pub output: u64,
}

impl Usage {
    pub fn context(&self) -> u64 {
        self.input + self.cache_creation + self.cache_read + self.output
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct WorkflowLaunch {
    pub run_id: String,
    pub task_id: Option<String>,
    pub workflow_name: Option<String>,
    pub summary: Option<String>,
    pub script_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub task_id: String,
    pub tool_use_id: Option<String>,
    pub status: Option<String>,
    /// The agent stopped but has background work of its own and may resume: Claude Code still
    /// reports `completed`, and says so only in the summary and the note.
    pub still_working: bool,
}

/// How much of an assistant message's text is kept.
pub const TEXT_MAX: usize = 600;

#[derive(Debug, Clone, PartialEq)]
pub enum Record {
    User {
        ts: Option<EpochMs>,
        cwd: Option<String>,
        /// String content (a prompt, a framed prompt, a notice, a reminder).
        text: Option<String>,
        results: Vec<ToolResult>,
        workflow_launch: Option<WorkflowLaunch>,
        /// Set on the result of a Bash call that went to the background (`backgroundTaskId`).
        background_task_id: Option<String>,
    },
    Assistant {
        ts: Option<EpochMs>,
        cwd: Option<String>,
        message_id: Option<String>,
        model: Option<String>,
        stop_reason: Option<String>,
        usage: Option<Usage>,
        is_api_error: bool,
        tool_uses: Vec<ToolUse>,
        /// The message's text blocks joined, cut to `TEXT_MAX` characters.
        text: Option<String>,
    },
    AiTitle(String),
    Notice { ts: Option<EpochMs>, notice: Notice },
}

fn quick_type_hint(line: &[u8]) -> bool {
    // Cheap pre-filter so that megabyte-long records of other types are not parsed.
    const NEEDLES: [&[u8]; 4] = [b"\"user\"", b"\"assistant\"", b"\"ai-title\"", b"\"queue-operation\""];
    NEEDLES.iter().any(|n| line.windows(n.len()).any(|w| w == *n))
}

/// Parses one transcript line. `Ok(None)` for a valid record the app does not read; `Err(())` for a
/// line that is not JSON.
#[allow(clippy::result_unit_err)]
pub fn parse_record(line: &[u8]) -> Result<Option<Record>, ()> {
    if !quick_type_hint(line) {
        // Still verify it is JSON so bad lines are counted, but without building a value.
        return match serde_json::from_slice::<serde::de::IgnoredAny>(line) {
            Ok(_) => Ok(None),
            Err(_) => Err(()),
        };
    }
    let v: Value = serde_json::from_slice(line).map_err(|_| ())?;
    let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
    Ok(match ty {
        "user" => Some(parse_user(&v)),
        "assistant" => Some(parse_assistant(&v)),
        "ai-title" => s(&v, "aiTitle").map(Record::AiTitle),
        "queue-operation" => {
            if v.get("operation").and_then(Value::as_str) != Some("enqueue") {
                return Ok(None);
            }
            let content = match v.get("content") {
                Some(Value::String(c)) => c.as_str(),
                _ => return Ok(None),
            };
            parse_notice(content).map(|notice| Record::Notice { ts: ts(&v), notice })
        }
        _ => None,
    })
}

pub fn parse_notice(content: &str) -> Option<Notice> {
    // Subagents get it behind a "[SYSTEM NOTIFICATION ...]" preamble, so look a little way in.
    let head_end = content.char_indices().nth(600).map_or(content.len(), |(i, _)| i);
    let start = content[..head_end].find("<task-notification>")?;
    let content = &content[start..];
    let task_id = xml_tag(content, "task-id")?.to_string();
    Some(Notice {
        task_id,
        tool_use_id: xml_tag(content, "tool-use-id").map(str::to_string),
        status: xml_tag(content, "status").map(str::to_string),
        still_working: content.contains("waiting on its own background work")
            || content.contains("background work of its own still running"),
    })
}

fn parse_user(v: &Value) -> Record {
    let msg = v.get("message");
    let content = msg.and_then(|m| m.get("content"));
    let mut text = None;
    let mut results = Vec::new();
    match content {
        Some(Value::String(t)) => text = Some(t.clone()),
        Some(Value::Array(blocks)) => {
            for blk in blocks {
                if blk.get("type").and_then(Value::as_str) == Some("tool_result") {
                    if let Some(id) = s(blk, "tool_use_id") {
                        results.push(ToolResult { tool_use_id: id, is_error: b(blk, "is_error") });
                    }
                }
            }
        }
        _ => {}
    }
    let workflow_launch = v.get("toolUseResult").filter(|r| r.is_object()).and_then(|r| {
        if r.get("taskType").and_then(Value::as_str) != Some("local_workflow") {
            return None;
        }
        Some(WorkflowLaunch {
            run_id: s(r, "runId").filter(|x| !x.is_empty())?,
            task_id: s(r, "taskId"),
            workflow_name: s(r, "workflowName"),
            summary: s(r, "summary"),
            script_path: s(r, "scriptPath"),
        })
    });
    let background_task_id =
        v.get("toolUseResult").filter(|r| r.is_object()).and_then(|r| s(r, "backgroundTaskId")).filter(|x| !x.is_empty());
    Record::User { ts: ts(v), cwd: s(v, "cwd"), text, results, workflow_launch, background_task_id }
}

fn parse_assistant(v: &Value) -> Record {
    let msg = v.get("message").cloned().unwrap_or(Value::Null);
    let usage = msg.get("usage").filter(|x| x.is_object()).map(|x| Usage {
        input: u(x, "input_tokens").unwrap_or(0),
        cache_creation: u(x, "cache_creation_input_tokens").unwrap_or(0),
        cache_read: u(x, "cache_read_input_tokens").unwrap_or(0),
        output: u(x, "output_tokens").unwrap_or(0),
    });
    let mut tool_uses = Vec::new();
    let mut text = String::new();
    if let Some(Value::Array(blocks)) = msg.get("content") {
        for blk in blocks {
            if blk.get("type").and_then(Value::as_str) == Some("text") {
                if let Some(t) = blk.get("text").and_then(Value::as_str) {
                    if text.chars().count() < TEXT_MAX {
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.extend(t.chars().take(TEXT_MAX));
                    }
                }
            }
            if blk.get("type").and_then(Value::as_str) == Some("tool_use") {
                if let Some(id) = s(blk, "id") {
                    tool_uses.push(ToolUse {
                        id,
                        name: s(blk, "name").unwrap_or_default(),
                        input: blk.get("input").cloned().unwrap_or(Value::Null),
                    });
                }
            }
        }
    }
    Record::Assistant {
        ts: ts(v),
        cwd: s(v, "cwd"),
        message_id: s(&msg, "id"),
        model: s(&msg, "model"),
        stop_reason: s(&msg, "stop_reason"),
        usage,
        is_api_error: b(v, "isApiErrorMessage"),
        tool_uses,
        text: Some(text).filter(|t| !t.trim().is_empty()),
    }
}

/// Splits `buf` into complete lines. Returns the lines (without `\n`, `\r` trimmed) and the number of
/// bytes consumed (up to and including the last `\n`).
pub fn complete_lines(buf: &[u8]) -> (Vec<&[u8]>, usize) {
    let Some(last_nl) = buf.iter().rposition(|&c| c == b'\n') else { return (Vec::new(), 0) };
    let lines = buf[..last_nl]
        .split(|&c| c == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
        .filter(|l| !l.iter().all(u8::is_ascii_whitespace))
        .collect();
    (lines, last_nl + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_complete_lines_only() {
        let (lines, used) = complete_lines(b"{\"a\":1}\n{\"b\":2}\n{\"c\"");
        assert_eq!(lines, vec![&b"{\"a\":1}"[..], &b"{\"b\":2}"[..]]);
        assert_eq!(used, 16);
        assert_eq!(complete_lines(b"partial").1, 0);
        assert_eq!(complete_lines(b"\n\n").0.len(), 0);
    }

    #[test]
    fn tolerant_records() {
        assert_eq!(parse_record(b"not json"), Err(()));
        assert_eq!(parse_record(b"{\"type\":\"mode\"}"), Ok(None));
        assert_eq!(parse_record(b"{\"type\":\"future\",\"x\":{\"type\":\"user\"}}"), Ok(None));
        let r = parse_record(br#"{"type":"assistant","timestamp":"2026-11-14T09:53:20.000Z","message":{"id":"m1","model":"claude-x","stop_reason":"tool_use","usage":{"input_tokens":1,"output_tokens":2},"content":[{"type":"tool_use","id":"t1","name":"Read","input":{"file_path":"/a"}}]}}"#)
            .unwrap()
            .unwrap();
        match r {
            Record::Assistant { ts, message_id, usage, tool_uses, .. } => {
                assert_eq!(ts, Some(1794650000000));
                assert_eq!(message_id.as_deref(), Some("m1"));
                assert_eq!(usage.unwrap().context(), 3);
                assert_eq!(tool_uses[0].name, "Read");
            }
            _ => panic!(),
        }
        // Wrong types for known fields do not fail the record.
        let r = parse_record(br#"{"type":"user","timestamp":5,"message":{"content":42}}"#).unwrap().unwrap();
        assert!(matches!(r, Record::User { ts: Some(5), text: None, .. }));
    }

    #[test]
    fn parses_meta_and_session_tolerantly() {
        assert_eq!(parse_meta(b"[]"), None);
        let m = parse_meta(br#"{"description":"x","spawnDepth":"two","finished":true,"extra":{}}"#).unwrap();
        assert_eq!(m.description.as_deref(), Some("x"));
        assert_eq!(m.spawn_depth, None);
        assert!(m.finished);
        assert_eq!(parse_session_file(br#"{"pid":1}"#), None);
        assert_eq!(parse_session_file(br#"{"sessionId":"s","pid":-4}"#).unwrap().pid, None);
    }

    #[test]
    fn parses_journal() {
        assert_eq!(
            parse_journal_line(br#"{"type":"failed","key":"k","agentId":""}"#),
            Some(JournalLine::Failed { key: "k".into(), agent_id: "".into() })
        );
        assert_eq!(parse_journal_line(br#"{"type":"launched"}"#), Some(JournalLine::Other));
        assert_eq!(parse_journal_line(b"{"), None);
    }
}
