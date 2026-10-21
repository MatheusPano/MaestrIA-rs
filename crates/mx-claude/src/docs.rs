use std::path::Path;
use std::sync::OnceLock;

use chrono::{DateTime, Local, NaiveDateTime, Timelike};
use mx_core::text::{clip_utf16, utf16_len};
use percent_encoding::percent_decode_str;
use regex::Regex;
use serde_json::{Map, Value, json};

const MAX_SAVED: usize = 128 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocSource {
    Plan,
    File,
    Message,
    Report,
    Plugin,
}

impl DocSource {
    pub const ALL: [DocSource; 5] = [
        Self::Plan,
        Self::File,
        Self::Message,
        Self::Report,
        Self::Plugin,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::File => "file",
            Self::Message => "message",
            Self::Report => "report",
            Self::Plugin => "plugin",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Plan => "plano",
            Self::File => "arquivo",
            Self::Message => "recado",
            Self::Report => "relatório",
            Self::Plugin => "plugin",
        }
    }

    pub fn on_disk(self) -> bool {
        self == Self::File
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MxDoc {
    pub source: DocSource,
    pub title: String,
    pub text: String,
    pub path: Option<String>,
    pub origin: Option<String>,
    pub at: NaiveDateTime,
}

fn base_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn to_micros(at: NaiveDateTime) -> NaiveDateTime {
    at.with_nanosecond(at.nanosecond() / 1000 * 1000)
        .unwrap_or(at)
}

fn now() -> NaiveDateTime {
    to_micros(Local::now().naive_local())
}

pub fn dart_iso(at: &NaiveDateTime) -> String {
    let micros = at.nanosecond() / 1000 % 1_000_000;
    let millis = micros / 1000;
    let rest = micros % 1000;
    let mut out = format!("{}.{millis:03}", at.format("%Y-%m-%dT%H:%M:%S"));
    if rest != 0 {
        out.push_str(&format!("{rest:03}"));
    }
    out
}

fn parse_at(text: &str) -> Option<NaiveDateTime> {
    if let Ok(at) = DateTime::parse_from_rfc3339(text) {
        return Some(at.with_timezone(&Local).naive_local());
    }
    [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
    ]
    .iter()
    .find_map(|fmt| NaiveDateTime::parse_from_str(text, fmt).ok())
}

impl MxDoc {
    pub fn new(source: DocSource, title: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            source,
            title: title.into(),
            text: text.into(),
            path: None,
            origin: None,
            at: now(),
        }
    }

    pub fn file(path: &str, origin: Option<String>) -> Self {
        Self {
            path: Some(path.to_owned()),
            origin,
            ..Self::new(DocSource::File, base_name(path), "")
        }
    }

    pub fn identity(&self) -> String {
        format!(
            "{}|{}|{}",
            self.source.name(),
            self.path.as_deref().unwrap_or(""),
            self.title
        )
    }

    pub fn missing(&self) -> bool {
        self.source.on_disk() && self.path.as_deref().is_none_or(|p| !Path::new(p).is_file())
    }

    pub fn reload(&mut self) -> bool {
        let Some(path) = self.path.as_deref().filter(|_| self.source.on_disk()) else {
            return false;
        };
        let file = Path::new(path);
        if !file.is_file() {
            return false;
        }
        let Ok(read) = std::fs::read_to_string(file) else {
            return false;
        };
        if read == self.text {
            return false;
        }
        self.text = read;
        if let Ok(modified) = std::fs::metadata(file).and_then(|m| m.modified()) {
            self.at = to_micros(DateTime::<Local>::from(modified).naive_local());
        }
        true
    }

    pub fn to_json(&self) -> Value {
        let mut out = Map::new();
        out.insert("source".into(), json!(self.source.name()));
        out.insert("title".into(), json!(self.title));
        if let Some(path) = &self.path {
            out.insert("path".into(), json!(path));
        }
        if let Some(origin) = &self.origin {
            out.insert("origin".into(), json!(origin));
        }
        out.insert("at".into(), json!(dart_iso(&self.at)));
        if !self.source.on_disk() {
            let text = if utf16_len(&self.text) <= MAX_SAVED {
                self.text.clone()
            } else {
                format!("{}\n\n…", clip_utf16(&self.text, MAX_SAVED))
            };
            out.insert("text".into(), json!(text));
        }
        Value::Object(out)
    }

    pub fn from_json(json: &Value) -> Option<Self> {
        let source = DocSource::from_name(json.get("source")?.as_str()?)?;
        let field = |key: &str| json.get(key).and_then(Value::as_str).map(str::to_owned);
        let path = field("path");
        if source.on_disk() && path.as_deref().is_none_or(str::is_empty) {
            return None;
        }
        let title = field("title")
            .or_else(|| path.as_deref().map(|p| base_name(p).to_owned()))
            .unwrap_or_else(|| source.label().to_owned());
        Some(Self {
            source,
            title,
            text: field("text").unwrap_or_default(),
            at: field("at")
                .as_deref()
                .and_then(parse_at)
                .unwrap_or_else(now),
            origin: field("origin"),
            path,
        })
    }
}

pub fn is_markdown_path(path: &str) -> bool {
    let name = base_name(path).to_lowercase();
    name.ends_with(".md") || name.ends_with(".markdown") || name.ends_with(".mdx")
}

pub fn expand_home(path: &str, home: Option<&str>) -> String {
    if path != "~" && !path.starts_with("~/") {
        return path.to_owned();
    }
    let Some(home) = home.filter(|h| !h.is_empty()) else {
        return path.to_owned();
    };
    let home = home.strip_suffix('/').unwrap_or(home);
    format!("{home}{}", &path[1..])
}

pub fn resolve_link_path(path: &str, base: Option<&str>, home: Option<&str>) -> String {
    let expanded = expand_home(path, home);
    match base {
        _ if expanded.starts_with('/') => expanded,
        Some(base) if !base.is_empty() => format!("{base}/{expanded}"),
        _ => expanded,
    }
}

fn scheme_of(href: &str) -> Option<&str> {
    static SCHEME: OnceLock<Regex> = OnceLock::new();
    SCHEME
        .get_or_init(|| Regex::new(r"^([A-Za-z][A-Za-z0-9+.-]*):").unwrap())
        .captures(href)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
}

pub fn decode_href(href: &str) -> String {
    if scheme_of(href).is_some() {
        return href.to_owned();
    }
    percent_decode_str(href)
        .decode_utf8()
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| href.to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkAction {
    Nothing,
    OpenExternal(String),
    Missing(String),
    Read(String),
    QuickLook(String),
}

impl LinkAction {
    pub fn problem(&self) -> Option<String> {
        match self {
            Self::Missing(target) => Some(format!(
                "Esse link aponta pra um arquivo que não existe: {target}"
            )),
            _ => None,
        }
    }
}

pub fn follow_link(href: &str, base: Option<&str>, home: Option<&str>) -> LinkAction {
    if href.trim().is_empty() {
        return LinkAction::Nothing;
    }
    if let Some(scheme) = scheme_of(href)
        && matches!(scheme.to_lowercase().as_str(), "http" | "https" | "mailto")
    {
        return LinkAction::OpenExternal(href.to_owned());
    }
    let path = href.split('#').next().unwrap_or("");
    if path.is_empty() {
        return LinkAction::Nothing;
    }
    let target = resolve_link_path(path, base, home);
    if !Path::new(&target).is_file() {
        return LinkAction::Missing(target);
    }
    if is_markdown_path(&target) {
        LinkAction::Read(target)
    } else {
        LinkAction::QuickLook(target)
    }
}
