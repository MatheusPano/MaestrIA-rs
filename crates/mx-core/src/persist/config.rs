use serde_json::{Value, json};

use crate::json::{Obj, f64_value};
use crate::model::sidebar::root_order_keys;
use crate::model::{FeatureOrHotfix, Folder, Launcher, PaneGroup, Workspace};

pub const DEFAULT_SIDEBAR_WIDTH: f64 = 352.0;
pub const MIN_SIDEBAR_WIDTH: f64 = 248.0;
pub const MAX_SIDEBAR_WIDTH: f64 = 680.0;
pub const DEFAULT_HIBERNATE_MINUTES: i64 = 30;
pub const MAX_HIBERNATE_MINUTES: i64 = 1440;
pub const DEFAULT_THEME: &str = "maestria";
pub const RETIRED_THEMES: [(&str, &str); 4] = [
    ("tokyo-night", "maestria"),
    ("catppuccin-frappe", "catppuccin-mocha"),
    ("rose-pine", "catppuccin-mocha"),
    ("chatgpt-dark", "graphite"),
];

pub fn resolve_theme(raw: Option<&str>, known: &dyn Fn(&str) -> bool) -> String {
    let Some(id) = raw else {
        return DEFAULT_THEME.into();
    };
    if known(id) {
        return id.into();
    }
    RETIRED_THEMES
        .iter()
        .find(|(old, _)| *old == id)
        .map_or(DEFAULT_THEME, |(_, new)| new)
        .into()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Typography {
    pub mono: String,
    pub size: f64,
    pub line: f64,
}

impl Default for Typography {
    fn default() -> Self {
        Typography {
            mono: Self::DEFAULT_MONO.into(),
            size: Self::DEFAULT_SIZE,
            line: Self::DEFAULT_LINE,
        }
    }
}

impl Typography {
    pub const DEFAULT_MONO: &str = "Hack";
    pub const DEFAULT_SIZE: f64 = 13.0;
    pub const DEFAULT_LINE: f64 = 1.2;
    pub const MIN_SIZE: f64 = 9.0;
    pub const MAX_SIZE: f64 = 24.0;
    pub const MIN_LINE: f64 = 1.0;
    pub const MAX_LINE: f64 = 1.8;

    pub fn new(mono: Option<&str>, size: Option<f64>, line: Option<f64>) -> Typography {
        let round = |v: f64| (v * 100.0).round() / 100.0;
        Typography {
            mono: mono.unwrap_or(Self::DEFAULT_MONO).to_owned(),
            size: round(
                size.unwrap_or(Self::DEFAULT_SIZE)
                    .clamp(Self::MIN_SIZE, Self::MAX_SIZE),
            ),
            line: round(
                line.unwrap_or(Self::DEFAULT_LINE)
                    .clamp(Self::MIN_LINE, Self::MAX_LINE),
            ),
        }
    }

    pub fn clamp_zoom(&self, zoom: i64) -> i64 {
        let lo = (Self::MIN_SIZE - self.size).ceil() as i64;
        let hi = (Self::MAX_SIZE - self.size).floor() as i64;
        zoom.clamp(lo, hi)
    }

    pub fn size_at(&self, zoom: i64) -> f64 {
        self.size + self.clamp_zoom(zoom) as f64
    }

    pub fn to_json(&self) -> Obj {
        let mut m = Obj::new();
        if self.mono != Self::DEFAULT_MONO {
            m.insert("mono".into(), json!(self.mono));
        }
        if self.size != Self::DEFAULT_SIZE {
            m.insert("size".into(), f64_value(self.size));
        }
        if self.line != Self::DEFAULT_LINE {
            m.insert("line".into(), f64_value(self.line));
        }
        m
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LegacySlots {
    pub active: Option<i64>,
    pub split: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub panes: Vec<Obj>,
    pub tree: Option<Value>,
    pub focused: i64,
    pub legacy: Option<LegacySlots>,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            panes: Vec::new(),
            tree: None,
            focused: -1,
            legacy: None,
        }
    }
}

impl Layout {
    pub fn to_json(&self) -> Value {
        let mut m = Obj::new();
        m.insert(
            "panes".into(),
            Value::Array(self.panes.iter().cloned().map(Value::Object).collect()),
        );
        if let Some(tree) = self.tree.as_ref().filter(|t| !t.is_null()) {
            m.insert("tree".into(), tree.clone());
        }
        m.insert("focused".into(), json!(self.focused));
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub folders: Vec<Folder>,
    pub workspaces: Vec<Workspace>,
    pub root_order: Vec<String>,
    pub features_or_hotfixes: Vec<FeatureOrHotfix>,
    pub groups: Vec<PaneGroup>,
    pub launchers: Vec<Launcher>,
    pub sidebar_width: f64,
    pub sidebar_hidden: bool,
    pub status_bar_top: bool,
    pub rail_order: Vec<String>,
    pub sidebar_view: Option<String>,
    pub do_not_disturb: bool,
    pub suggestions_to_plugin: bool,
    pub floats: Obj,
    pub groups_collapsed: bool,
    pub hibernate_minutes: i64,
    pub theme: Option<String>,
    pub typography: Typography,
    pub shortcuts: Obj,
    pub plugins: Obj,
    pub layout: Option<Layout>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            folders: Vec::new(),
            workspaces: Vec::new(),
            root_order: Vec::new(),
            features_or_hotfixes: Vec::new(),
            groups: Vec::new(),
            launchers: Vec::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            sidebar_hidden: false,
            status_bar_top: false,
            rail_order: Vec::new(),
            sidebar_view: None,
            do_not_disturb: false,
            suggestions_to_plugin: false,
            floats: Obj::new(),
            groups_collapsed: false,
            hibernate_minutes: DEFAULT_HIBERNATE_MINUTES,
            theme: None,
            typography: Typography::default(),
            shortcuts: Obj::new(),
            plugins: Obj::new(),
            layout: None,
        }
    }
}

impl Config {
    pub fn to_json(&self) -> Value {
        let list = |items: Vec<Value>| Value::Array(items);
        let mut m = Obj::new();
        m.insert(
            "folders".into(),
            list(self.folders.iter().map(Folder::to_json).collect()),
        );
        if !self.workspaces.is_empty() {
            m.insert(
                "workspaces".into(),
                list(self.workspaces.iter().map(Workspace::to_json).collect()),
            );
        }
        m.insert(
            "rootOrder".into(),
            json!(root_order_keys(
                &self.folders,
                &self.workspaces,
                &self.root_order
            )),
        );
        m.insert(
            "featuresOrHotfixes".into(),
            list(
                self.features_or_hotfixes
                    .iter()
                    .map(FeatureOrHotfix::to_json)
                    .collect(),
            ),
        );
        if !self.groups.is_empty() {
            m.insert(
                "groups".into(),
                list(self.groups.iter().map(PaneGroup::to_json).collect()),
            );
        }
        if !self.launchers.is_empty() {
            m.insert(
                "launchers".into(),
                list(self.launchers.iter().map(Launcher::to_json).collect()),
            );
        }
        m.insert("sidebarWidth".into(), f64_value(self.sidebar_width));
        flag(&mut m, "sidebarHidden", self.sidebar_hidden);
        flag(&mut m, "statusBarTop", self.status_bar_top);
        if !self.rail_order.is_empty() {
            m.insert("railOrder".into(), json!(self.rail_order));
        }
        if let Some(v) = &self.sidebar_view {
            m.insert("sidebarView".into(), json!(v));
        }
        flag(&mut m, "doNotDisturb", self.do_not_disturb);
        flag(&mut m, "suggestionsToPlugin", self.suggestions_to_plugin);
        if !self.floats.is_empty() {
            m.insert("floats".into(), Value::Object(self.floats.clone()));
        }
        flag(&mut m, "groupsCollapsed", self.groups_collapsed);
        if self.hibernate_minutes != DEFAULT_HIBERNATE_MINUTES {
            m.insert("hibernateMinutes".into(), json!(self.hibernate_minutes));
        }
        m.insert(
            "theme".into(),
            json!(self.theme.as_deref().unwrap_or(DEFAULT_THEME)),
        );
        let typography = self.typography.to_json();
        if !typography.is_empty() {
            m.insert("type".into(), Value::Object(typography));
        }
        if !self.shortcuts.is_empty() {
            m.insert("shortcuts".into(), Value::Object(self.shortcuts.clone()));
        }
        if !self.plugins.is_empty() {
            m.insert("plugins".into(), Value::Object(self.plugins.clone()));
        }
        let layout = self.layout.clone().unwrap_or_default();
        m.insert("layout".into(), layout.to_json());
        Value::Object(m)
    }

    pub fn to_json_string(&self) -> String {
        serde_json::to_string(&self.to_json()).unwrap_or_else(|_| "{}".into())
    }
}

fn flag(m: &mut Obj, key: &str, on: bool) {
    if on {
        m.insert(key.into(), json!(true));
    }
}
