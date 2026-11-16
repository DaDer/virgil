//! Writes `fixtures/contract-sample.json`: what the Rust core really serialises across the UI
//! boundary (data-contract sections 3 and 7). The UI's contract test (`src/lib/data/contract.test.ts`)
//! feeds it through the TypeScript reducer, so a field renamed or retyped on either side fails a test.
//!
//! cargo run -p virgil-core --example contract_sample [-- <out path>]
//! (`npm run contract:sample` does this.) `tests/contract_sample.rs` fails when the file is stale.
//!
//! The stream is the synthetic `fixtures/recording-small` played from an empty snapshot in fixed
//! 250 ms steps, so the output is deterministic. Three `backgroundTaskUpserted` events are appended
//! because the recording has no background command, then a `sessionUpserted` + `sessionRemoved`
//! pair for a throwaway session because it never removes a session.

use virgil_core::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const STEP_MS: i64 = 250;
const REL_RECORDING: &str = "fixtures/recording-small";

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("repo root")
}

pub fn sample_path() -> PathBuf {
    repo_root().join("fixtures/contract-sample.json")
}

/// Builds the sample, pretty-printed with a trailing newline.
pub fn build() -> String {
    let dir = repo_root().join(REL_RECORDING);
    let mut src = ReplaySource::open(&dir).expect("open fixtures/recording-small");
    let (start, end) = (src.start(), src.end());

    let mut engine = Engine::new(EngineConfig::replay());
    let mut events = vec![engine.reset_event(1)];
    let mut t = start;
    loop {
        src.set_position(t.min(end));
        loop {
            let changes = src.poll();
            if changes.is_empty() {
                break;
            }
            engine.apply_changes(&src, changes);
        }
        events.extend(engine.take_events());
        if t >= end {
            break;
        }
        t += STEP_MS;
    }

    // Background commands, by the main thread and by an agent, and a session that comes and goes,
    // so every event type is present.
    let mut snap = engine.snapshot().clone();
    let template = snap.sessions.values().next().expect("recording has a session");
    let sid = template.info.session_id.clone();
    let agent = template.agents.keys().find(|a| *a != MAIN_AGENT_ID).expect("recording has an agent").clone();
    let task = |task_id: &str, agent_id: &str, status: BackgroundTaskStatus, ended_at: Option<EpochMs>, output: bool| BackgroundTask {
        task_id: task_id.into(),
        session_id: sid.clone(),
        agent_id: agent_id.into(),
        tool_use_id: Some(format!("toolu_sample_{task_id}")),
        summary: "Run the test suite".into(),
        status,
        started_at: end - 30_000,
        ended_at,
        output_path: output.then(|| format!("/tmp/claude-1000/-home-dev-projects-orchard/{sid}/tasks/{task_id}.output")),
    };
    let mut gone = template.info.clone();
    gone.session_id = "00000000-0000-4000-8000-00000000dead".into();
    gone.pid = Some(4243);
    gone.name = Some("throwaway".into());
    let next = snap.seq + 1;
    for (i, kind) in [
        EventKind::BackgroundTaskUpserted { session_id: sid.clone(), task: Box::new(task("b1", MAIN_AGENT_ID, BackgroundTaskStatus::Running, None, false)) },
        EventKind::BackgroundTaskUpserted { session_id: sid.clone(), task: Box::new(task("b2", &agent, BackgroundTaskStatus::Failed, Some(end - 10_000), true)) },
        EventKind::BackgroundTaskUpserted { session_id: sid.clone(), task: Box::new(task("b1", MAIN_AGENT_ID, BackgroundTaskStatus::Completed, Some(end), true)) },
        EventKind::SessionUpserted { session: gone.clone() },
        EventKind::SessionRemoved { session_id: gone.session_id.clone() },
    ]
    .into_iter()
    .enumerate()
    {
        let e = Event { seq: next + i as u64, t: end, kind };
        snap.apply(&e);
        events.push(e);
    }

    let mut info = src.info();
    info.path = REL_RECORDING.into();
    let replay_status = SourceStatus {
        mode: SourceMode::Replay,
        claude_dir: None,
        recording: Some(info.clone()),
        replay: Some(ReplayState { playing: false, speed: 1.0, position: end, start, end, skip_gaps: true }),
        error: None,
        problems: Vec::new(),
    };
    let live_status = SourceStatus {
        mode: SourceMode::Live,
        claude_dir: Some("/home/dev/.claude".into()),
        recording: None,
        replay: None,
        error: Some("Claude dir not found: /home/dev/.claude".into()),
        problems: vec![Problem {
            t: end,
            path: "projects/-home-dev-app/s1.jsonl".into(),
            reason: ProblemReason::BadLine,
            count: 2,
        }],
    };
    let tool_history =
        ToolHistory { seq: engine.snapshot().seq, agents: engine.tool_history(), timeline_times: src.event_times().collect() };
    let error = CommandError::new(CommandErrorCode::NotReplaying, "not in replay mode");

    let v: Value = json!({
        "about": "Generated by `npm run contract:sample` (crates/core/examples/contract_sample.rs). Do not edit.",
        "events": events,
        "batch": EventBatch { events: events[1..4].to_vec() },
        "snapshot": snap,
        "statusReplay": replay_status,
        "statusLive": live_status,
        "recordings": [info],
        "toolHistory": tool_history,
        "commandError": error,
    });
    let mut s = serde_json::to_string_pretty(&v).expect("serialise");
    s.push('\n');
    s
}

#[allow(dead_code)]
fn main() {
    let out = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(sample_path);
    let s = build();
    std::fs::write(&out, &s).unwrap_or_else(|e| panic!("write {}: {e}", out.display()));
    println!("wrote {} ({} bytes)", out.display(), s.len());
}
