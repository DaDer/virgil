//! The attention list: for each running session, whether it needs the person, has finished, has
//! gone quiet, or is working, with one line saying where it stands. Derived from the snapshot plus
//! [`SessionExtras`]; live mode only. Pure, so the rules can be tested without files.

use crate::contract::{AgentState, EpochMs, Liveness, Session, Snapshot, MAIN_AGENT_ID};
use crate::engine::SessionExtras;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A working session with no sign of life for this long is listed as quiet.
pub const QUIET_AFTER_MS: i64 = 15 * 60_000;
/// Finished sessions older than this drop to the idle group.
pub const FINISHED_FOR_MS: i64 = 24 * 3_600_000;
/// A background command older than this is assumed to have ended without a notice.
const BACKGROUND_MAX_MS: i64 = 12 * 3_600_000;
const LINE_MAX: usize = 140;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum AttentionKind {
    /// It asked something (a question or a permission) and waits for an answer.
    NeedsYou,
    /// Its turn ended and nothing of its own is still running.
    Finished,
    /// It should be working but nothing has been written for a while.
    Quiet,
    /// Working: the main thread, its agents, or its background commands.
    Working,
    /// Nothing to report: never used, or finished long ago.
    Idle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionItem {
    pub session_id: String,
    /// The session's name, else its title, else the folder name.
    pub name: String,
    /// Last folder of the working directory.
    pub project: Option<String>,
    pub title: Option<String>,
    pub kind: AttentionKind,
    /// When it entered this state.
    pub since: EpochMs,
    /// Where it stands, in one line.
    pub line: String,
    pub running_agents: u32,
    pub background_tasks: u32,
}

/// Whether the wake lock should be held: something is working.
pub fn anything_working(items: &[AttentionItem]) -> bool {
    items.iter().any(|i| matches!(i.kind, AttentionKind::Working | AttentionKind::Quiet))
}

pub fn derive(snapshot: &Snapshot, extras: &BTreeMap<String, SessionExtras>, now: EpochMs) -> Vec<AttentionItem> {
    let none = SessionExtras::default();
    let mut items: Vec<AttentionItem> = snapshot
        .sessions
        .values()
        .filter(|s| s.info.liveness == Liveness::Alive && s.info.pid.is_some())
        .map(|s| item(s, extras.get(&s.info.session_id).unwrap_or(&none), now))
        .collect();
    items.sort_by(|a, b| a.kind.cmp(&b.kind).then(b.since.cmp(&a.since)).then(a.session_id.cmp(&b.session_id)));
    items
}

fn item(s: &Session, x: &SessionExtras, now: EpochMs) -> AttentionItem {
    let info = &s.info;
    let project = info.cwd.as_deref().and_then(|c| c.rsplit(['/', '\\']).find(|p| !p.is_empty())).map(str::to_string);
    let name = info.name.clone().or_else(|| info.title.clone()).or_else(|| project.clone()).unwrap_or_else(|| info.session_id.chars().take(8).collect());
    let status_at = info.status_updated_at.or(info.updated_at).or(info.started_at).unwrap_or(now);

    let subs: Vec<_> = s.agents.values().filter(|a| a.agent_id != MAIN_AGENT_ID && !a.superseded).collect();
    let running: Vec<_> = subs.iter().filter(|a| matches!(a.state, AgentState::Running | AgentState::Queued)).collect();
    let paused = subs.iter().filter(|a| a.state == AgentState::Waiting && a.state_reason.as_deref() == Some("backgroundWork")).count();
    let background: Vec<_> = x.background.iter().filter(|b| now - b.started_at < BACKGROUND_MAX_MS).collect();
    let own_work = running.len() + paused + background.len();
    let last_activity = s.agents.values().filter_map(|a| a.last_activity_at).chain([status_at]).max().unwrap_or(status_at);
    let said = x.last_text.as_deref().map(first_line).filter(|l| !l.is_empty());
    let main = s.agents.get(MAIN_AGENT_ID);

    let (kind, since, line) = match info.status.as_deref() {
        Some("waiting") => (AttentionKind::NeedsYou, status_at, said.unwrap_or_else(|| "Waiting for your answer".into())),
        None | Some("busy") | Some("shell") => {
            let doing = if !running.is_empty() {
                let first = &running[0].label;
                match running.len() {
                    1 => format!("1 agent running: {first}"),
                    n => format!("{n} agents running: {first} and {} more", n - 1),
                }
            } else if let Some(t) = main.and_then(|a| a.recent_tools.iter().rev().find(|c| c.ended_at.is_none())) {
                if t.summary.is_empty() {
                    format!("Running {}", t.name)
                } else {
                    format!("{}: {}", t.name, t.summary)
                }
            } else if let Some(b) = background.first() {
                format!("Background command: {}", or_unnamed(&b.summary))
            } else {
                "Thinking".to_string()
            };
            if now - last_activity > QUIET_AFTER_MS {
                (AttentionKind::Quiet, last_activity, doing)
            } else {
                (AttentionKind::Working, status_at, doing)
            }
        }
        // `idle`, and any status this version does not know.
        Some(_) => {
            if own_work > 0 {
                let mut parts = Vec::new();
                if !running.is_empty() || paused > 0 {
                    parts.push(count(running.len() + paused, "agent"));
                }
                if !background.is_empty() {
                    parts.push(count(background.len(), "background command"));
                }
                let what = running.first().map(|a| a.label.clone()).or_else(|| background.first().map(|b| or_unnamed(&b.summary)));
                let line = format!("Waiting for {}{}", parts.join(" and "), what.map(|w| format!(": {w}")).unwrap_or_default());
                // Background commands write nothing while they run, so silence here is not a stall.
                let kind = AttentionKind::Working;
                (kind, status_at, line)
            } else if !info.has_transcript || main.is_none_or(|a| a.last_activity_at.is_none()) {
                (AttentionKind::Idle, status_at, "Not used yet".to_string())
            } else if now - status_at > FINISHED_FOR_MS {
                (AttentionKind::Idle, status_at, said.unwrap_or_else(|| "Finished".into()))
            } else {
                (AttentionKind::Finished, status_at, said.unwrap_or_else(|| "Finished".into()))
            }
        }
    };

    AttentionItem {
        session_id: info.session_id.clone(),
        name,
        project,
        title: info.title.clone(),
        kind,
        since,
        line: cut(&line, LINE_MAX),
        running_agents: (running.len() + paused) as u32,
        background_tasks: background.len() as u32,
    }
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

fn or_unnamed(s: &str) -> String {
    if s.trim().is_empty() {
        "a command".to_string()
    } else {
        s.to_string()
    }
}

/// First line with real words, without Markdown marks. A heading is skipped when text follows it.
fn first_line(text: &str) -> String {
    let clean = |l: &str| l.trim().trim_start_matches(['#', '-', '*', '>', ' ']).replace("**", "").replace('`', "");
    let lines: Vec<&str> = text.lines().filter(|l| l.chars().any(char::is_alphanumeric)).collect();
    let body = lines.iter().find(|l| !l.trim_start().starts_with('#'));
    body.or(lines.first()).map(|l| clean(l)).unwrap_or_default()
}

fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::BackgroundTask;
    use serde_json::json;

    const NOW: EpochMs = 1_800_000_000_000;

    fn session(status: &str, status_ago_min: i64, agents: serde_json::Value) -> Session {
        serde_json::from_value(json!({
            "sessionId": "s1", "pid": 7, "liveness": "alive", "cwd": "/home/dev/shop", "projectSlug": "-home-dev-shop",
            "name": "shop-a1", "title": "Checkout flow", "status": status, "kind": "interactive", "version": "2",
            "startedAt": NOW - 10 * 3_600_000, "updatedAt": NOW - status_ago_min * 60_000,
            "statusUpdatedAt": NOW - status_ago_min * 60_000, "hasTranscript": true,
            "agents": agents, "runs": {}, "files": {}
        }))
        .expect("session json")
    }

    fn agent(id: &str, state: &str, reason: Option<&str>, active_ago_min: i64) -> serde_json::Value {
        json!({
            "agentId": id, "sessionId": "s1", "kind": if id == "main" { "main" } else { "subagent" },
            "parentAgentId": if id == "main" { None } else { Some("main") }, "spawnToolUseId": null, "spawnDepth": 0,
            "agentType": null, "name": null, "label": format!("label {id}"), "task": "t", "model": null,
            "state": state, "stateReason": reason, "runId": null, "phaseIndex": null, "phaseTitle": null,
            "attempt": null, "superseded": false, "queuedAt": null, "startedAt": NOW - 3_600_000, "endedAt": null,
            "lastActivityAt": NOW - active_ago_min * 60_000, "tokens": 0, "outputTokens": 0, "toolCalls": 0,
            "lastTool": null, "recentTools": [], "resultPreview": null, "worktreePath": null
        })
    }

    fn one(s: Session, x: SessionExtras) -> AttentionItem {
        item(&s, &x, NOW)
    }

    fn said(text: &str) -> SessionExtras {
        SessionExtras { last_text: Some(text.into()), ..Default::default() }
    }

    #[test]
    fn idle_with_nothing_running_is_finished_and_quotes_the_last_message() {
        let s = session("idle", 12, json!({"main": agent("main", "waiting", None, 12)}));
        let i = one(s, said("## Done\n\nAll **checks** passed, 3 files changed.\nMore."));
        assert_eq!(i.kind, AttentionKind::Finished);
        assert_eq!(i.line, "All checks passed, 3 files changed.");
        assert_eq!(i.since, NOW - 12 * 60_000);
        assert_eq!((i.name.as_str(), i.project.as_deref()), ("shop-a1", Some("shop")));
    }

    #[test]
    fn idle_with_background_work_is_still_working() {
        let s = session("idle", 40, json!({"main": agent("main", "waiting", None, 40)}));
        let x = SessionExtras { background: vec![BackgroundTask { summary: "Run the tests".into(), started_at: NOW - 40 * 60_000, agent_id: "main".into() }], ..said("Started the tests.") };
        let i = one(s, x);
        assert_eq!(i.kind, AttentionKind::Working);
        assert_eq!(i.line, "Waiting for 1 background command: Run the tests");
        assert_eq!(i.background_tasks, 1);

        let s = session("idle", 5, json!({"main": agent("main", "waiting", None, 5), "a1": agent("a1", "waiting", Some("backgroundWork"), 1), "a2": agent("a2", "done", None, 30)}));
        let i = one(s, SessionExtras::default());
        assert_eq!((i.kind, i.running_agents), (AttentionKind::Working, 1));
    }

    #[test]
    fn waiting_needs_you_and_busy_goes_quiet_after_a_while() {
        let s = session("waiting", 4, json!({"main": agent("main", "waiting", None, 4)}));
        assert_eq!(one(s, said("Which database should I use?")).kind, AttentionKind::NeedsYou);

        let s = session("busy", 3, json!({"main": agent("main", "running", None, 1), "a1": agent("a1", "running", None, 1), "a2": agent("a2", "running", None, 2)}));
        let i = one(s, SessionExtras::default());
        assert_eq!(i.kind, AttentionKind::Working);
        assert_eq!(i.line, "2 agents running: label a1 and 1 more");

        let s = session("busy", 50, json!({"main": agent("main", "running", None, 25)}));
        let i = one(s, SessionExtras::default());
        assert_eq!((i.kind, i.since), (AttentionKind::Quiet, NOW - 25 * 60_000));
    }

    #[test]
    fn old_and_unused_sessions_are_idle_and_the_list_is_sorted_by_urgency() {
        let old = session("idle", 30 * 60, json!({"main": agent("main", "waiting", None, 30 * 60)}));
        assert_eq!(one(old, said("x")).kind, AttentionKind::Idle);

        let mut snap = Snapshot { seq: 1, t: NOW, mode: crate::contract::SourceMode::Live, sessions: BTreeMap::new() };
        for (id, status) in [("a", "busy"), ("b", "idle"), ("c", "waiting")] {
            let mut s = session(status, 2, json!({"main": agent("main", "waiting", None, 2)}));
            s.info.session_id = id.into();
            snap.sessions.insert(id.into(), s);
        }
        let mut gone = session("busy", 2, json!({}));
        gone.info.session_id = "d".into();
        gone.info.liveness = Liveness::Stale;
        snap.sessions.insert("d".into(), gone);
        let kinds: Vec<_> = derive(&snap, &BTreeMap::new(), NOW).into_iter().map(|i| (i.session_id, i.kind)).collect();
        assert_eq!(kinds, vec![("c".into(), AttentionKind::NeedsYou), ("b".into(), AttentionKind::Finished), ("a".into(), AttentionKind::Working)]);
        assert!(anything_working(&derive(&snap, &BTreeMap::new(), NOW)));
    }
}
