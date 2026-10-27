use std::collections::VecDeque;

use serde_json::Value;

pub const CONSOLE_MAX: usize = 5000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub tone: Option<String>,
    pub bold: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleLine {
    pub text: String,
    pub tone: Option<String>,
    pub spans: Option<Vec<Span>>,
}

fn tone_of(m: &serde_json::Map<String, Value>) -> Option<String> {
    m.get("tone").and_then(Value::as_str).map(str::to_owned)
}

fn shown(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(other) => crate::rpc::dart_string(other),
    }
}

impl ConsoleLine {
    pub fn plain(text: &str) -> Self {
        ConsoleLine { text: text.to_owned(), tone: None, spans: None }
    }

    pub fn parse(raw: &Value) -> Option<ConsoleLine> {
        match raw {
            Value::String(s) => Some(Self::plain(s)),
            Value::Object(m) => {
                if let Some(Value::Array(parts)) = m.get("spans") {
                    let spans: Vec<Span> = parts
                        .iter()
                        .filter_map(|s| match s {
                            Value::Object(o) if o.contains_key("text") => Some(Span {
                                text: shown(o.get("text")),
                                tone: tone_of(o),
                                bold: o.get("bold") == Some(&Value::Bool(true)),
                            }),
                            Value::String(t) => Some(Span { text: t.clone(), tone: None, bold: false }),
                            _ => None,
                        })
                        .collect();
                    let text = spans.iter().map(|s| s.text.as_str()).collect();
                    return Some(ConsoleLine { text, tone: tone_of(m), spans: Some(spans) });
                }
                if m.contains_key("text") {
                    return Some(ConsoleLine { text: shown(m.get("text")), tone: tone_of(m), spans: None });
                }
                None
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginConsole {
    pub lines: VecDeque<ConsoleLine>,
    pub max: usize,
    pub revision: u64,
}

impl Default for PluginConsole {
    fn default() -> Self {
        PluginConsole { lines: VecDeque::new(), max: CONSOLE_MAX, revision: 0 }
    }
}

impl PluginConsole {
    pub fn replace(&mut self, raw: &Value) {
        self.lines.clear();
        self.append(raw);
    }

    pub fn append(&mut self, raw: &Value) {
        match raw {
            Value::Array(items) => self.lines.extend(items.iter().filter_map(ConsoleLine::parse)),
            other => self.lines.extend(ConsoleLine::parse(other)),
        }
        while self.lines.len() > self.max {
            self.lines.pop_front();
        }
        self.revision += 1;
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.revision += 1;
    }

    pub fn texts(&self) -> Vec<&str> {
        self.lines.iter().map(|l| l.text.as_str()).collect()
    }
}
