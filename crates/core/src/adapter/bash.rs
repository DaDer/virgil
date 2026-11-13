//! Conservative file-touch detection in Bash commands (contract 2.7).

use crate::contract::FileOp;

pub const MAX_TOUCHES: usize = 8;

#[derive(Debug, PartialEq)]
enum Tok {
    Word(String),
    Sep,
    /// An output redirection (`>`, `>>`, `2>`, `&>`); the next word is its target.
    RedirOut,
    /// An input redirection (`<`); the next word is consumed and ignored.
    RedirIn,
}

#[allow(unused_assignments)]
fn tokenize(cmd: &str) -> Vec<Tok> {
    let mut toks = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let chars: Vec<char> = cmd.chars().collect();
    let mut i = 0;
    macro_rules! flush {
        () => {
            if in_word {
                toks.push(Tok::Word(std::mem::take(&mut word)));
                in_word = false;
            }
        };
    }
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\'' => {
                in_word = true;
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    word.push(chars[i]);
                    i += 1;
                }
            }
            '"' => {
                in_word = true;
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() && matches!(chars[i + 1], '"' | '\\' | '$' | '`') {
                        i += 1;
                    }
                    word.push(chars[i]);
                    i += 1;
                }
            }
            '\\' => {
                in_word = true;
                if i + 1 < chars.len() {
                    i += 1;
                    if chars[i] != '\n' {
                        word.push(chars[i]);
                    }
                }
            }
            ' ' | '\t' => flush!(),
            '\n' | ';' => {
                flush!();
                toks.push(Tok::Sep);
            }
            '|' => {
                flush!();
                if chars.get(i + 1) == Some(&'|') {
                    i += 1;
                }
                toks.push(Tok::Sep);
            }
            '&' => {
                if chars.get(i + 1) == Some(&'&') {
                    flush!();
                    i += 1;
                    toks.push(Tok::Sep);
                } else if chars.get(i + 1) == Some(&'>') && !in_word {
                    i += 1;
                    if chars.get(i + 1) == Some(&'>') {
                        i += 1;
                    }
                    toks.push(Tok::RedirOut);
                } else if in_word {
                    word.push('&');
                } else {
                    toks.push(Tok::Sep);
                }
            }
            '<' => {
                if chars.get(i + 1) == Some(&'<') {
                    // A heredoc: stop reading the command here.
                    flush!();
                    return toks;
                }
                flush!();
                toks.push(Tok::RedirIn);
            }
            '>' => {
                // `2>` and `1>`: a word consisting only of a fd number becomes part of the operator.
                if in_word && (word == "2" || word == "1") {
                    word.clear();
                    in_word = false;
                } else {
                    flush!();
                }
                if chars.get(i + 1) == Some(&'>') {
                    i += 1;
                }
                toks.push(Tok::RedirOut);
            }
            _ => {
                in_word = true;
                word.push(c);
            }
        }
        i += 1;
    }
    flush!();
    toks
}

/// Whether a word looks like a path argument.
fn is_path_arg(w: &str) -> bool {
    if w.is_empty() || w.starts_with('-') || w.starts_with('~') || w.ends_with('/') {
        return false;
    }
    if w.contains(['*', '?', '[', '$', '`']) {
        return false;
    }
    if w.contains('/') {
        return true;
    }
    match w.rfind('.') {
        Some(i) => {
            let ext = &w[i + 1..];
            (1..=8).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric()) && i > 0
        }
        None => false,
    }
}

/// The (path, op) pairs a Bash command reads or writes, paths as written (not yet normalised).
pub fn touches(command: &str) -> Vec<(String, FileOp)> {
    let toks = tokenize(command);
    let mut out: Vec<(String, FileOp)> = Vec::new();
    let mut simple: Vec<String> = Vec::new();
    let mut it = toks.into_iter().peekable();
    let finish = |words: &mut Vec<String>, out: &mut Vec<(String, FileOp)>| {
        command_touches(words, out);
        words.clear();
    };
    while let Some(tok) = it.next() {
        match tok {
            Tok::Word(w) => simple.push(w),
            Tok::Sep => finish(&mut simple, &mut out),
            Tok::RedirOut => {
                if let Some(Tok::Word(target)) = it.peek() {
                    if !target.starts_with("/dev/") && !target.starts_with('&') && !target.is_empty() {
                        out.push((target.clone(), FileOp::Write));
                    }
                    it.next();
                }
            }
            Tok::RedirIn => {
                if let Some(Tok::Word(_)) = it.peek() {
                    it.next();
                }
            }
        }
    }
    finish(&mut simple, &mut out);
    out.truncate(MAX_TOUCHES);
    out
}

fn command_touches(words: &[String], out: &mut Vec<(String, FileOp)>) {
    let mut i = 0;
    while i < words.len() && (words[i] == "sudo" || is_assignment(&words[i])) {
        i += 1;
    }
    let Some(cmd) = words.get(i) else { return };
    let name = cmd.rsplit('/').next().unwrap_or(cmd);
    let args = &words[i + 1..];
    let paths: Vec<&String> = args.iter().filter(|w| is_path_arg(w)).collect();
    match name {
        "cat" | "head" | "tail" | "less" | "more" | "wc" | "stat" | "file" | "source" | "." => {
            out.extend(paths.iter().map(|p| (p.to_string(), FileOp::Read)))
        }
        "touch" | "rm" | "tee" | "mv" => out.extend(paths.iter().map(|p| (p.to_string(), FileOp::Write))),
        "cp" if paths.len() >= 2 => {
            let (last, rest) = paths.split_last().unwrap();
            out.extend(rest.iter().map(|p| (p.to_string(), FileOp::Read)));
            out.push((last.to_string(), FileOp::Write));
        }
        "sed" if args.iter().any(|a| a.starts_with("-i")) => {
            if let Some(last) = paths.last() {
                out.push((last.to_string(), FileOp::Write));
            }
        }
        _ => {}
    }
}

fn is_assignment(w: &str) -> bool {
    match w.find('=') {
        Some(i) if i > 0 => w[..i].chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use FileOp::*;

    fn t(cmd: &str) -> Vec<(String, FileOp)> {
        touches(cmd)
    }

    #[test]
    fn detects_reads_and_writes() {
        assert_eq!(t("cat package.json"), vec![("package.json".into(), Read)]);
        assert_eq!(t("echo hi > /tmp/out.txt"), vec![("/tmp/out.txt".into(), Write)]);
        assert_eq!(t("echo hi >/tmp/out.txt 2>&1"), vec![("/tmp/out.txt".into(), Write)]);
        assert_eq!(t("make 2> err.log"), vec![("err.log".into(), Write)]);
        assert_eq!(t("npm test > /dev/null"), vec![]);
        assert_eq!(t("cp a.txt b/c.txt"), vec![("a.txt".into(), Read), ("b/c.txt".into(), Write)]);
        assert_eq!(t("sed -i '' 's/a/b/' src/x.ts"), vec![("src/x.ts".into(), Write)]);
        assert_eq!(t("FOO=1 sudo rm -rf build/out.js && ls"), vec![("build/out.js".into(), Write)]);
        assert_eq!(t("cd x && head -n 5 'my file.md' | wc -l"), vec![("my file.md".into(), Read)]);
        assert_eq!(t("cat *.ts $HOME/x.txt dir/"), vec![]);
        assert_eq!(t("cat <<EOF > out.txt\nhello\nEOF"), vec![]);
        assert_eq!(t("npm run build"), vec![]);
        assert_eq!(t("git status; npm test"), vec![]);
    }

    #[test]
    fn caps_touches() {
        let cmd = format!("touch {}", (0..20).map(|i| format!("f{i}.txt")).collect::<Vec<_>>().join(" "));
        assert_eq!(t(&cmd).len(), MAX_TOUCHES);
    }
}
