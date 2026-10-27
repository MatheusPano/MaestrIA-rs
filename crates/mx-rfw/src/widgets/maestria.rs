use crate::decoders::{Color, EdgeInsets, color, edge_insets};
use crate::tree::{Child, Event, Node, Widget};
use crate::value::{Map, Value};

fn s(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

pub struct Svg {
    pub svg: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
}

impl Svg {
    pub fn from_node(n: &Node) -> Self {
        Self { svg: s(n.arg("svg")), width: n.arg("width").as_double(), height: n.arg("height").as_double() }
    }
}

pub struct Glyph {
    pub icon: Option<String>,
    pub size: f64,
    pub color: Option<Color>,
}

impl Glyph {
    pub fn from_node(n: &Node) -> Self {
        Self {
            icon: s(n.arg("icon")),
            size: n.arg("size").as_double().unwrap_or(16.0),
            color: color(n.arg("color")),
        }
    }
}

pub struct IconButton<'a> {
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub size: f64,
    pub extent: f64,
    pub color: Option<Color>,
    pub on_pressed: Vec<&'a Event>,
}

impl<'a> IconButton<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        let disabled = n.arg("disabled").as_bool() == Some(true);
        Self {
            icon: s(n.arg("icon")),
            tooltip: s(n.arg("tooltip")),
            size: n.arg("size").as_double().unwrap_or(16.0),
            extent: n.arg("extent").as_double().unwrap_or(26.0),
            color: color(n.arg("color")),
            on_pressed: if disabled { Vec::new() } else { n.handlers("onPressed") },
        }
    }

    pub fn enabled(&self) -> bool {
        !self.on_pressed.is_empty()
    }
}

pub struct Tooltip<'a> {
    pub message: String,
    pub child: Child<'a>,
}

impl<'a> Tooltip<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self { message: s(n.arg("message")).unwrap_or_default(), child: n.child("child") }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubMenuItem {
    pub action: String,
    pub label: String,
    pub color: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    pub action: Option<String>,
    pub label: String,
    pub icon: Option<String>,
    pub red: bool,
    pub disabled: bool,
    pub children: Vec<SubMenuItem>,
}

impl MenuItem {
    fn from_value(v: &Value) -> Self {
        let flag = |k: &str| v.get(k).as_bool() == Some(true);
        let children = v.get("children").as_list().unwrap_or_default();
        Self {
            action: if flag("divider") { None } else { s(v.get("action")) },
            label: s(v.get("label")).unwrap_or_default(),
            icon: s(v.get("icon")),
            red: flag("red"),
            disabled: flag("disabled"),
            children: children
                .iter()
                .map(|c| SubMenuItem {
                    action: s(c.get("action")).unwrap_or_default(),
                    label: s(c.get("label")).unwrap_or_default(),
                    color: c.get("color").as_int().unwrap_or(0),
                })
                .collect(),
        }
    }

    pub fn is_divider(&self) -> bool {
        self.action.is_none()
    }

    pub fn shown_label(&self) -> &str {
        if self.label.is_empty() { self.action.as_deref().unwrap_or("") } else { &self.label }
    }
}

pub fn menu_extra(choice: &str) -> Map {
    Map::from_iter([("action".to_owned(), Value::from(choice))])
}

pub struct Pressable<'a> {
    pub button: bool,
    pub on_tap: Vec<&'a Event>,
    pub on_double_tap: Vec<&'a Event>,
    pub color: Option<Color>,
    pub hover_color: Option<Color>,
    pub radius: f64,
    pub padding: Option<EdgeInsets>,
    pub child: Option<&'a Widget>,
    pub menu: Vec<MenuItem>,
    pub on_menu: Vec<&'a Event>,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub size: f64,
}

impl<'a> Pressable<'a> {
    pub fn from_node(n: &'a Node, button: bool) -> Self {
        Self {
            button,
            on_tap: n.handlers("onTap"),
            on_double_tap: n.handlers("onDoubleTap"),
            color: color(n.arg("color")),
            hover_color: color(n.arg("hoverColor")),
            radius: n.arg("radius").as_double().unwrap_or(0.0),
            padding: edge_insets(n.arg("padding")),
            child: if button { None } else { n.optional_child("child") },
            menu: n.arg("menu").as_list().unwrap_or_default().iter().map(MenuItem::from_value).collect(),
            on_menu: n.handlers("onMenu"),
            icon: s(n.arg("icon")),
            tooltip: s(n.arg("tooltip")),
            size: n.arg("size").as_double().unwrap_or(16.0),
        }
    }
}

pub struct Draggable<'a> {
    pub payload: String,
    pub targets: Vec<String>,
    pub width: Option<f64>,
    pub disabled: bool,
    pub child: Option<&'a Widget>,
    pub feedback: Option<&'a Widget>,
}

impl<'a> Draggable<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            payload: s(n.arg("payload")).unwrap_or_default(),
            targets: n.arg("targets").as_list().unwrap_or_default().iter().filter_map(s).collect(),
            width: n.arg("width").as_double(),
            disabled: n.arg("disabled").as_bool() == Some(true),
            child: n.optional_child("child"),
            feedback: n.optional_child("feedback"),
        }
    }

    pub fn draggable(&self) -> bool {
        !self.disabled && !self.payload.is_empty()
    }

    pub fn fits(&self, target_id: &str) -> bool {
        self.targets.is_empty() || self.targets.iter().any(|t| t == target_id)
    }
}

pub struct DropTarget<'a> {
    pub id: String,
    pub hint: Option<String>,
    pub color: Option<Color>,
    pub hover_color: Option<Color>,
    pub radius: f64,
    pub child: Option<&'a Widget>,
    pub on_drop: Vec<&'a Event>,
}

impl<'a> DropTarget<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            id: s(n.arg("id")).unwrap_or_default(),
            hint: s(n.arg("hint")),
            color: color(n.arg("color")),
            hover_color: color(n.arg("hoverColor")),
            radius: n.arg("radius").as_double().unwrap_or(0.0),
            child: n.optional_child("child"),
            on_drop: n.handlers("onDrop"),
        }
    }

    pub fn drop_extra(&self, payload: &str) -> Map {
        Map::from_iter([("payload".to_owned(), Value::from(payload)), ("to".to_owned(), Value::from(self.id.as_str()))])
    }
}

pub const MARKDOWN_BASE_SIZE: f64 = 13.5;

pub struct Markdown {
    pub text: String,
    pub font_size: f64,
}

impl Markdown {
    pub fn from_node(n: &Node) -> Self {
        Self {
            text: s(n.arg("text")).unwrap_or_default(),
            font_size: n.arg("fontSize").as_double().unwrap_or(MARKDOWN_BASE_SIZE),
        }
    }
}

pub struct Anchor<'a> {
    pub id: String,
    pub child: Child<'a>,
}

impl<'a> Anchor<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self { id: s(n.arg("id")).unwrap_or_default(), child: n.child("child") }
    }

    pub fn key(&self, plugin_id: &str) -> String {
        anchor_key(plugin_id, &self.id)
    }
}

pub fn anchor_key(plugin_id: &str, id: &str) -> String {
    format!("{plugin_id}/{id}")
}

pub struct SelectionArea<'a> {
    pub lines_only: bool,
    pub child: Child<'a>,
}

impl<'a> SelectionArea<'a> {
    pub fn from_node(n: &'a Node, lines_only: bool) -> Self {
        Self { lines_only, child: n.child("child") }
    }
}
