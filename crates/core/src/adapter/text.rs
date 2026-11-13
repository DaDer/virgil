//! Text rules: the task rule (contract 2.4), tool summaries (2.5) and completion notice tags (1.2).

use serde_json::Value;

pub const TASK_MAX: usize = 280;
pub const SUMMARY_MAX: usize = 120;
const HARNESS_PREFIX: &str = "[Workflow harness";
const COMPUTED_TASK_PREFIX: &str = "[Workflow harness — computed task]";

/// Cuts `s` to at most `max` characters, ending with `…` when cut.
pub fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

pub fn is_harness_frame(text: &str) -> bool {
    text.starts_with(HARNESS_PREFIX)
}

pub fn is_computed_task(text: &str) -> bool {
    text.starts_with(COMPUTED_TASK_PREFIX)
}

/// The text of a `[Workflow harness — computed task]` record without its first line and with the
/// two-space indent removed.
pub fn unframe_computed_task(text: &str) -> String {
    let rest = match text.find('\n') {
        Some(i) => &text[i + 1..],
        None => "",
    };
    rest.split('\n').map(|l| l.strip_prefix("  ").unwrap_or(l)).collect::<Vec<_>>().join("\n")
}

/// Trim, cut at the first blank line, collapse whitespace, cut to 280 characters.
pub fn tidy_task(text: &str) -> String {
    let text = text.replace("\r\n", "\n");
    let trimmed = text.trim();
    let mut first_para = String::new();
    for line in trimmed.split('\n') {
        if line.trim().is_empty() {
            break;
        }
        first_para.push_str(line);
        first_para.push('\n');
    }
    let collapsed = first_para.split_whitespace().collect::<Vec<_>>().join(" ");
    cut(&collapsed, TASK_MAX)
}

/// Applies the task rule to a single prompt text that may be framed (used for run-file
/// `promptPreview`, where only one text exists).
pub fn task_from_single_prompt(text: &str) -> String {
    if let Some(i) = text.find(COMPUTED_TASK_PREFIX) {
        return tidy_task(&unframe_computed_task(&text[i..]));
    }
    if is_harness_frame(text) {
        return String::new();
    }
    tidy_task(text)
}

/// Relative display path for summaries.
pub fn display_path(path: &str, cwd: Option<&str>) -> String {
    if let Some(cwd) = cwd {
        let cwd = cwd.trim_end_matches('/');
        if !cwd.is_empty() {
            if let Some(rest) = path.strip_prefix(cwd) {
                if let Some(rel) = rest.strip_prefix('/') {
                    if !rel.is_empty() {
                        return rel.to_string();
                    }
                }
            }
        }
    }
    path.to_string()
}

fn str_field<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input.get(key).and_then(Value::as_str)
}

/// One-line summary of a tool call, at most 120 characters.
pub fn tool_summary(name: &str, input: &Value, cwd: Option<&str>, workflow_name: Option<&str>) -> String {
    let s: String = match name {
        "Bash" => match str_field(input, "description").filter(|d| !d.trim().is_empty()) {
            Some(d) => d.to_string(),
            None => str_field(input, "command").and_then(|c| c.lines().next()).unwrap_or("").to_string(),
        },
        "Read" | "Edit" | "Write" | "MultiEdit" => {
            str_field(input, "file_path").map(|p| display_path(p, cwd)).unwrap_or_default()
        }
        "NotebookEdit" => str_field(input, "notebook_path").map(|p| display_path(p, cwd)).unwrap_or_default(),
        "Agent" | "Task" => str_field(input, "description").unwrap_or("").to_string(),
        "Workflow" => workflow_name.unwrap_or("workflow").to_string(),
        "Grep" | "Glob" => str_field(input, "pattern").unwrap_or("").to_string(),
        "WebFetch" => str_field(input, "url").unwrap_or("").to_string(),
        "WebSearch" => str_field(input, "query").unwrap_or("").to_string(),
        "Skill" => str_field(input, "skill").unwrap_or("").to_string(),
        "SendMessage" => str_field(input, "summary").unwrap_or("").to_string(),
        _ => String::new(),
    };
    let one_line = s.split_whitespace().collect::<Vec<_>>().join(" ");
    cut(&one_line, SUMMARY_MAX)
}

/// The text between `<tag>` and `</tag>`, trimmed.
pub fn xml_tag<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&close)? + start;
    Some(text[start..end].trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn task_rule() {
        assert_eq!(tidy_task("  Survey the inventory module.\n\nList every place"), "Survey the inventory module.");
        assert_eq!(tidy_task("a\n  b\tc"), "a b c");
        let long = "x".repeat(400);
        let t = tidy_task(&long);
        assert_eq!(t.chars().count(), 280);
        assert!(t.ends_with('…'));
        let framed = "[Workflow harness — computed task] The task text below...\n  Outline the work\n  for stock.\n\n  More.";
        assert_eq!(tidy_task(&unframe_computed_task(framed)), "Outline the work for stock.");
        assert_eq!(task_from_single_prompt(framed), "Outline the work for stock.");
    }

    #[test]
    fn summaries() {
        let cwd = Some("/home/dev/p");
        assert_eq!(tool_summary("Read", &json!({"file_path":"/home/dev/p/src/a.ts"}), cwd, None), "src/a.ts");
        assert_eq!(tool_summary("Read", &json!({"file_path":"/etc/x"}), cwd, None), "/etc/x");
        assert_eq!(tool_summary("Bash", &json!({"command":"npm test\nmore"}), cwd, None), "npm test");
        assert_eq!(tool_summary("Bash", &json!({"command":"x","description":"Run tests"}), cwd, None), "Run tests");
        assert_eq!(tool_summary("Workflow", &json!({}), cwd, None), "workflow");
        assert_eq!(tool_summary("Mystery", &json!({"a":1}), cwd, None), "");
        assert_eq!(tool_summary("Grep", &json!({"pattern":"a".repeat(200)}), cwd, None).chars().count(), 120);
    }

    #[test]
    fn tags() {
        let n = "<task-notification>\n<task-id>abc</task-id>\n<status>completed</status>";
        assert_eq!(xml_tag(n, "task-id"), Some("abc"));
        assert_eq!(xml_tag(n, "status"), Some("completed"));
        assert_eq!(xml_tag(n, "tool-use-id"), None);
    }
}
