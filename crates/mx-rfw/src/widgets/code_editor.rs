use crate::tree::{Event, Node};
use crate::value::Value;

pub const DEFAULT_FONT_SIZE: f64 = 13.0;

pub struct CodeEditor<'a> {
    pub doc: String,
    pub text: Option<String>,
    pub revision: i64,
    pub language: Option<String>,
    pub line: Option<i64>,
    pub column: Option<i64>,
    pub jump: i64,
    pub read_only: bool,
    pub word_wrap: bool,
    pub font_size: f64,
    pub line_numbers: bool,
    pub autofocus: bool,
    pub hint: Option<String>,
    pub on_change: Vec<&'a Event>,
    pub on_save: Vec<&'a Event>,
    pub on_cursor: Vec<&'a Event>,
}

fn int(v: &Value) -> Option<i64> {
    v.as_int().or_else(|| v.as_double().filter(|d| d.fract() == 0.0).map(|d| d as i64))
}

fn number(v: &Value) -> Option<f64> {
    v.as_double().or_else(|| v.as_int().map(|i| i as f64))
}

impl<'a> CodeEditor<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            doc: n.arg("doc").as_str().unwrap_or_default().to_owned(),
            text: n.arg("text").as_str().map(str::to_owned),
            revision: int(n.arg("revision")).unwrap_or(0),
            language: n.arg("language").as_str().map(str::to_owned),
            line: int(n.arg("line")).filter(|l| *l >= 1),
            column: int(n.arg("column")).filter(|c| *c >= 1),
            jump: int(n.arg("jump")).unwrap_or(0),
            read_only: n.arg("readOnly").as_bool() == Some(true),
            word_wrap: n.arg("wordWrap").as_bool() == Some(true),
            font_size: number(n.arg("fontSize")).filter(|s| *s > 0.0).unwrap_or(DEFAULT_FONT_SIZE),
            line_numbers: n.arg("lineNumbers").as_bool() != Some(false),
            autofocus: n.arg("autofocus").as_bool() != Some(false),
            hint: n.arg("hint").as_str().map(str::to_owned),
            on_change: n.handlers("onChange"),
            on_save: n.handlers("onSave"),
            on_cursor: n.handlers("onCursor"),
        }
    }
}
