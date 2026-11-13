//! Prints the snapshot the core builds, as a readable summary or as JSON.
//!
//! cargo run -p virgil-core --example dump -- (--live | --recording <dir>) [--at <epoch ms>] [--json]
//!     [--claude-dir <dir>] [--window-hours <h>] [--no-tasks]
//!
//! Task text comes from prompts, which are private: it goes to stdout only; `--no-tasks` hides it.

use virgil_core::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::exit;
use std::time::Instant;

const MAX_CHILDREN: usize = 12;

fn usage() -> ! {
    eprintln!("usage: dump (--live | --recording <dir>) [--at <epoch ms>] [--json] [--claude-dir <dir>] [--window-hours <h>] [--no-tasks]");
    exit(2)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut live, mut recording, mut at, mut json, mut tasks) = (false, None::<PathBuf>, None::<i64>, false, true);
    let (mut claude_dir, mut window_h) = (None::<PathBuf>, Some(24.0f64));
    while let Some(a) = args.next() {
        match a.as_str() {
            "--live" => live = true,
            "--recording" => recording = Some(args.next().unwrap_or_else(|| usage()).into()),
            "--at" => at = Some(args.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage())),
            "--json" => json = true,
            "--no-tasks" => tasks = false,
            "--claude-dir" => claude_dir = Some(args.next().unwrap_or_else(|| usage()).into()),
            "--window-hours" => {
                let h: f64 = args.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage());
                window_h = (h > 0.0).then_some(h);
            }
            _ => usage(),
        }
    }
    if live == recording.is_some() {
        usage();
    }
    let started = Instant::now();
    let mut hub = Hub::new_live(move || LiveConfig {
        claude_dir: claude_dir.clone(),
        recent_window_ms: window_h.map(|h| (h * 3600.0 * 1000.0) as i64),
        watch: false,
        ..LiveConfig::default()
    });
    if let Some(dir) = recording {
        if let Err(e) = hub.open_recording(&dir) {
            eprintln!("cannot open recording: {e}");
            exit(1);
        }
        if let Some(t) = at {
            hub.replay_seek(t).expect("seek");
        } else {
            let end = hub.status().replay.map(|r| r.end).unwrap_or(0);
            hub.replay_seek(end).expect("seek");
        }
        hub.tick();
    } else {
        hub.settle();
    }
    let elapsed = started.elapsed();
    let snap = hub.snapshot();
    if json {
        println!("{}", serde_json::to_string_pretty(snap).unwrap());
        return;
    }
    let status = hub.status();
    match &status.replay {
        Some(r) => println!(
            "replay {}  position {} ({:+.1} s from start, timeline {:.1} s)  events {}  complete {}",
            status.recording.as_ref().map(|r| r.name.as_str()).unwrap_or("?"),
            r.position,
            (r.position - r.start) as f64 / 1000.0,
            (r.end - r.start) as f64 / 1000.0,
            status.recording.as_ref().map(|r| r.event_count).unwrap_or(0),
            status.recording.as_ref().map(|r| r.complete).unwrap_or(false),
        ),
        None => println!("live {}{}", status.claude_dir.unwrap_or_default(), status.error.map(|e| format!("  error: {e}")).unwrap_or_default()),
    }
    let st = hub.stats();
    println!(
        "built in {:.2} s: {} MB read, {} lines, {} bad lines, {} bad files; snapshot seq {}",
        elapsed.as_secs_f64(),
        st.bytes_read / 1_000_000,
        st.lines_read,
        st.bad_lines,
        st.bad_files,
        snap.seq
    );
    let tool_history = hub.tool_history();
    println!(
        "tool history: {} calls over {} agents; {} timeline times",
        tool_history.agents.iter().map(|a| a.calls.len()).sum::<usize>(),
        tool_history.agents.len(),
        tool_history.timeline_times.len()
    );
    for p in &status.problems {
        println!("problem: {:?} x{} {}", p.reason, p.count, p.path);
    }
    println!("{} sessions", snap.sessions.len());
    for s in snap.sessions.values() {
        print_session(s, tasks);
    }
}

fn print_session(s: &Session, tasks: bool) {
    let i = &s.info;
    println!();
    println!(
        "session {}  pid {}  {}  status {}  {}",
        i.session_id,
        i.pid.map(|p| p.to_string()).unwrap_or("-".into()),
        i.liveness.as_str(),
        i.status.as_deref().unwrap_or("-"),
        i.cwd.as_deref().unwrap_or("-"),
    );
    println!(
        "  title: {}   transcript: {}   agents {}   runs {}   files {} ({} shared)",
        i.title.as_deref().or(i.name.as_deref()).unwrap_or("-"),
        if i.has_transcript { i.project_slug.as_deref().unwrap_or("?") } else { "none" },
        s.agents.len(),
        s.runs.len(),
        s.files.len(),
        s.files.values().filter(|f| f.touches.len() > 1).count(),
    );
    let mut by_state: BTreeMap<&str, usize> = BTreeMap::new();
    for a in s.agents.values() {
        *by_state.entry(a.state.as_str()).or_default() += 1;
    }
    println!("  agent states: {}", by_state.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", "));
    // Main and plain subagents as a tree; workflow agents under their runs.
    let mut children: BTreeMap<String, Vec<&Agent>> = BTreeMap::new();
    for a in s.agents.values().filter(|a| a.run_id.is_none() && a.agent_id != MAIN_AGENT_ID) {
        let parent = a.parent_agent_id.clone().filter(|p| s.agents.contains_key(p)).unwrap_or(MAIN_AGENT_ID.into());
        children.entry(parent).or_default().push(a);
    }
    for v in children.values_mut() {
        v.sort_by_key(|a| a.started_at.unwrap_or(0));
    }
    if let Some(main) = s.agents.get(MAIN_AGENT_ID) {
        print_tree(main, &children, 1, tasks);
    }
    let mut runs: Vec<&WorkflowRun> = s.runs.values().collect();
    runs.sort_by_key(|r| r.started_at.unwrap_or(0));
    for r in runs {
        println!(
            "  run {} \"{}\"  {}{}  parent {}  agents {}{}  tokens {}  tool calls {}",
            r.run_id,
            r.name,
            r.status.as_str(),
            if r.is_final { " (final)" } else { " (reconstructed)" },
            r.parent_agent_id,
            r.agent_ids.len(),
            r.declared_agent_count.map(|c| format!(" (run file: {c})")).unwrap_or_default(),
            r.total_tokens.map(|t| t.to_string()).unwrap_or_else(|| {
                format!("~{}", r.agent_ids.iter().filter_map(|id| s.agents.get(id)).map(|a| a.tokens).sum::<u64>())
            }),
            r.total_tool_calls.map(|t| t.to_string()).unwrap_or_else(|| {
                format!("~{}", r.agent_ids.iter().filter_map(|id| s.agents.get(id)).map(|a| a.tool_calls as u64).sum::<u64>())
            }),
        );
        for p in &r.phases {
            let agents: Vec<&Agent> =
                r.agent_ids.iter().filter_map(|id| s.agents.get(id)).filter(|a| a.phase_index == Some(p.index)).collect();
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for a in &agents {
                *counts.entry(if a.superseded { "superseded" } else { a.state.as_str() }).or_default() += 1;
            }
            println!(
                "    phase {} {:<24} {:<8} {} agents: {}",
                p.index,
                p.title,
                p.state.as_str(),
                agents.len(),
                counts.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ")
            );
        }
        let orphan = r.agent_ids.iter().filter_map(|id| s.agents.get(id)).filter(|a| a.phase_index.is_none()).count();
        if orphan > 0 {
            println!("    (no phase: {orphan} agents)");
        }
    }
}

fn print_tree(a: &Agent, children: &BTreeMap<String, Vec<&Agent>>, depth: usize, tasks: bool) {
    let pad = "  ".repeat(depth);
    println!(
        "{pad}{} [{}{}] {}  {}  tools {}  tokens {}{}",
        a.agent_id,
        a.state.as_str(),
        a.state_reason.as_ref().map(|r| format!(": {r}")).unwrap_or_default(),
        a.label,
        a.model.as_deref().unwrap_or("-"),
        a.tool_calls,
        a.tokens,
        a.last_tool.as_ref().map(|t| format!("  last {}", t.name)).unwrap_or_default(),
    );
    if tasks && !a.task.is_empty() {
        println!("{pad}  task: {}", virgil_core::adapter::text::cut(&a.task, 110));
    }
    let kids = children.get(&a.agent_id).map(Vec::as_slice).unwrap_or(&[]);
    for c in kids.iter().take(MAX_CHILDREN) {
        print_tree(c, children, depth + 1, tasks);
    }
    if kids.len() > MAX_CHILDREN {
        println!("{pad}  ... {} more", kids.len() - MAX_CHILDREN);
    }
}
