//! Lexical path normalisation (contract 2.7) and classification of Claude-dir paths.

/// Normalises a path from a tool input: `\` becomes `/`, relative paths are joined to `cwd`, `.` and
/// `..` are resolved lexically. Returns `None` for an empty path or a relative path without a cwd.
pub fn normalise(path: &str, cwd: Option<&str>) -> Option<String> {
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let path = path.replace('\\', "/");
    let full = if is_absolute(&path) {
        path
    } else {
        let cwd = cwd?.replace('\\', "/");
        if !is_absolute(&cwd) {
            return None;
        }
        format!("{}/{}", cwd.trim_end_matches('/'), path)
    };
    // Split off a drive prefix such as `C:`.
    let (prefix, rest) = match full.find('/') {
        Some(i) if i > 0 && full[..i].ends_with(':') => (full[..i].to_string(), &full[i..]),
        _ => (String::new(), full.as_str()),
    };
    let mut parts: Vec<&str> = Vec::new();
    for seg in rest.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    Some(format!("{}/{}", prefix, parts.join("/")))
}

fn is_absolute(p: &str) -> bool {
    if p.starts_with('/') {
        return true;
    }
    let b = p.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'/'
}

/// What a path relative to the Claude dir is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudePath {
    SessionFile { pid: u32 },
    MainTranscript { slug: String, session_id: String },
    AgentTranscript { slug: String, session_id: String, run_id: Option<String>, agent_id: String },
    AgentMeta { slug: String, session_id: String, run_id: Option<String>, agent_id: String },
    Journal { slug: String, session_id: String, run_id: String },
    RunFile { slug: String, session_id: String, run_id: String },
    Script { slug: String, session_id: String, file_name: String },
}

impl ClaudePath {
    pub fn session_id(&self) -> Option<&str> {
        match self {
            ClaudePath::SessionFile { .. } => None,
            ClaudePath::MainTranscript { session_id, .. }
            | ClaudePath::AgentTranscript { session_id, .. }
            | ClaudePath::AgentMeta { session_id, .. }
            | ClaudePath::Journal { session_id, .. }
            | ClaudePath::RunFile { session_id, .. }
            | ClaudePath::Script { session_id, .. } => Some(session_id),
        }
    }
}

fn agent_file(name: &str) -> Option<(String, bool)> {
    let rest = name.strip_prefix("agent-")?;
    if let Some(id) = rest.strip_suffix(".meta.json") {
        return (!id.is_empty()).then(|| (id.to_string(), true));
    }
    let id = rest.strip_suffix(".jsonl")?;
    (!id.is_empty()).then(|| (id.to_string(), false))
}

/// Classifies a path relative to the Claude dir (forward slashes). `None` for anything the app ignores.
pub fn classify(rel: &str) -> Option<ClaudePath> {
    let parts: Vec<&str> = rel.split('/').collect();
    match parts.as_slice() {
        ["sessions", file] => {
            let pid = file.strip_suffix(".json")?;
            if pid.contains('.') {
                return None;
            }
            Some(ClaudePath::SessionFile { pid: pid.parse().ok()? })
        }
        ["projects", slug, file] => {
            let sid = file.strip_suffix(".jsonl")?;
            Some(ClaudePath::MainTranscript { slug: slug.to_string(), session_id: sid.to_string() })
        }
        ["projects", slug, sid, "subagents", file] => {
            let (agent_id, meta) = agent_file(file)?;
            let (slug, session_id) = (slug.to_string(), sid.to_string());
            Some(if meta {
                ClaudePath::AgentMeta { slug, session_id, run_id: None, agent_id }
            } else {
                ClaudePath::AgentTranscript { slug, session_id, run_id: None, agent_id }
            })
        }
        ["projects", slug, sid, "subagents", "workflows", run, file] => {
            let (slug, session_id, run_id) = (slug.to_string(), sid.to_string(), run.to_string());
            if *file == "journal.jsonl" {
                return Some(ClaudePath::Journal { slug, session_id, run_id });
            }
            let (agent_id, meta) = agent_file(file)?;
            Some(if meta {
                ClaudePath::AgentMeta { slug, session_id, run_id: Some(run_id), agent_id }
            } else {
                ClaudePath::AgentTranscript { slug, session_id, run_id: Some(run_id), agent_id }
            })
        }
        ["projects", slug, sid, "workflows", file] => {
            let run_id = file.strip_suffix(".json")?;
            Some(ClaudePath::RunFile { slug: slug.to_string(), session_id: sid.to_string(), run_id: run_id.to_string() })
        }
        ["projects", slug, sid, "workflows", "scripts", file] if file.ends_with(".js") => Some(ClaudePath::Script {
            slug: slug.to_string(),
            session_id: sid.to_string(),
            file_name: file.to_string(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises() {
        assert_eq!(normalise("src/a.ts", Some("/home/dev/p")).as_deref(), Some("/home/dev/p/src/a.ts"));
        assert_eq!(normalise("./x/../b.ts", Some("/r/")).as_deref(), Some("/r/b.ts"));
        assert_eq!(normalise("/a/./b//c/../d", None).as_deref(), Some("/a/b/d"));
        assert_eq!(normalise("C:\\Users\\x\\..\\y.txt", None).as_deref(), Some("C:/Users/y.txt"));
        assert_eq!(normalise("rel", None), None);
    }

    #[test]
    fn classifies() {
        assert_eq!(classify("sessions/42.json"), Some(ClaudePath::SessionFile { pid: 42 }));
        assert_eq!(classify("sessions/42.abc.key"), None);
        assert!(matches!(classify("projects/-a/s1.jsonl"), Some(ClaudePath::MainTranscript { .. })));
        assert_eq!(
            classify("projects/-a/s1/subagents/agent-afix-the-b-0c1d.meta.json"),
            Some(ClaudePath::AgentMeta {
                slug: "-a".into(),
                session_id: "s1".into(),
                run_id: None,
                agent_id: "afix-the-b-0c1d".into()
            })
        );
        assert!(matches!(classify("projects/-a/s1/subagents/workflows/wf_1/journal.jsonl"), Some(ClaudePath::Journal { .. })));
        assert!(matches!(classify("projects/-a/s1/workflows/wf_1.json"), Some(ClaudePath::RunFile { .. })));
        assert!(matches!(classify("projects/-a/s1/workflows/scripts/x-wf_1.js"), Some(ClaudePath::Script { .. })));
        assert_eq!(classify("projects/-a/s1/tool-results/x.txt"), None);
    }
}
