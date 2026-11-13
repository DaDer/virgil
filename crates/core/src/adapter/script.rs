//! Lenient extraction of `meta.name` and `meta.phases` from a workflow script (contract 4, step 2).
//! No JavaScript parser: bracket counting that skips string literals and comments.

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DeclaredPhase {
    pub title: String,
    pub detail: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScriptMeta {
    pub name: Option<String>,
    /// `None` when the phases could not be extracted.
    pub phases: Option<Vec<DeclaredPhase>>,
}

pub fn extract_meta(script: &str) -> ScriptMeta {
    let Some(body) = meta_object(script) else { return ScriptMeta::default() };
    let name = top_level_string(body, "name");
    let phases = phases_array(body).map(|arr| {
        top_level_objects(arr)
            .into_iter()
            .filter_map(|obj| {
                let title = top_level_string(obj, "title")?;
                Some(DeclaredPhase {
                    title,
                    detail: top_level_string(obj, "detail"),
                    model: top_level_string(obj, "model"),
                })
            })
            .collect::<Vec<_>>()
    });
    ScriptMeta { name, phases }
}

/// Text inside the `{ ... }` of `meta = { ... }`.
fn meta_object(src: &str) -> Option<&str> {
    let b = src.as_bytes();
    let mut at = 0;
    while at < b.len() {
        match b[at] {
            b'\'' | b'"' | b'`' => {
                at = skip_string(b, at)?;
                continue;
            }
            b'/' if matches!(b.get(at + 1), Some(b'/') | Some(b'*')) => {
                at = skip_comment(b, at);
                continue;
            }
            _ => {}
        }
        let is_token = b[at..].starts_with(b"meta")
            && (at == 0 || !is_ident(b[at - 1]))
            && b.get(at + 4).is_none_or(|c| !is_ident(*c));
        if is_token {
            let i = skip_ws(b, at + 4);
            if b.get(i) == Some(&b'=') && b.get(i + 1) != Some(&b'=') {
                let i = skip_ws(b, i + 1);
                if b.get(i) == Some(&b'{') {
                    let end = matching(b, i)?;
                    return Some(&src[i + 1..end]);
                }
            }
        }
        at += 1;
    }
    None
}

fn phases_array(body: &str) -> Option<&str> {
    let b = body.as_bytes();
    let key = find_top_level_key(body, "phases")?;
    let mut i = skip_ws(b, key);
    if b.get(i) != Some(&b'[') {
        return None;
    }
    let end = matching(b, i)?;
    i += 1;
    Some(&body[i..end])
}

/// Position just after `key:` for a key at depth 0 of `body`.
fn find_top_level_key(body: &str, key: &str) -> Option<usize> {
    let b = body.as_bytes();
    let mut depth = 0i32;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            b'\'' | b'"' | b'`' => {
                let end = skip_string(b, i)?;
                // Quoted keys: 'phases': [...]
                if depth == 0 && &b[i + 1..end - 1] == key.as_bytes() {
                    let j = skip_ws(b, end);
                    if b.get(j) == Some(&b':') {
                        return Some(j + 1);
                    }
                }
                i = end;
                continue;
            }
            b'/' if b.get(i + 1) == Some(&b'/') || b.get(i + 1) == Some(&b'*') => {
                i = skip_comment(b, i);
                continue;
            }
            b'{' | b'[' | b'(' => depth += 1,
            b'}' | b']' | b')' => depth -= 1,
            _ if depth == 0 && b[i..].starts_with(key.as_bytes()) && (i == 0 || !is_ident(b[i - 1])) => {
                let j = skip_ws(b, i + key.len());
                if b.get(i + key.len()).is_none_or(|c| !is_ident(*c)) && b.get(j) == Some(&b':') {
                    return Some(j + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn top_level_string(body: &str, key: &str) -> Option<String> {
    let b = body.as_bytes();
    let i = skip_ws(b, find_top_level_key(body, key)?);
    match b.get(i) {
        Some(b'\'') | Some(b'"') | Some(b'`') => {
            let end = skip_string(b, i)?;
            Some(unescape(&body[i + 1..end - 1]))
        }
        _ => None,
    }
}

fn top_level_objects(arr: &str) -> Vec<&str> {
    let b = arr.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\'' | b'"' | b'`' => match skip_string(b, i) {
                Some(e) => i = e,
                None => break,
            },
            b'/' if b.get(i + 1) == Some(&b'/') || b.get(i + 1) == Some(&b'*') => i = skip_comment(b, i),
            b'{' => match matching(b, i) {
                Some(e) => {
                    out.push(&arr[i + 1..e]);
                    i = e + 1;
                }
                None => break,
            },
            b'[' | b'(' => match matching(b, i) {
                Some(e) => i = e + 1,
                None => break,
            },
            _ => i += 1,
        }
    }
    out
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c == b'.'
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if b.get(i) == Some(&b'/') && matches!(b.get(i + 1), Some(b'/') | Some(b'*')) {
            i = skip_comment(b, i);
        } else {
            return i;
        }
    }
}

fn skip_comment(b: &[u8], i: usize) -> usize {
    if b.get(i + 1) == Some(&b'/') {
        let mut j = i + 2;
        while j < b.len() && b[j] != b'\n' {
            j += 1;
        }
        j
    } else {
        let mut j = i + 2;
        while j + 1 < b.len() && !(b[j] == b'*' && b[j + 1] == b'/') {
            j += 1;
        }
        (j + 2).min(b.len())
    }
}

/// Index just past the closing quote of the string literal starting at `i`.
fn skip_string(b: &[u8], i: usize) -> Option<usize> {
    let q = b[i];
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            c if c == q => return Some(j + 1),
            _ => j += 1,
        }
    }
    None
}

/// Index of the bracket matching the one at `open`.
fn matching(b: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'\'' | b'"' | b'`' => {
                i = skip_string(b, i)?;
                continue;
            }
            b'/' if matches!(b.get(i + 1), Some(b'/') | Some(b'*')) => {
                i = skip_comment(b, i);
                continue;
            }
            b'{' | b'[' | b'(' => depth += 1,
            b'}' | b']' | b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_phases() {
        let src = r#"
// leading comment with meta = { broken
export const meta = {
  name: 'orchard-stock',
  description: "uses ] and } in a string, and a 'quote'",
  phases: [
    { title: 'Plan', detail: 'one outline for everybody', model: 'opus' },
    { title: "Build", detail: 'it\'s parallel' }, // trailing comment
    { detail: 'no title is skipped' },
  ],
}
export default async function () { const phases = [] }
"#;
        let m = extract_meta(src);
        assert_eq!(m.name.as_deref(), Some("orchard-stock"));
        let p = m.phases.unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0], DeclaredPhase { title: "Plan".into(), detail: Some("one outline for everybody".into()), model: Some("opus".into()) });
        assert_eq!(p[1].title, "Build");
        assert_eq!(p[1].detail.as_deref(), Some("it's parallel"));
        assert_eq!(p[1].model, None);
    }

    #[test]
    fn tolerates_garbage() {
        assert_eq!(extract_meta("not a script"), ScriptMeta::default());
        assert_eq!(extract_meta("const meta = { name: 'x', phases: [ {title: 'a'"), ScriptMeta::default());
        let m = extract_meta("const meta = { name: 'x' }");
        assert_eq!(m.name.as_deref(), Some("x"));
        assert_eq!(m.phases, None);
    }
}
