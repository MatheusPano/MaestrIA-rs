use serde_json::{Value, json};

use crate::json::{Obj, get_str};
use crate::text::{Time, clip_utf16, iso8601, last_segment, parse_iso8601_or, utf16_len};

pub const MAX_SAVED_TEXT: usize = 128 * 1024;

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

    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    pub source: DocSource,
    pub title: String,
    pub text: String,
    pub path: Option<String>,
    pub origin: Option<String>,
    pub at: Time,
}

impl Doc {
    pub fn file(path: &str, origin: Option<String>, now: Time) -> Doc {
        Doc {
            source: DocSource::File,
            title: last_segment(path).to_owned(),
            text: String::new(),
            path: Some(path.to_owned()),
            origin,
            at: now,
        }
    }

    pub fn to_json(&self) -> Value {
        let mut m = Obj::new();
        m.insert("source".into(), json!(self.source.name()));
        m.insert("title".into(), json!(self.title));
        if let Some(p) = &self.path {
            m.insert("path".into(), json!(p));
        }
        if let Some(o) = &self.origin {
            m.insert("origin".into(), json!(o));
        }
        m.insert("at".into(), json!(iso8601(&self.at)));
        if !self.source.on_disk() {
            m.insert("text".into(), json!(clip_saved(&self.text)));
        }
        Value::Object(m)
    }

    pub fn from_json(v: &Value, now: Time) -> Option<Doc> {
        let m = v.as_object()?;
        let source = DocSource::by_name(get_str(m, "source").typed()?.unwrap_or(""))?;
        let path = get_str(m, "path").typed()?;
        if source.on_disk() && path.is_none_or(str::is_empty) {
            return None;
        }
        let title = match get_str(m, "title").typed()? {
            Some(t) => t.to_owned(),
            None => path.map_or_else(|| source.label().to_owned(), |p| last_segment(p).to_owned()),
        };
        Some(Doc {
            source,
            title,
            text: get_str(m, "text").typed()?.unwrap_or("").to_owned(),
            path: path.map(str::to_owned),
            origin: get_str(m, "origin").typed()?.map(str::to_owned),
            at: parse_iso8601_or(get_str(m, "at").lenient(), now),
        })
    }
}

fn clip_saved(text: &str) -> String {
    if utf16_len(text) <= MAX_SAVED_TEXT {
        text.to_owned()
    } else {
        format!("{}\n\n…", clip_utf16(text, MAX_SAVED_TEXT))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setup {
    pub root: String,
    pub selected: Option<String>,
}

impl Setup {
    pub fn name(&self) -> &str {
        last_segment(&self.root)
    }

    pub fn to_json(&self) -> Value {
        let mut m = Obj::new();
        m.insert("root".into(), json!(self.root));
        if let Some(s) = &self.selected {
            m.insert("selected".into(), json!(s));
        }
        Value::Object(m)
    }

    pub fn from_json(v: &Value) -> Option<Setup> {
        let m = v.as_object()?;
        let root = get_str(m, "root").typed()??.trim();
        if root.is_empty() {
            return None;
        }
        Some(Setup {
            root: root.to_owned(),
            selected: get_str(m, "selected").typed()?.map(str::to_owned),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginViewRef {
    pub title: String,
    pub plugin_name: String,
}

pub fn is_markdown_path(path: &str) -> bool {
    let name = last_segment(path).to_lowercase();
    name.ends_with(".md") || name.ends_with(".markdown") || name.ends_with(".mdx")
}
