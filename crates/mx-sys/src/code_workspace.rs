use std::path::Path;

use serde_json::Value;

use mx_core::paths::expand_home_with;
use mx_core::text::last_segment as basename;

pub const EXT: &str = ".code-workspace";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceFolder {
    pub path: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeWorkspace {
    pub path: String,
    pub name: String,
    pub folders: Vec<WorkspaceFolder>,
}

pub fn looks_like_one(path: &str) -> bool {
    path.trim().to_lowercase().ends_with(EXT)
}

pub async fn read(raw_path: &str) -> Option<CodeWorkspace> {
    read_with(raw_path, std::env::var("HOME").ok().as_deref()).await
}

pub async fn read_with(raw_path: &str, home: Option<&str>) -> Option<CodeWorkspace> {
    let path = expand_home_with(raw_path.trim(), home);
    if !Path::new(&path).exists() {
        return None;
    }
    let source = tokio::fs::read_to_string(&path).await.ok()?;
    parse_with(&source, &path, home)
}

pub fn parse(source: &str, path: &str) -> Option<CodeWorkspace> {
    parse_with(source, path, std::env::var("HOME").ok().as_deref())
}

pub fn parse_with(source: &str, path: &str, home: Option<&str>) -> Option<CodeWorkspace> {
    let json: Value = serde_json::from_str(&strip_jsonc(source)).ok()?;
    let obj = json.as_object()?;
    let from = dir_of(path);
    let mut folders = Vec::new();
    let entries = obj.get("folders").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    for entry in entries.iter().filter_map(Value::as_object) {
        let text = |k: &str| entry.get(k).and_then(Value::as_str).unwrap_or("").trim();
        let raw = text("path");
        if raw.is_empty() {
            continue;
        }
        let name = text("name");
        folders.push(WorkspaceFolder {
            path: resolve_with(raw, &from, home),
            name: (!name.is_empty()).then(|| name.to_owned()),
        });
    }
    Some(CodeWorkspace { path: path.to_owned(), name: name_of(path), folders })
}

pub fn resolve(raw: &str, from: &str) -> String {
    resolve_with(raw, from, std::env::var("HOME").ok().as_deref())
}

pub fn resolve_with(raw: &str, from: &str, home: Option<&str>) -> String {
    let expanded = expand_home_with(raw.trim(), home);
    if expanded.starts_with('/') {
        normalize(&expanded)
    } else {
        normalize(&format!("{from}/{expanded}"))
    }
}

pub fn normalize(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut out: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if out.last().is_some_and(|l| *l != "..") {
                    out.pop();
                } else if !absolute {
                    out.push("..");
                }
            }
            s => out.push(s),
        }
    }
    let joined = out.join("/");
    if absolute {
        format!("/{joined}")
    } else if joined.is_empty() {
        ".".into()
    } else {
        joined
    }
}

pub fn name_of(path: &str) -> String {
    let base = basename(path);
    let cut = base.len().checked_sub(EXT.len());
    let name = match cut.and_then(|c| Some((c, base.get(c..)?))) {
        Some((c, tail)) if tail.eq_ignore_ascii_case(EXT) => &base[..c],
        _ => base,
    };
    if name.is_empty() { base.to_owned() } else { name.to_owned() }
}

fn dir_of(path: &str) -> String {
    match path.rfind('/') {
        None => ".".into(),
        Some(0) => "/".into(),
        Some(i) => path[..i].to_owned(),
    }
}

pub fn strip_jsonc(source: &str) -> String {
    drop_trailing_commas(&drop_comments(source))
}

#[derive(Default)]
struct StringTracker {
    in_string: bool,
    escaped: bool,
}

impl StringTracker {
    fn feed_inside(&mut self, c: char) {
        if self.escaped {
            self.escaped = false;
        } else if c == '\\' {
            self.escaped = true;
        } else if c == '"' {
            self.in_string = false;
        }
    }
}

fn drop_comments(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut s = StringTracker::default();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if s.in_string {
            out.push(c);
            s.feed_inside(c);
            i += 1;
            continue;
        }
        if c == '"' {
            s.in_string = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < chars.len() {
            if chars[i + 1] == '/' {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if chars[i + 1] == '*' {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(chars.len());
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

fn drop_trailing_commas(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut s = StringTracker::default();
    for (i, &c) in chars.iter().enumerate() {
        if s.in_string {
            out.push(c);
            s.feed_inside(c);
            continue;
        }
        if c == '"' {
            s.in_string = true;
            out.push(c);
            continue;
        }
        if c == ',' {
            let next = chars[i + 1..].iter().find(|c| !matches!(c, ' ' | '\t' | '\n' | '\r'));
            if matches!(next, Some('}' | ']')) {
                continue;
            }
        }
        out.push(c);
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceImport<F> {
    pub workspace: CodeWorkspace,
    pub added: Vec<F>,
    pub already: Vec<F>,
    pub missing: Vec<String>,
}

impl<F> WorkspaceImport<F> {
    pub fn sticky(&self) -> bool {
        !self.missing.is_empty()
    }

    pub fn summary(&self) -> String {
        summary(&self.workspace.name, self.added.len(), self.already.len(), self.missing.len())
    }
}

pub fn summary(name: &str, added: usize, already: usize, missing: usize) -> String {
    let mut parts = Vec::new();
    if added > 0 {
        let what = if added == 1 { "pasta adicionada" } else { "pastas adicionadas" };
        parts.push(format!("{added} {what}"));
    }
    if already > 0 {
        let verb = if already == 1 { "estava" } else { "estavam" };
        parts.push(format!("{already} já {verb} aqui"));
    }
    if missing > 0 {
        let verb = if missing == 1 { "existe" } else { "existem" };
        parts.push(format!("{missing} não {verb} no disco"));
    }
    if parts.is_empty() {
        return format!("Workspace \"{name}\": nada pra adicionar");
    }
    format!("Workspace \"{name}\": {}", parts.join(", "))
}
