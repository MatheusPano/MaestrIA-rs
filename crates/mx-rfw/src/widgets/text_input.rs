use crate::tree::{Event, Node};
use crate::value::{Map, Value};

pub const SHOWN_COMMANDS: usize = 8;
pub const SEND_TOOLTIP: &str = "enviar (enter)";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
}

pub struct TextInput<'a> {
    pub id: String,
    pub placeholder: Option<String>,
    pub value: Option<String>,
    pub hint: Option<String>,
    pub multiline: bool,
    pub max_lines: i64,
    pub autofocus: bool,
    pub disabled: bool,
    pub clear: Option<i64>,
    pub send_icon: Option<String>,
    pub font_size: f64,
    pub allow_empty: bool,
    pub commands: Vec<SlashCommand>,
    pub on_submit: Vec<&'a Event>,
    pub on_paste_image: Vec<&'a Event>,
    pub on_arrow: Vec<&'a Event>,
}

pub fn rounded(v: &Value) -> Option<i64> {
    v.as_int().or_else(|| v.as_double().map(|d| d.round() as i64))
}

pub fn number(v: &Value) -> Option<f64> {
    v.as_double().or_else(|| v.as_int().map(|i| i as f64))
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

fn commands(v: &Value) -> Vec<SlashCommand> {
    v.as_list()
        .unwrap_or_default()
        .iter()
        .filter_map(|c| {
            let name = c.get("name").as_str()?.trim();
            if name.is_empty() {
                return None;
            }
            Some(SlashCommand {
                name: name.strip_prefix('/').unwrap_or(name).to_owned(),
                description: text(c.get("description")).unwrap_or_default(),
            })
        })
        .collect()
}

impl<'a> TextInput<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        let flag = |k: &str| n.arg(k).as_bool() == Some(true);
        Self {
            id: text(n.arg("id")).unwrap_or_default(),
            placeholder: text(n.arg("placeholder")),
            value: text(n.arg("value")),
            hint: text(n.arg("hint")),
            multiline: flag("multiline"),
            max_lines: rounded(n.arg("maxLines")).unwrap_or(6),
            autofocus: flag("autofocus"),
            disabled: flag("disabled"),
            clear: rounded(n.arg("clear")),
            send_icon: text(n.arg("sendIcon")),
            font_size: n.arg("fontSize").as_double().unwrap_or(13.0),
            allow_empty: flag("allowEmpty"),
            commands: commands(n.arg("commands")),
            on_submit: n.handlers("onSubmit"),
            on_paste_image: n.handlers("onPasteImage"),
            on_arrow: n.handlers("onArrow"),
        }
    }

    pub fn lines(&self) -> i64 {
        if self.multiline { self.max_lines.max(1) } else { 1 }
    }

    pub fn shown_hint(&self) -> Option<&str> {
        if self.disabled { self.hint.as_deref().or(self.placeholder.as_deref()) } else { self.placeholder.as_deref() }
    }

    pub fn submitted(&self, field: &str) -> Option<Map> {
        if self.disabled {
            return None;
        }
        let text = field.trim();
        if text.is_empty() && !self.allow_empty {
            return None;
        }
        Some(Map::from_iter([("text".to_owned(), Value::from(text)), ("id".to_owned(), Value::from(self.id.as_str()))]))
    }

    pub fn arrow(&self, key: &str, field: &str) -> Option<Map> {
        if self.on_arrow.is_empty() || self.disabled || !field.is_empty() || !matches!(key, "up" | "down") {
            return None;
        }
        Some(Map::from_iter([("key".to_owned(), Value::from(key)), ("id".to_owned(), Value::from(self.id.as_str()))]))
    }

    pub fn image_extra(&self) -> Map {
        Map::from_iter([("id".to_owned(), Value::from(self.id.as_str()))])
    }

    pub fn matches(&self, field: &str, dismissed: Option<&str>) -> Vec<&SlashCommand> {
        if self.commands.is_empty() || self.disabled || dismissed == Some(field) {
            return Vec::new();
        }
        let Some(rest) = field.strip_prefix('/') else { return Vec::new() };
        if rest.chars().any(char::is_whitespace) {
            return Vec::new();
        }
        let q = rest.to_lowercase();
        let (starts, inside): (Vec<_>, Vec<_>) = self
            .commands
            .iter()
            .filter(|c| c.name.to_lowercase().contains(&q))
            .partition(|c| c.name.to_lowercase().starts_with(&q));
        starts.into_iter().chain(inside).take(SHOWN_COMMANDS).collect()
    }
}

pub fn completed(c: &SlashCommand) -> String {
    format!("/{} ", c.name)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClearSeen {
    seen: std::collections::HashMap<String, Option<i64>>,
}

impl ClearSeen {
    pub fn should_clear(&mut self, id: &str, clear: Option<i64>) -> bool {
        let empty = self.seen.get(id).is_some_and(|before| *before != clear);
        self.seen.insert(id.to_owned(), clear);
        empty
    }
}
