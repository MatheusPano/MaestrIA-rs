use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::json::Obj;
use crate::model::HookState;
use crate::model::hook_state::MAX_TOUCHED;
use crate::model::is_markdown_path;
use crate::paths::expand_home_with;

static SEGMENTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[|;\n]|&&").expect("valid regex"));
static REDIRECT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?-u:\d)?>>?\s*(?:'([^']+)'|"([^"]+)"|([^\s'"|;&<>()]+))"#).expect("valid regex")
});
static HEREDOC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<<-?\s*(?:'([^']+)'|"([^"]+)"|((?-u:\w)+))"#).expect("valid regex")
});
static ASSIGNMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?-u:\w)+=").expect("valid regex"));
static WORD_GAP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").expect("valid regex"));

pub(super) fn touch(st: &mut HookState, p: &Obj, home: Option<&str>) {
    let tool = p.get("tool_name").and_then(Value::as_str).unwrap_or("");
    let Some(input) = p.get("tool_input").and_then(Value::as_object) else {
        return;
    };
    let cwd = p.get("cwd").and_then(Value::as_str);
    if tool == "Bash" {
        let Some(command) = input.get("command").and_then(Value::as_str) else {
            return;
        };
        for found in shell_markdown(command) {
            remember(st, absolute(&found, cwd, home));
        }
        return;
    }
    let key = match tool {
        "Write" | "Edit" | "MultiEdit" => "file_path",
        "NotebookEdit" => "notebook_path",
        _ => return,
    };
    let Some(raw) = input
        .get(key)
        .and_then(Value::as_str)
        .filter(|r| !r.is_empty())
    else {
        return;
    };
    remember(st, absolute(raw, cwd, home));
}

fn remember(st: &mut HookState, path: String) {
    if st.touched.contains(&path) {
        return;
    }
    st.touched.push(path);
    if st.touched.len() > MAX_TOUCHED {
        st.touched.remove(0);
    }
}

pub fn absolute(path: &str, cwd: Option<&str>, home: Option<&str>) -> String {
    if path.starts_with('/') {
        return path.to_owned();
    }
    if path == "~" || path.starts_with("~/") {
        return expand_home_with(path, home);
    }
    let Some(cwd) = cwd.filter(|c| !c.is_empty()) else {
        return path.to_owned();
    };
    let rel = path.strip_prefix("./").unwrap_or(path);
    let base = cwd.strip_suffix('/').unwrap_or(cwd);
    format!("{base}/{rel}")
}

pub fn shell_markdown(command: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut keep = |token: Option<&str>| {
        let Some(path) = plain_path(token) else {
            return;
        };
        if is_markdown_path(&path) && !found.contains(&path) {
            found.push(path);
        }
    };
    let kept = commands(command);
    for segment in SEGMENTS.split(&kept) {
        for c in REDIRECT.captures_iter(segment) {
            keep(c.get(1).or(c.get(2)).or(c.get(3)).map(|m| m.as_str()));
        }
        let words: Vec<&str> = WORD_GAP.split(segment.trim()).collect();
        let Some(head) = words.iter().position(|w| !ASSIGNMENT.is_match(w)) else {
            continue;
        };
        let verb = crate::text::last_segment(words[head]);
        let args = &words[head + 1..];
        let flags: Vec<&str> = args
            .iter()
            .copied()
            .filter(|w| w.starts_with('-'))
            .collect();
        let operands: Vec<&str> = args
            .iter()
            .copied()
            .filter(|w| !w.starts_with('-'))
            .collect();
        match verb {
            "tee" => operands.iter().for_each(|o| keep(Some(o))),
            "sed"
                if flags
                    .iter()
                    .any(|f| f.starts_with("-i") || *f == "--in-place") =>
            {
                operands.iter().for_each(|o| keep(Some(o)))
            }
            "cp" | "mv" | "install" => keep(operands.last().copied()),
            _ => {}
        }
    }
    found
}

fn commands(command: &str) -> String {
    let mut lines = Vec::new();
    let mut delimiter: Option<String> = None;
    for line in command.split('\n') {
        if let Some(d) = &delimiter {
            if line.trim() == d {
                delimiter = None;
            }
            continue;
        }
        lines.push(line);
        if let Some(c) = HEREDOC.captures(line) {
            delimiter = c
                .get(1)
                .or(c.get(2))
                .or(c.get(3))
                .map(|m| m.as_str().to_owned());
        }
    }
    lines.join("\n")
}

fn plain_path(token: Option<&str>) -> Option<String> {
    let mut path = token?.trim();
    if path.len() >= 2 && (path.starts_with('\'') || path.starts_with('"')) {
        let quote = &path[..1];
        if path.ends_with(quote) {
            path = &path[1..path.len() - 1];
        }
    }
    if path.is_empty() || path.contains(['$', '*', '?', '`']) {
        return None;
    }
    Some(path.to_owned())
}
