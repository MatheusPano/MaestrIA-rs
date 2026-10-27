use std::fmt;

use crate::value::{MISSING, Map, Value, map_to_json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalLibrary {
    CoreWidgets,
    CoreMaterial,
    Maestria,
}

impl LocalLibrary {
    pub fn name(self) -> &'static str {
        match self {
            LocalLibrary::CoreWidgets => "core.widgets",
            LocalLibrary::CoreMaterial => "core.material",
            LocalLibrary::Maestria => "maestria",
        }
    }

    pub fn from_import(dotted: &str) -> Option<Self> {
        match dotted {
            "core.widgets" => Some(LocalLibrary::CoreWidgets),
            "core.material" => Some(LocalLibrary::CoreMaterial),
            "maestria" => Some(LocalLibrary::Maestria),
            _ => None,
        }
    }

    pub fn widgets(self) -> &'static [&'static str] {
        match self {
            LocalLibrary::CoreWidgets => &[
                "AnimationDefaults", "Align", "AspectRatio", "Center", "ClipRRect", "ColoredBox", "Column",
                "Container", "DefaultTextStyle", "Directionality", "Expanded", "Flexible", "FittedBox",
                "FractionallySizedBox", "GestureDetector", "GridView", "Icon", "IconTheme", "IntrinsicHeight",
                "IntrinsicWidth", "Image", "ListBody", "ListView", "Opacity", "Padding", "Placeholder",
                "Positioned", "Rotation", "Row", "SafeArea", "Scale", "SingleChildScrollView", "SizedBox",
                "SizedBoxExpand", "SizedBoxShrink", "Spacer", "Stack", "Text", "Wrap",
            ],
            LocalLibrary::CoreMaterial => &[
                "AboutListTile", "AppBar", "ButtonBar", "OverflowBar", "Card", "CircularProgressIndicator",
                "Divider", "Drawer", "DrawerHeader", "DropdownButton", "ElevatedButton", "FloatingActionButton",
                "InkResponse", "InkWell", "LinearProgressIndicator", "ListTile", "Material", "OutlinedButton",
                "Scaffold", "Slider", "TextButton", "VerticalDivider",
            ],
            LocalLibrary::Maestria => &[
                "Svg", "Glyph", "IconButton", "Tooltip", "Pressable", "MenuButton", "Draggable", "DropTarget",
                "CodeEditor", "Sprite", "ZoomView", "TextInput", "Anchor", "Selectable", "SelectionLines", "Markdown",
            ],
        }
    }

    pub fn has(self, widget: &str) -> bool {
        self.widgets().contains(&widget)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub library: LocalLibrary,
    pub name: String,
    pub args: Map,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Widget {
    Node(Node),
    Error(String),
}

pub type Child<'a> = Result<&'a Widget, String>;

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub name: String,
    pub args: Map,
}

impl Event {
    pub fn values(&self, extra: &Map) -> serde_json::Map<String, serde_json::Value> {
        let mut merged = self.args.clone();
        for (k, v) in extra {
            merged.insert(k.clone(), v.clone());
        }
        map_to_json(&merged)
    }
}

impl fmt::Display for Widget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Widget::Node(n) => f.write_str(&n.full_name()),
            Widget::Error(e) => write!(f, "ErrorWidget({e})"),
        }
    }
}

impl Widget {
    pub fn as_node(&self) -> Option<&Node> {
        if let Widget::Node(n) = self { Some(n) } else { None }
    }

    pub fn walk<'a>(&'a self, visit: &mut impl FnMut(&'a Widget)) {
        visit(self);
        if let Widget::Node(n) = self {
            for v in n.args.values() {
                walk_value(v, visit);
            }
        }
    }
}

fn walk_value<'a>(v: &'a Value, visit: &mut impl FnMut(&'a Widget)) {
    match v {
        Value::Widget(w) => w.walk(visit),
        Value::List(l) => l.iter().for_each(|v| walk_value(v, visit)),
        Value::Map(m) => m.values().for_each(|v| walk_value(v, visit)),
        _ => {}
    }
}

impl Node {
    pub fn full_name(&self) -> String {
        format!("{}:{}", self.library.name(), self.name)
    }

    pub fn arg(&self, key: &str) -> &Value {
        self.args.get(key).unwrap_or(&MISSING)
    }

    fn not_a_widget(&self, key: &str, got: &Value) -> String {
        format!("Not a widget at [{key}] (got {got}) for {}.", self.full_name())
    }

    pub fn child(&self, key: &str) -> Child<'_> {
        match self.arg(key) {
            Value::Widget(w) => Ok(w),
            other => Err(self.not_a_widget(key, other)),
        }
    }

    pub fn optional_child(&self, key: &str) -> Option<&Widget> {
        self.arg(key).as_widget()
    }

    pub fn child_list(&self, key: &str) -> Vec<Child<'_>> {
        match self.arg(key) {
            Value::List(items) => items
                .iter()
                .map(|v| v.as_widget().ok_or_else(|| self.not_a_widget(key, v)))
                .collect(),
            Value::Missing => Vec::new(),
            other => vec![Err(format!("Not a widget list at [{key}] (got {other}) for {}.", self.full_name()))],
        }
    }

    pub fn indexed_children(&self, key: &str) -> Vec<Child<'_>> {
        let list = self.arg(key).as_list().unwrap_or_default();
        list.iter()
            .enumerate()
            .map(|(i, v)| {
                v.as_widget().ok_or_else(|| {
                    format!("Not a widget at [{key}, {i}] (got {v}) for {}.", self.full_name())
                })
            })
            .collect()
    }

    pub fn handlers(&self, key: &str) -> Vec<&Event> {
        handlers(self.arg(key))
    }
}

pub fn handlers(v: &Value) -> Vec<&Event> {
    match v {
        Value::Event(e) => vec![e],
        Value::List(l) => l.iter().filter_map(Value::as_event).collect(),
        _ => Vec::new(),
    }
}
