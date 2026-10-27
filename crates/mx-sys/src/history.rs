use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Datelike, Local, NaiveDate};
use mx_core::text::{clip_utf16, utf16_len};
use serde_json::Value;
use tokio::io::AsyncReadExt;

use mx_core::text::last_segment as basename;

pub const HEAD_BYTES: u64 = 32 * 1024;
pub const LIMIT_DEFAULT: usize = 40;
const MAX_TITLE: usize = 72;
const MAX_LABEL: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatEntry {
    pub session_id: String,
    pub cwd: String,
    pub title: String,
    pub at: SystemTime,
    pub bytes: u64,
    pub missing: bool,
}

impl ChatEntry {
    pub fn where_(&self) -> &str {
        if self.cwd.is_empty() { "?" } else { basename(&self.cwd) }
    }

    pub fn label(&self) -> String {
        if utf16_len(&self.title) <= MAX_LABEL {
            return self.title.clone();
        }
        format!("{}…", clip_utf16(&self.title, MAX_LABEL))
    }

    pub fn ago(&self, now: SystemTime) -> String {
        let since = now.duration_since(self.at).unwrap_or(Duration::ZERO).as_secs();
        let (minutes, hours, days) = (since / 60, since / 3600, since / 86_400);
        if minutes < 1 {
            return "agora".into();
        }
        if minutes < 60 {
            return format!("há {minutes} min");
        }
        if hours < 24 {
            return format!("há {hours} h");
        }
        if days == 1 {
            return "ontem".into();
        }
        if days < 7 {
            return format!("há {days} dias");
        }
        let at: DateTime<Local> = self.at.into();
        format!("{}/{}", at.day(), at.month())
    }

    pub fn size(&self) -> String {
        const KIB: u64 = 1024;
        const MIB: u64 = 1024 * 1024;
        if self.bytes < KIB {
            return format!("{} B", self.bytes);
        }
        if self.bytes < MIB {
            return format!("{} kB", (self.bytes as f64 / KIB as f64).round() as u64);
        }
        let tenths = (self.bytes as f64 / MIB as f64 * 10.0).round() / 10.0;
        format!("{tenths:.1} MB")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatStanding {
    Fresh,
    OnScreen,
    Live,
    Background,
    Gone,
}

impl ChatStanding {
    pub fn note(self) -> Option<&'static str> {
        match self {
            ChatStanding::Fresh => None,
            ChatStanding::OnScreen => Some("já está num painel"),
            ChatStanding::Live => Some("aberta fora daqui"),
            ChatStanding::Background => Some("em segundo plano · anexar"),
            ChatStanding::Gone => Some("a pasta não existe mais"),
        }
    }

    pub fn opens(self) -> bool {
        matches!(self, ChatStanding::Fresh | ChatStanding::Background)
    }
}

pub fn home_root(home: &str) -> PathBuf {
    PathBuf::from(format!("{home}/.claude/projects"))
}

pub fn dir_for(cwd: &str) -> String {
    let mut out = String::with_capacity(cwd.len());
    for c in cwd.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else {
            for _ in 0..c.len_utf16() {
                out.push('-');
            }
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct ReadOptions {
    pub cwds: Option<Vec<String>>,
    pub on: Option<NaiveDate>,
    pub limit: usize,
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self { cwds: None, on: None, limit: LIMIT_DEFAULT }
    }
}

struct Found {
    file: PathBuf,
    cwd: Option<String>,
    modified: SystemTime,
    size: u64,
}

pub async fn read(root: &Path, opts: &ReadOptions) -> Vec<ChatEntry> {
    let mut places: Vec<(PathBuf, Option<String>)> = Vec::new();
    match &opts.cwds {
        None => {
            let Ok(mut dir) = tokio::fs::read_dir(root).await else {
                return Vec::new();
            };
            while let Ok(Some(entry)) = dir.next_entry().await {
                if is_dir(&entry.path()).await {
                    places.push((entry.path(), None));
                }
            }
        }
        Some(cwds) => {
            for cwd in cwds.iter().filter(|c| !c.is_empty()) {
                places.push((root.join(dir_for(cwd)), Some(cwd.clone())));
            }
        }
    }

    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (dir, cwd) in places {
        if !seen.insert(dir.clone()) {
            continue;
        }
        let Ok(mut list) = tokio::fs::read_dir(&dir).await else {
            continue;
        };
        while let Ok(Some(entry)) = list.next_entry().await {
            let path = entry.path();
            if !path.to_string_lossy().ends_with(".jsonl") {
                continue;
            }
            let Ok(meta) = tokio::fs::metadata(&path).await else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            found.push(Found { file: path, cwd: cwd.clone(), modified, size: meta.len() });
        }
    }
    found.sort_by(|a, b| b.modified.cmp(&a.modified));
    if let Some(day) = opts.on {
        found.retain(|f| DateTime::<Local>::from(f.modified).date_naive() == day);
    }

    let mut chats = Vec::new();
    for one in found.into_iter().take(opts.limit) {
        chats.push(entry_of(one).await);
    }
    chats
}

async fn is_dir(path: &Path) -> bool {
    tokio::fs::metadata(path).await.is_ok_and(|m| m.is_dir())
}

async fn read_head(path: &Path) -> String {
    let Ok(file) = tokio::fs::File::open(path).await else {
        return String::new();
    };
    let mut buf = Vec::new();
    if file.take(HEAD_BYTES).read_to_end(&mut buf).await.is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&buf).into_owned()
}

async fn entry_of(found: Found) -> ChatEntry {
    let name = found.file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let id = name.strip_suffix(".jsonl").unwrap_or(&name).to_owned();
    let head = read_head(&found.file).await;
    let head = scan_head(&head);

    let cwd = found.cwd.or(head.inside).unwrap_or_default();
    let title = clean(head.titled.as_deref())
        .or_else(|| clean(head.spoke.as_deref()))
        .or_else(|| clean(head.named.as_deref()))
        .unwrap_or_else(|| id.split('-').next().unwrap_or_default().to_owned());
    let missing = cwd.is_empty() || !is_dir(Path::new(&cwd)).await;
    ChatEntry { session_id: id, cwd, title, at: found.modified, bytes: found.size, missing }
}

#[derive(Default)]
struct Head {
    titled: Option<String>,
    named: Option<String>,
    spoke: Option<String>,
    inside: Option<String>,
}

fn scan_head(head: &str) -> Head {
    let mut h = Head::default();
    for line in head.split('\n') {
        if !line.starts_with('{') {
            continue;
        }
        let Ok(Value::Object(event)) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let text = |k: &str| event.get(k).and_then(Value::as_str).map(str::to_owned);
        if h.inside.is_none() {
            h.inside = text("cwd").filter(|c| !c.is_empty());
        }
        match event.get("type").and_then(Value::as_str) {
            Some("ai-title") if h.titled.is_none() => h.titled = text("aiTitle"),
            Some("custom-title") if h.named.is_none() => h.named = text("customTitle"),
            Some("agent-name") if h.named.is_none() => h.named = text("agentName"),
            Some("user") if h.spoke.is_none() => h.spoke = said_in(&event),
            _ => {}
        }
    }
    h
}

fn said_in(event: &serde_json::Map<String, Value>) -> Option<String> {
    if event.get("isSidechain") == Some(&Value::Bool(true)) {
        return None;
    }
    match event.get("message")?.as_object()?.get("content")? {
        Value::String(s) => Some(s.clone()),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(Value::as_object)
            .find(|b| b.get("type").and_then(Value::as_str) == Some("text"))
            .and_then(|b| b.get("text")?.as_str().map(str::to_owned)),
        _ => None,
    }
}

pub fn clean(text: Option<&str>) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for line in text?.split('\n') {
        let bare = line.trim();
        let one = match bare.trim_start_matches('#') {
            rest if rest.len() < bare.len() => rest.trim_start(),
            _ => bare,
        };
        if one.is_empty() || one.starts_with('<') {
            continue;
        }
        parts.push(one);
        if !bare.starts_with('#') {
            break;
        }
    }
    let title = parts.join(" ");
    if title.is_empty() {
        return None;
    }
    if utf16_len(&title) <= MAX_TITLE {
        return Some(title);
    }
    Some(format!("{}…", clip_utf16(&title, MAX_TITLE)))
}
