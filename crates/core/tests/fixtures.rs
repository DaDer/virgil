//! Asserts what fixtures/README.md and fixtures/expected.json say a correct reader concludes.

use virgil_core::source::pid::FixedPidProbe;
use virgil_core::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn expected() -> Value {
    serde_json::from_slice(&std::fs::read(fixtures().join("expected.json")).unwrap()).unwrap()
}

/// Builds a live snapshot of a fixture Claude dir with pid 4242 alive. Also checks the reducer rule:
/// replaying the emitted events onto an empty snapshot gives the engine's snapshot.
fn live_snapshot(name: &str) -> Snapshot {
    let src = LiveSource::new(LiveConfig {
        claude_dir: Some(fixtures().join(name)),
        recent_window_ms: None,
        quiet_after_ms: None,
        watch: false,
        probe: Box::new(FixedPidProbe { alive: vec![4242] }),
    });
    let mut engine = Engine::new(EngineConfig { mode: SourceMode::Live, recent_window_ms: None, quiet_after_ms: None });
    engine.initial_scan(&src);
    assert!(engine.run_queue(&src, None));
    let events = engine.take_events();
    let mut replayed = Snapshot::empty(SourceMode::Live);
    for (i, e) in events.iter().enumerate() {
        assert_eq!(e.seq, i as u64 + 1, "seq increases by one");
        replayed.apply(e);
    }
    assert_eq!(&replayed, engine.snapshot(), "reducer rule");
    // The JSON form round-trips.
    let json = serde_json::to_string(engine.snapshot()).unwrap();
    let back: Snapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(&back, engine.snapshot());
    assert_eq!(engine.stats().bad_lines, 0);
    engine.snapshot().clone()
}

#[derive(Clone, Copy, PartialEq)]
enum Flavour {
    Live,
    /// Replay: liveness unknown, session 2 only has its session file.
    Replay,
}

fn opt_str(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

fn check_session(sid: &str, exp: &Value, s: &Session, flavour: Flavour) {
    let ctx = |what: &str| format!("session {sid}: {what}");
    let session2_replay = flavour == Flavour::Replay && sid.starts_with("2222");
    assert_eq!(s.info.cwd.as_deref(), exp["cwd"].as_str(), "{}", ctx("cwd"));
    assert_eq!(s.info.name.as_deref(), exp["name"].as_str(), "{}", ctx("name"));
    assert_eq!(s.info.pid, exp["pid"].as_u64().map(|p| p as u32), "{}", ctx("pid"));
    assert_eq!(s.info.status.as_deref(), exp["status"].as_str(), "{}", ctx("status"));
    match flavour {
        Flavour::Replay => assert_eq!(s.info.liveness, Liveness::Unknown),
        Flavour::Live => {
            let want = if exp["liveness"].as_str().unwrap().starts_with("alive") { Liveness::Alive } else { Liveness::Stale };
            assert_eq!(s.info.liveness, want, "{}", ctx("liveness"));
        }
    }
    if session2_replay {
        assert!(!s.info.has_transcript);
        assert_eq!(s.info.title, None);
        assert_eq!(s.agents.len(), 1);
        let main = &s.agents[MAIN_AGENT_ID];
        assert_eq!(main.tool_calls, 0);
        assert_eq!(main.state, AgentState::Running);
        assert!(s.runs.is_empty());
        return;
    }
    assert!(s.info.has_transcript);
    assert_eq!(s.info.title.as_deref(), exp["title"].as_str(), "{}", ctx("title"));
    assert_eq!(s.agents.len() as u64, exp["agentCount"].as_u64().unwrap(), "{}", ctx("agent count"));

    for (aid, ea) in exp["agents"].as_object().unwrap() {
        let a = s.agents.get(aid).unwrap_or_else(|| panic!("{}", ctx(&format!("missing agent {aid}"))));
        let c = |f: &str| ctx(&format!("agent {aid} {f}"));
        let kind = serde_json::to_value(a.kind).unwrap();
        assert_eq!(kind, ea["kind"], "{}", c("kind"));
        if let Some(l) = ea.get("label") {
            assert_eq!(a.label, l.as_str().unwrap(), "{}", c("label"));
        }
        assert_eq!(a.last_tool.as_ref().map(|t| t.name.clone()), opt_str(&ea["lastToolName"]), "{}", c("lastTool"));
        if let Some(m) = ea.get("model") {
            assert_eq!(a.model.as_deref(), m.as_str(), "{}", c("model"));
        }
        let open = a.recent_tools.iter().filter(|t| t.ended_at.is_none()).count() as u64;
        assert_eq!(open, ea["openToolCalls"].as_u64().unwrap(), "{}", c("open tool calls"));
        assert_eq!(a.parent_agent_id, opt_str(&ea["parentAgentId"]), "{}", c("parent"));
        assert_eq!(a.run_id, opt_str(&ea["runId"]), "{}", c("runId"));
        assert_eq!(a.spawn_depth as u64, ea["spawnDepth"].as_u64().unwrap(), "{}", c("spawnDepth"));
        assert_eq!(serde_json::to_value(a.state).unwrap(), ea["state"], "{}", c("state"));
        if let Some(r) = ea.get("stateReason") {
            assert_eq!(a.state_reason.as_deref(), r.as_str(), "{}", c("stateReason"));
        }
        if let Some(t) = ea.get("task") {
            assert_eq!(a.task, t.as_str().unwrap(), "{}", c("task"));
        }
        assert_eq!(a.tool_calls as u64, ea["toolCalls"].as_u64().unwrap(), "{}", c("toolCalls"));
        let tokens = ea.get("tokens").unwrap_or(&ea["transcriptTokens"]).as_u64().unwrap();
        assert_eq!(a.tokens, tokens, "{}", c("tokens"));
        if let Some(p) = ea.get("phaseIndex") {
            assert_eq!(a.phase_index.map(|x| x as u64), p.as_u64(), "{}", c("phaseIndex"));
        }
        let mut files: BTreeMap<String, (u32, u32)> = BTreeMap::new();
        for (path, f) in &s.files {
            if let Some(t) = f.touches.get(aid) {
                files.insert(path.clone(), (t.reads, t.writes));
            }
        }
        let want: BTreeMap<String, (u32, u32)> = ea["files"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(p, v)| (p.clone(), (v["reads"].as_u64().unwrap() as u32, v["writes"].as_u64().unwrap() as u32)))
            .collect();
        assert_eq!(files, want, "{}", c("files"));
    }

    let shared: BTreeMap<String, BTreeSet<String>> = s
        .files
        .iter()
        .filter(|(_, f)| f.touches.len() > 1)
        .map(|(p, f)| (p.clone(), f.touches.keys().cloned().collect()))
        .collect();
    let want_shared: BTreeMap<String, BTreeSet<String>> = exp
        .get("filesTouchedByMoreThanOneAgent")
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .map(|(p, v)| (p.clone(), v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(shared, want_shared, "{}", ctx("shared files"));

    let runs = exp["runs"].as_object().unwrap();
    assert_eq!(s.runs.len(), runs.len(), "{}", ctx("run count"));
    for (rid, er) in runs {
        let r = &s.runs[rid];
        let c = |f: &str| ctx(&format!("run {rid} {f}"));
        let ids: Vec<String> = er["agentIds"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
        assert_eq!(r.agent_ids, ids, "{}", c("agentIds"));
        assert_eq!(r.ended_at, er["endedAt"].as_i64(), "{}", c("endedAt"));
        assert_eq!(r.is_final, er["final"].as_bool().unwrap(), "{}", c("final"));
        assert_eq!(r.name, er["name"].as_str().unwrap(), "{}", c("name"));
        assert_eq!(r.parent_agent_id, er["parentAgentId"].as_str().unwrap(), "{}", c("parent"));
        assert_eq!(r.started_at, er["startedAt"].as_i64(), "{}", c("startedAt"));
        assert_eq!(serde_json::to_value(r.status).unwrap(), er["status"], "{}", c("status"));
        let phases: Vec<(u64, String, String)> =
            r.phases.iter().map(|p| (p.index as u64, p.title.clone(), p.state.as_str().to_string())).collect();
        let want: Vec<(u64, String, String)> = er["phases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (p["index"].as_u64().unwrap(), p["title"].as_str().unwrap().into(), p["state"].as_str().unwrap().into()))
            .collect();
        assert_eq!(phases, want, "{}", c("phases"));
        if let Some(t) = er.get("totalTokens") {
            assert_eq!(r.total_tokens, t.as_u64(), "{}", c("totalTokens"));
            assert_eq!(r.total_tool_calls, er["totalToolCalls"].as_u64(), "{}", c("totalToolCalls"));
        } else {
            assert_eq!(r.total_tokens, None);
        }
    }
}

fn check_snapshot(snap: &Snapshot, exp: &Value, flavour: Flavour) {
    let sessions = exp["sessions"].as_object().unwrap();
    assert_eq!(snap.sessions.len(), sessions.len());
    for (sid, es) in sessions {
        check_session(sid, es, &snap.sessions[sid], flavour);
    }
}

#[test]
fn claude_small() {
    let snap = live_snapshot("claude-small");
    check_snapshot(&snap, &expected()["claude-small"], Flavour::Live);
    // Extra checks from the README.
    let s = &snap.sessions["11111111-1111-4111-8111-111111111111"];
    let run = &s.runs["wf_0a1b2c3d-4e5"];
    assert_eq!(run.declared_agent_count, Some(4));
    let api = &s.agents["ab20d5e8f3a7b9202"];
    assert!(api.recent_tools.iter().any(|t| t.name == "Bash" && t.is_error == Some(true)), "one Bash result is an error");
    let survey = &s.agents["a3f1c09d2b7e4a611"];
    assert!(s.files["/tmp/orchard-adjust.txt"].touches[&survey.agent_id].via_bash);
    assert_eq!(survey.spawn_tool_use_id.as_deref().map(|t| t.starts_with("toolu_")), Some(true));
    for id in &run.agent_ids {
        let a = &s.agents[id];
        assert_eq!(a.spawn_tool_use_id, run.tool_use_id, "workflow agents point at the Workflow call");
        assert!(a.ended_at.is_some() && a.started_at.is_some());
    }
}

#[test]
fn claude_inflight() {
    let snap = live_snapshot("claude-inflight");
    check_snapshot(&snap, &expected()["claude-inflight"], Flavour::Live);
    let run = &snap.sessions["11111111-1111-4111-8111-111111111111"].runs["wf_0a1b2c3d-4e5"];
    assert_eq!(run.total_tool_calls, None);
    assert_eq!(run.phases[0].detail.as_deref().map(|d| !d.is_empty()), Some(true), "phase detail parsed from the script");
}

// ---------- replay ----------

fn recording() -> PathBuf {
    fixtures().join("recording-small")
}

/// Rebuilds from the start to `t` (a seek).
fn replay_at(t: i64) -> (Snapshot, EngineStats) {
    let mut src = ReplaySource::open(recording()).unwrap();
    src.set_position(t);
    let mut engine = Engine::new(EngineConfig::replay());
    loop {
        let ch = src.poll();
        if ch.is_empty() {
            break;
        }
        engine.apply_changes(&src, ch);
    }
    (engine.snapshot().clone(), engine.stats().clone())
}

#[test]
fn recording_bounds() {
    let exp = &expected()["recording-small"];
    let src = ReplaySource::open(recording()).unwrap();
    assert_eq!(src.start(), exp["timeline"]["start"].as_i64().unwrap());
    assert_eq!(src.end(), exp["timeline"]["end"].as_i64().unwrap());
    assert_eq!(src.event_count() as u64, exp["eventCount"].as_u64().unwrap());
    assert_eq!(src.initial_scan_len() as u64, exp["initialScanEventCount"].as_u64().unwrap());
    let info = src.info();
    assert!(info.complete);
    let listed = list_recordings(&[fixtures()]);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "recording-small");
}

#[test]
fn replay_start_equals_inflight() {
    let exp = expected();
    let (snap, _) = replay_at(exp["recording-small"]["timeline"]["start"].as_i64().unwrap());
    check_snapshot(&snap, &exp["claude-inflight"], Flavour::Replay);
}

#[test]
fn replay_end_equals_small() {
    let exp = expected();
    let (snap, stats) = replay_at(exp["recording-small"]["timeline"]["end"].as_i64().unwrap());
    check_snapshot(&snap, &exp["claude-small"], Flavour::Replay);
    assert_eq!(stats.bad_lines, 0, "the partial line of event 34 is not a bad line");
}

#[test]
fn replay_at_20s() {
    let exp = &expected()["recording-small"]["at20s"];
    let (snap, _) = replay_at(exp["t"].as_i64().unwrap());
    let s = &snap.sessions["11111111-1111-4111-8111-111111111111"];
    let run = &s.runs["wf_0a1b2c3d-4e5"];
    assert_eq!(run.is_final, exp["runFinal"].as_bool().unwrap());
    assert_eq!(run.agent_ids.len() as u64, exp["workflowAgentsKnown"].as_u64().unwrap());
    for (id, st) in exp["states"].as_object().unwrap() {
        assert_eq!(serde_json::to_value(s.agents[id].state).unwrap(), *st, "agent {id}");
    }
}

#[test]
fn replay_milestones() {
    let m = &expected()["recording-small"]["milestones"];
    let sid = "11111111-1111-4111-8111-111111111111";
    let state = |t: i64, id: &str| replay_at(t).0.sessions[sid].agents.get(id).map(|a| a.state);
    let ui_done = m["buildUiDoneAt"].as_i64().unwrap();
    assert_eq!(state(ui_done - 600, "ab30e6f9a4b8c0303"), Some(AgentState::Running));
    assert_eq!(state(ui_done + 600, "ab30e6f9a4b8c0303"), Some(AgentState::Done));
    let tests_seen = m["buildTestsFirstSeenAt"].as_i64().unwrap();
    assert_eq!(state(tests_seen - 600, "ab40f7a0b5c9d1404"), None);
    assert!(state(tests_seen + 600, "ab40f7a0b5c9d1404").is_some());
    let run_file = m["runFileAppearsAt"].as_i64().unwrap();
    assert!(!replay_at(run_file - 600).0.sessions[sid].runs["wf_0a1b2c3d-4e5"].is_final);
    assert!(replay_at(run_file + 600).0.sessions[sid].runs["wf_0a1b2c3d-4e5"].is_final);
    let idle = m["sessionBecomesIdleAt"].as_i64().unwrap();
    assert_eq!(state(idle - 600, MAIN_AGENT_ID), Some(AgentState::Running));
    assert_eq!(state(idle + 600, MAIN_AGENT_ID), Some(AgentState::Waiting));
}

/// Playing from the start in small steps gives the same snapshot as seeking, at every step.
#[test]
fn play_equals_seek() {
    let mut src = ReplaySource::open(recording()).unwrap();
    let (start, end) = (src.start(), src.end());
    let mut engine = Engine::new(EngineConfig::replay());
    let mut t = start;
    let mut all_events = Vec::new();
    while t <= end + 250 {
        src.set_position(t);
        loop {
            let ch = src.poll();
            if ch.is_empty() {
                break;
            }
            engine.apply_changes(&src, ch);
        }
        all_events.extend(engine.take_events());
        if (t - start) % 5000 == 0 || t >= end {
            let (seeked, _) = replay_at(t);
            assert_eq!(seeked.sessions, engine.snapshot().sessions, "play vs seek at +{} ms", t - start);
        }
        t += 250;
    }
    // Events are numbered without gaps, and an event's t is never after the end.
    for (i, e) in all_events.iter().enumerate() {
        assert_eq!(e.seq, i as u64 + 1);
        assert!(e.t <= end);
    }
}

/// The partial line at fs-event 34 is neither lost nor counted twice.
#[test]
fn partial_line_is_read_once() {
    let exp = &expected()["recording-small"];
    let seq = exp["partialLineEventSeq"].as_u64().unwrap();
    let lines: Vec<Value> = std::fs::read_to_string(recording().join("fs-events.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let t34 = lines.iter().find(|e| e["seq"] == seq).unwrap()["t"].as_i64().unwrap();
    let t35 = lines.iter().find(|e| e["seq"] == seq + 1).unwrap()["t"].as_i64().unwrap();
    let sid = "11111111-1111-4111-8111-111111111111";
    let at34 = replay_at(t34).0.sessions[sid].agents["ab40f7a0b5c9d1404"].tool_calls;
    let at35 = replay_at(t35).0.sessions[sid].agents["ab40f7a0b5c9d1404"].tool_calls;
    assert!(at34 <= at35);
    let end = replay_at(exp["timeline"]["end"].as_i64().unwrap()).0;
    assert_eq!(end.sessions[sid].agents["ab40f7a0b5c9d1404"].tool_calls, 5);
}

#[test]
fn hub_replay_commands() {
    let mut hub = Hub::new_live(|| LiveConfig {
        claude_dir: Some(fixtures().join("claude-small")),
        recent_window_ms: None,
        quiet_after_ms: None,
        watch: false,
        probe: Box::new(FixedPidProbe { alive: vec![4242] }),
    });
    hub.settle();
    let live_events = hub.tick();
    let first_seq = hub.snapshot().seq;
    assert!(first_seq > 0 || live_events.is_empty());
    assert_eq!(hub.replay_play().unwrap_err().code, CommandErrorCode::NotReplaying);

    let st = hub.open_recording(&recording()).unwrap();
    let r = st.replay.unwrap();
    assert!(!r.playing);
    assert_eq!(r.position, r.start);
    let ev = hub.tick();
    assert_eq!(ev[0].kind.type_name(), "snapshotReset");
    assert_eq!(ev[0].seq, first_seq + 1, "seq continues across a mode switch");
    let seq_after_open = hub.snapshot().seq;

    let st = hub.replay_seek(r.start + 20_000).unwrap();
    assert_eq!(st.replay.unwrap().position, r.start + 20_000);
    let ev = hub.tick();
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0].seq, seq_after_open + 1);
    assert_eq!(hub.replay_seek(i64::MAX).unwrap().replay.unwrap().position, r.end, "seek clamps");
    assert_eq!(hub.replay_set_speed(10_000.0).unwrap().replay.unwrap().speed, 600.0);
    assert_eq!(hub.replay_set_speed(0.0).unwrap().replay.unwrap().speed, 0.1);
    hub.tick();
    // Play at the end restarts from the start.
    let st = hub.replay_play().unwrap().replay.unwrap();
    assert!(st.playing);
    assert_eq!(st.position, st.start);
    hub.replay_pause().unwrap();
    let st = hub.switch_to_live();
    assert_eq!(st.mode, SourceMode::Live);
    let ev = hub.tick();
    assert_eq!(ev[0].kind.type_name(), "snapshotReset");
}

#[test]
fn hub_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Hub>();
}

/// The live watcher reports a new session file and the transcript appended to it.
#[test]
fn live_watcher_sees_changes() {
    let dir = std::env::temp_dir().join(format!("virgil-watch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sessions")).unwrap();
    std::fs::create_dir_all(dir.join("projects/-w")).unwrap();
    let mut hub = Hub::new_live({
        let dir = dir.clone();
        move || LiveConfig {
            claude_dir: Some(dir.clone()),
            recent_window_ms: None,
            quiet_after_ms: None,
            watch: true,
            probe: Box::new(FixedPidProbe { alive: vec![1] }),
        }
    });
    hub.settle();
    assert!(hub.status().error.is_none());
    std::thread::sleep(std::time::Duration::from_millis(200));
    std::fs::write(dir.join("sessions/1.json"), br#"{"pid":1,"sessionId":"w1","cwd":"/w","status":"busy"}"#).unwrap();
    std::fs::write(
        dir.join("projects/-w/w1.jsonl"),
        b"{\"type\":\"ai-title\",\"aiTitle\":\"Watched\"}\n",
    )
    .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut title = None;
    while std::time::Instant::now() < deadline {
        hub.tick();
        title = hub.snapshot().sessions.get("w1").and_then(|s| s.info.title.clone());
        if title.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(title.as_deref(), Some("Watched"));
}

/// A tiny recording with one quiet stretch of 99 s between two fs-events.
fn gap_recording() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("virgil-gap-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let s: i64 = 1_800_000_000_000;
    std::fs::write(
        dir.join("manifest.json"),
        format!(r#"{{"format":1,"startedAt":{s},"pollMs":500,"stoppedAt":{}}}"#, s + 200_000),
    )
    .unwrap();
    let ev = |seq: u64, t: i64| format!(r#"{{"seq":{seq},"t":{t},"op":"modify","path":"projects/-x/a.txt","size":1,"mtimeMs":{t}}}"#);
    std::fs::write(dir.join("fs-events.jsonl"), format!("{}\n{}\n{}\n", ev(1, s + 10), ev(2, s + 1_000), ev(3, s + 100_000))).unwrap();
    dir
}

#[test]
fn hub_skips_quiet_gaps_when_asked() {
    let dir = gap_recording();
    let run = |skip: bool| {
        let mut hub = Hub::new_live(|| LiveConfig { claude_dir: Some(fixtures().join("claude-small")), watch: false, ..LiveConfig::default() });
        assert!(hub.set_skip_gaps(skip).replay.is_none(), "the choice can be made in live mode");
        let st = hub.open_recording(&dir).unwrap().replay.unwrap();
        assert_eq!(st.skip_gaps, skip, "the choice carries over to the recording");
        hub.replay_seek(st.start + 2_000).unwrap();
        hub.replay_play().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        hub.tick();
        hub.status().replay.unwrap().position - (st.start + 2_000)
    };
    let skipped = run(true);
    let normal = run(false);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(normal < 2_000, "1x moves about 300 ms, moved {normal}");
    assert!(skipped > 20_000, "a 99 s gap plays in about 1 s, moved {skipped} in 300 ms");
}

#[test]
fn hub_set_claude_dir_rebuilds_live() {
    let mut hub = Hub::new_live(|| LiveConfig {
        claude_dir: Some(fixtures().join("claude-small")),
        recent_window_ms: None,
        quiet_after_ms: None,
        watch: false,
        probe: Box::new(FixedPidProbe { alive: vec![4242] }),
    });
    hub.settle();
    let small = hub.snapshot().sessions.keys().cloned().collect::<BTreeSet<_>>();
    assert!(!small.is_empty());
    let err = hub.set_claude_dir(Some(fixtures().join("nope"))).unwrap_err();
    assert_eq!(err.code, CommandErrorCode::NotFound);
    let st = hub.set_claude_dir(Some(fixtures().join("claude-inflight"))).unwrap();
    assert!(st.claude_dir.unwrap().ends_with("claude-inflight"));
    let ev = hub.tick();
    assert_eq!(ev[0].kind.type_name(), "snapshotReset");
    hub.settle();
    let st = hub.set_claude_dir(None).unwrap();
    assert!(st.claude_dir.unwrap().ends_with("claude-small"), "None goes back to the configured default");
    // In replay the choice waits for the next switch to live.
    hub.open_recording(&recording()).unwrap();
    let st = hub.set_claude_dir(Some(fixtures().join("claude-inflight"))).unwrap();
    assert_eq!(st.mode, SourceMode::Replay);
    assert!(hub.switch_to_live().claude_dir.unwrap().ends_with("claude-inflight"));
}

#[test]
fn hub_tool_history_lists_the_recording_timeline() {
    let mut hub = Hub::new_live(|| LiveConfig { claude_dir: Some(fixtures().join("claude-small")), watch: false, ..LiveConfig::default() });
    let st = hub.open_recording(&recording()).unwrap().replay.unwrap();
    hub.replay_seek(st.end).unwrap();
    let h = hub.tool_history();
    assert_eq!(h.seq, hub.snapshot().seq);
    assert_eq!(h.timeline_times.len(), 46);
    assert!(h.timeline_times.windows(2).all(|w| w[0] <= w[1]));
    let calls: usize = h.agents.iter().map(|a| a.calls.len()).sum();
    let counted: u32 = hub.snapshot().sessions.values().flat_map(|s| s.agents.values()).map(|a| a.tool_calls).sum();
    assert!(calls > 0 && calls as u32 <= counted);
}
