use serde_json::Value;

use crate::json::{Obj, get_bool, get_num, get_str, objects, strings};
use crate::model::workspace::code_workspace_name;
use crate::model::{FeatureOrHotfix, Folder, Launcher, PaneGroup, Workspace, new_workspace_id};
use crate::persist::config::{
    Config, DEFAULT_HIBERNATE_MINUTES, Layout, LegacySlots, MAX_SIDEBAR_WIDTH, MIN_SIDEBAR_WIDTH,
    Typography,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sidebar {
    pub folders: Vec<Folder>,
    pub features_or_hotfixes: Vec<FeatureOrHotfix>,
    pub workspaces: Vec<Workspace>,
    pub root_order: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Skipped(pub Vec<String>);

impl Skipped {
    fn note(&mut self, what: impl Into<String>) {
        self.0.push(what.into());
    }
}

fn list<'a>(j: &'a Obj, key: &str, skipped: &mut Skipped) -> &'a [Value] {
    match j.get(key) {
        Some(Value::Array(items)) => items,
        None | Some(Value::Null) => &[],
        Some(_) => {
            skipped.note(format!("{key}: not a list"));
            &[]
        }
    }
}

pub fn read_sidebar(j: &Obj) -> Sidebar {
    read_sidebar_noting(j, &mut Skipped::default())
}

pub(crate) fn read_sidebar_noting(j: &Obj, skipped: &mut Skipped) -> Sidebar {
    let mut out = Sidebar::default();
    let legacy = j.get("folders").is_none_or(Value::is_null);
    let raw_folders: Vec<&Value> = list(j, if legacy { "projects" } else { "folders" }, skipped)
        .iter()
        .filter(|v| v.is_object())
        .collect();
    for p in &raw_folders {
        match Folder::from_json(p) {
            Some(f) => out.folders.push(f),
            None => skipped.note("folder"),
        }
    }
    if !legacy {
        let key = if j.get("featuresOrHotfixes").is_none_or(Value::is_null) {
            "projects"
        } else {
            "featuresOrHotfixes"
        };
        for p in list(j, key, skipped) {
            match FeatureOrHotfix::from_json(p) {
                Some(f) => out.features_or_hotfixes.push(f),
                None => skipped.note("featureOrHotfix"),
            }
        }
    }
    for w in list(j, "workspaces", skipped) {
        match Workspace::from_json(w) {
            Some(ws) => out.workspaces.push(ws),
            None => skipped.note("workspace"),
        }
    }
    for p in &raw_folders {
        let Some(path) = p
            .get("workspace")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let at = match out
            .workspaces
            .iter()
            .position(|w| w.code_workspace_path.as_deref() == Some(path))
        {
            Some(i) => i,
            None => {
                let mut w = Workspace::new(new_workspace_id(), code_workspace_name(path));
                w.code_workspace_path = Some(path.to_owned());
                out.workspaces.push(w);
                out.workspaces.len() - 1
            }
        };
        let Some(root) = p.get("root").and_then(Value::as_str) else {
            continue;
        };
        let w = &mut out.workspaces[at];
        w.add_root(root);
        if p.get("collapsed") == Some(&Value::Bool(true)) {
            w.set_folder_collapsed(root, true);
        }
    }
    out.root_order = strings(j.get("rootOrder"));
    out
}

fn bool_or(j: &Obj, key: &str, skipped: &mut Skipped) -> bool {
    get_bool(j, key).typed().unwrap_or_else(|| {
        skipped.note(key);
        None
    }) == Some(true)
}

fn layout_of(v: Option<&Value>) -> Option<Layout> {
    let m = v?.as_object()?;
    let int = |k: &str| m.get(k).and_then(Value::as_i64);
    Some(Layout {
        panes: objects(m.get("panes")).into_iter().cloned().collect(),
        tree: m.get("tree").filter(|t| !t.is_null()).cloned(),
        focused: int("focused").unwrap_or(-1),
        legacy: m.contains_key("active").then(|| LegacySlots {
            active: int("active"),
            split: int("split"),
        }),
    })
}

impl Config {
    pub fn from_json(j: &Obj) -> (Config, Skipped) {
        let mut skipped = Skipped::default();
        let sidebar = read_sidebar_noting(j, &mut skipped);
        let mut c = Config {
            folders: sidebar.folders,
            workspaces: sidebar.workspaces,
            root_order: sidebar.root_order,
            features_or_hotfixes: sidebar.features_or_hotfixes,
            ..Config::default()
        };
        for g in list(j, "groups", &mut skipped) {
            match PaneGroup::from_json(g) {
                Some(g) => c.groups.push(g),
                None => skipped.note("group"),
            }
        }
        for l in list(j, "launchers", &mut skipped) {
            match Launcher::from_json(l) {
                Some(l) => c.launchers.push(l),
                None => skipped.note("launcher"),
            }
        }
        if let Some(w) = get_num(j, "sidebarWidth").lenient() {
            c.sidebar_width = w.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
        }
        c.sidebar_hidden = bool_or(j, "sidebarHidden", &mut skipped);
        c.status_bar_top = bool_or(j, "statusBarTop", &mut skipped);
        c.rail_order = strings(j.get("railOrder"));
        c.sidebar_view = get_str(j, "sidebarView").lenient().map(str::to_owned);
        c.do_not_disturb = bool_or(j, "doNotDisturb", &mut skipped);
        c.suggestions_to_plugin = bool_or(j, "suggestionsToPlugin", &mut skipped);
        c.floats = j
            .get("floats")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        c.groups_collapsed = bool_or(j, "groupsCollapsed", &mut skipped);
        c.hibernate_minutes = get_num(j, "hibernateMinutes")
            .lenient()
            .map_or(DEFAULT_HIBERNATE_MINUTES, |n| n.trunc() as i64);
        c.auto_update = get_bool(j, "autoUpdate").lenient() == Some(true);
        c.theme = get_str(j, "theme").lenient().map(str::to_owned);
        c.typography = match j.get("type").and_then(Value::as_object) {
            Some(t) => Typography::new(
                get_str(t, "mono").lenient(),
                get_num(t, "size").lenient(),
                get_num(t, "line").lenient(),
            ),
            None => Typography::default(),
        };
        c.shortcuts = j
            .get("shortcuts")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        c.plugins = j
            .get("plugins")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        c.layout = layout_of(j.get("layout"));
        (c, skipped)
    }
}
