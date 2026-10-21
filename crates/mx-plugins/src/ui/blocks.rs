use mx_core::json::Obj;
use serde_json::Value;

pub use super::menu::{Avatar, MenuEntry, menu_of};
use super::icons::material_icon;
use super::view::children;

pub const UNKNOWN_BLOCK: &str = "Bloco desconhecido: ";
pub const LIST_EMPTY: &str = "Nada aqui";
pub const CONSOLE_EMPTY: &str = "Nada no console ainda";
pub const TERMINAL_EMPTY: &str = "O terminal fechou";
pub const ROW_CHILD_WIDTH: f64 = 240.0;
pub const CONSOLE_HEIGHT: f64 = 320.0;
pub const TERMINAL_HEIGHT: f64 = 360.0;

pub(crate) fn s(b: &Obj, key: &str) -> Option<String> {
    b.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn text(b: &Obj, key: &str) -> String {
    s(b, key).unwrap_or_default()
}

fn on(b: &Obj, key: &str) -> bool {
    b.get(key) == Some(&Value::Bool(true))
}

fn num(b: &Obj, key: &str) -> Option<f64> {
    b.get(key).and_then(Value::as_f64)
}

fn shown(b: &Obj, key: &str) -> Option<String> {
    b.get(key).filter(|v| !v.is_null()).map(crate::rpc::dart_string)
}

fn objs<'a>(b: &'a Obj, key: &str) -> Vec<&'a Obj> {
    match b.get(key) {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_object).collect(),
        _ => Vec::new(),
    }
}

pub fn action_of(b: &Obj) -> Option<String> {
    match b.get("action").filter(|v| !v.is_null()).or_else(|| b.get("id")) {
        Some(Value::String(a)) => Some(a.clone()),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Button {
    pub action: Option<String>,
    pub label: Option<String>,
    pub style: Option<String>,
    pub icon: Option<String>,
    pub tone: Option<String>,
    pub tooltip: Option<String>,
    pub disabled: bool,
    pub menu: Vec<MenuEntry>,
}

impl Button {
    pub fn parse(b: &Obj) -> Button {
        Button {
            action: action_of(b),
            label: s(b, "label"),
            style: s(b, "style"),
            icon: s(b, "icon"),
            tone: s(b, "tone"),
            tooltip: s(b, "tooltip"),
            disabled: on(b, "disabled"),
            menu: menu_of(&objs(b, "menu")),
        }
    }

    pub fn icon_only(&self) -> bool {
        self.icon.as_deref().and_then(material_icon).is_some()
            && (self.style.as_deref() == Some("icon") || self.label.is_none())
    }

    pub fn shown_label(&self) -> String {
        self.label.clone().or_else(|| self.action.clone()).unwrap_or_else(|| "ok".into())
    }

    pub fn shown_tooltip(&self) -> Option<String> {
        self.tooltip.clone().or_else(|| self.label.clone())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemAction {
    pub action: String,
    pub icon: String,
    pub tooltip: Option<String>,
    pub tone: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub tone: Option<String>,
    pub badge: Option<String>,
    pub action: Option<String>,
    pub actions: Vec<ItemAction>,
    pub menu: Vec<MenuEntry>,
    pub avatar: Option<Avatar>,
    pub status: Option<String>,
    pub dim: bool,
    pub strong: bool,
    pub indent: f64,
    pub expanded: Option<bool>,
    pub toggle: Option<String>,
    pub meta: Option<String>,
    pub meta_tone: Option<String>,
    pub subtitle_tone: Option<String>,
    pub selected: bool,
}

impl ListItem {
    pub fn parse(it: &Obj) -> ListItem {
        let actions = objs(it, "actions")
            .into_iter()
            .filter_map(|a| {
                let icon = s(a, "icon").filter(|i| material_icon(i).is_some())?;
                Some(ItemAction { action: s(a, "action")?, icon, tooltip: s(a, "tooltip"), tone: s(a, "tone") })
            })
            .collect();
        ListItem {
            title: shown(it, "title").unwrap_or_default(),
            subtitle: s(it, "subtitle"),
            icon: s(it, "icon"),
            tone: s(it, "tone"),
            badge: s(it, "badge"),
            action: action_of(it),
            actions,
            menu: menu_of(&objs(it, "menu")),
            avatar: Avatar::parse(it.get("avatar")),
            status: s(it, "status"),
            dim: on(it, "dim"),
            strong: on(it, "strong"),
            indent: num(it, "indent").unwrap_or(0.0),
            expanded: it.get("expanded").and_then(Value::as_bool),
            toggle: s(it, "toggle").or_else(|| s(it, "action")),
            meta: s(it, "meta"),
            meta_tone: s(it, "metaTone"),
            subtitle_tone: s(it, "subtitleTone"),
            selected: on(it, "selected"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TabItem {
    pub label: String,
    pub action: Option<String>,
    pub active: bool,
    pub count: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading { text: String },
    Text { text: String, style: Option<String>, center: bool },
    Markdown { text: String },
    Code { text: String },
    Divider,
    Section { text: String, count: Option<String>, collapsed: Option<bool>, action: Option<String>, label: bool },
    Button(Button),
    Row { children: Vec<(Block, f64)>, align: Option<String> },
    Input { id: String, label: Option<String>, placeholder: Option<String>, multiline: bool, submit: Option<String>, icon: Option<String>, disabled: bool },
    Checkbox { id: String, label: String, action: Option<String>, disabled: bool },
    Select { id: String, options: Vec<(String, String)>, label: Option<String>, placeholder: Option<String>, loading: bool, action: Option<String>, disabled: bool },
    List { items: Vec<ListItem>, empty: String, flat: bool, always_actions: bool },
    Kv { items: Vec<(String, String, Option<String>)> },
    Progress { value: Option<f64>, label: Option<String>, detail: Option<String>, tone: Option<String> },
    Console { id: String, expand: bool, height: f64, empty: String, follow: bool },
    Calendar(super::calendar::CalendarBlock),
    Header { title: String, subtitle: Option<String>, tone: Option<String>, avatar: Option<Avatar>, status: Option<String>, dim: bool, wrap: bool, actions: Vec<Button> },
    Card { children: Vec<Block>, padding: Option<f64> },
    Tabs { items: Vec<TabItem>, center: bool },
    Columns { children: Vec<Block> },
    Terminal { tab_id: Option<String>, expand: bool, height: f64, autofocus: bool, empty: String },
    Unknown { label: String },
    Broken { message: String },
}

fn need_id(b: &Obj, kind: &str) -> Result<String, String> {
    s(b, "id").ok_or_else(|| format!("{kind} sem \"id\""))
}

impl Block {
    pub fn parse_all(blocks: &[Obj]) -> Vec<Block> {
        blocks.iter().map(Block::parse).collect()
    }

    fn kids(b: &Obj) -> Vec<Block> {
        children(b).into_iter().map(Block::parse).collect()
    }

    pub fn parse(b: &Obj) -> Block {
        Self::parse_inner(b).unwrap_or_else(|message| Block::Broken { message })
    }

    fn parse_inner(b: &Obj) -> Result<Block, String> {
        let kind = b.get("type").and_then(Value::as_str);
        Ok(match kind {
            Some("heading") => Block::Heading { text: text(b, "text") },
            Some("text") => Block::Text {
                text: text(b, "text"),
                style: s(b, "style"),
                center: b.get("align").and_then(Value::as_str) == Some("center"),
            },
            Some("markdown") => Block::Markdown { text: text(b, "text") },
            Some("code") => Block::Code { text: text(b, "text") },
            Some("divider") => Block::Divider,
            Some("section") => Block::Section {
                text: text(b, "text"),
                count: shown(b, "count"),
                collapsed: b.get("collapsed").and_then(Value::as_bool),
                action: action_of(b),
                label: b.get("style").and_then(Value::as_str) == Some("label"),
            },
            Some("button") => Block::Button(Button::parse(b)),
            Some("row") => Block::Row {
                children: children(b)
                    .into_iter()
                    .map(|c| (Block::parse(c), num(c, "width").unwrap_or(ROW_CHILD_WIDTH)))
                    .collect(),
                align: s(b, "align"),
            },
            Some("input") => Block::Input {
                id: need_id(b, "input")?,
                label: s(b, "label"),
                placeholder: s(b, "placeholder"),
                multiline: on(b, "multiline"),
                submit: s(b, "submit"),
                icon: s(b, "icon"),
                disabled: on(b, "disabled"),
            },
            Some("checkbox") => {
                let id = need_id(b, "checkbox")?;
                Block::Checkbox {
                    label: s(b, "label").unwrap_or_else(|| id.clone()),
                    id,
                    action: s(b, "action"),
                    disabled: on(b, "disabled"),
                }
            }
            Some("select") => Block::Select {
                id: need_id(b, "select")?,
                options: select_options(b),
                label: s(b, "label"),
                placeholder: s(b, "placeholder"),
                loading: on(b, "loading"),
                action: s(b, "action"),
                disabled: on(b, "disabled"),
            },
            Some("list") => Block::List {
                items: objs(b, "items").into_iter().map(ListItem::parse).collect(),
                empty: s(b, "empty").unwrap_or_else(|| LIST_EMPTY.into()),
                flat: on(b, "flat"),
                always_actions: on(b, "alwaysActions"),
            },
            Some("kv") => Block::Kv {
                items: objs(b, "items")
                    .into_iter()
                    .map(|it| {
                        (shown(it, "key").unwrap_or_default(), shown(it, "value").unwrap_or_default(), s(it, "tone"))
                    })
                    .collect(),
            },
            Some("progress") => Block::Progress {
                value: num(b, "value").map(|v| v.clamp(0.0, 1.0)),
                label: s(b, "label"),
                detail: s(b, "detail"),
                tone: s(b, "tone"),
            },
            Some("console") => Block::Console {
                id: need_id(b, "console")?,
                expand: on(b, "expand"),
                height: num(b, "height").unwrap_or(CONSOLE_HEIGHT),
                empty: s(b, "empty").unwrap_or_else(|| CONSOLE_EMPTY.into()),
                follow: b.get("follow") != Some(&Value::Bool(false)),
            },
            Some("calendar") => {
                let id = need_id(b, "calendar")?;
                Block::Calendar(super::calendar::CalendarBlock::parse(id, b))
            }
            Some("header") => Block::Header {
                title: text(b, "title"),
                subtitle: s(b, "subtitle"),
                tone: s(b, "tone"),
                avatar: Avatar::parse(b.get("avatar")),
                status: s(b, "status"),
                dim: on(b, "dim"),
                wrap: on(b, "wrap"),
                actions: objs(b, "actions")
                    .into_iter()
                    .map(|a| Button { style: Some("icon".into()), ..Button::parse(a) })
                    .collect(),
            },
            Some("card") => Block::Card { children: Self::kids(b), padding: num(b, "padding") },
            Some("tabs") => Block::Tabs {
                items: objs(b, "items")
                    .into_iter()
                    .map(|it| TabItem {
                        label: text(it, "label"),
                        action: action_of(it),
                        active: on(it, "active"),
                        count: shown(it, "count"),
                        icon: s(it, "icon"),
                    })
                    .collect(),
                center: b.get("align").and_then(Value::as_str) == Some("center"),
            },
            Some("columns") => Block::Columns { children: Self::kids(b) },
            Some("terminal") => Block::Terminal {
                tab_id: s(b, "tabId"),
                expand: on(b, "expand"),
                height: num(b, "height").unwrap_or(TERMINAL_HEIGHT),
                autofocus: on(b, "autofocus"),
                empty: s(b, "empty").unwrap_or_else(|| TERMINAL_EMPTY.into()),
            },
            other => Block::Unknown {
                label: format!("{UNKNOWN_BLOCK}{}", other.map_or_else(|| "(sem \"type\")".to_owned(), str::to_owned)),
            },
        })
    }
}

fn select_options(b: &Obj) -> Vec<(String, String)> {
    b.get("options")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|o| match o {
            Value::String(v) => Some((v.clone(), v.clone())),
            Value::Object(m) => {
                let v = s(m, "value")?;
                Some((v.clone(), s(m, "label").unwrap_or(v)))
            }
            _ => None,
        })
        .collect()
}

pub fn select_choice(options: &[(String, String)], value: Option<&Value>) -> Option<usize> {
    let v = value?.as_str()?;
    options.iter().position(|(o, _)| o == v)
}
