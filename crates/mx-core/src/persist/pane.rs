use serde_json::Value;

use crate::json::{Obj, get_num, get_str, is_true, strings};
use crate::layout::{self, PaneNode};
use crate::model::{Doc, PlanNote, Setup, SuggestedTask, Tint};
use crate::persist::config::Layout;
use crate::text::Time;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneKind {
    Shell,
    Claude,
    Reader,
    Setup,
    Browser,
}

impl PaneKind {
    pub fn parse(raw: Option<&str>) -> PaneKind {
        match raw {
            Some("claude") => PaneKind::Claude,
            Some("reader") => PaneKind::Reader,
            Some("setup") => PaneKind::Setup,
            Some("browser") => PaneKind::Browser,
            _ => PaneKind::Shell,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaneEntry {
    pub folder_root: Option<String>,
    pub loose: bool,
    pub cwd: Option<String>,
    pub kind: PaneKind,
    pub label: Option<String>,
    pub feature_or_hotfix_id: Option<String>,
    pub doc: Option<Doc>,
    pub setup: Option<Setup>,
    pub session_id: Option<String>,
    pub hibernated: bool,
    pub launcher: Option<String>,
    pub tint: Option<Tint>,
    pub zoom: i64,
    pub group: Option<String>,
    pub touched: Vec<String>,
    pub plans: Vec<PlanNote>,
    pub suggestions: Vec<SuggestedTask>,
    pub done: bool,
    pub pinned: bool,
}

pub fn feature_or_hotfix_id_in(pane: &Obj) -> Option<String> {
    let raw = match pane.get("featureOrHotfixId").filter(|v| !v.is_null()) {
        Some(v) => v.as_str(),
        None => get_str(pane, "projectId").lenient(),
    };
    raw.map(str::to_owned)
}

impl PaneEntry {
    pub fn parse(pane: &Obj, now: Time) -> PaneEntry {
        let owned = |k: &str| get_str(pane, k).lenient().map(str::to_owned);
        let empty = Value::Object(Obj::new());
        let list = |k: &str| match pane.get(k) {
            Some(Value::Array(items)) => items.as_slice(),
            _ => &[],
        };
        PaneEntry {
            folder_root: owned("folderRoot").or_else(|| owned("projectRoot")),
            loose: is_true(pane, "loose"),
            cwd: owned("cwd"),
            kind: PaneKind::parse(get_str(pane, "kind").lenient()),
            label: owned("label"),
            feature_or_hotfix_id: feature_or_hotfix_id_in(pane),
            doc: Doc::from_json(pane.get("doc").unwrap_or(&empty), now),
            setup: Setup::from_json(pane.get("setup").unwrap_or(&empty)),
            session_id: owned("sessionId"),
            hibernated: is_true(pane, "hibernated"),
            launcher: owned("launcher"),
            tint: Tint::by_name(get_str(pane, "tint").lenient()),
            zoom: get_num(pane, "zoom")
                .lenient()
                .map_or(0, |z| z.trunc() as i64),
            group: owned("group"),
            touched: strings(pane.get("touched")),
            plans: list("plans")
                .iter()
                .filter_map(|p| PlanNote::from_json(p, now))
                .collect(),
            suggestions: list("suggestions")
                .iter()
                .filter_map(|t| SuggestedTask::from_json(t, now))
                .collect(),
            done: is_true(pane, "done"),
            pinned: is_true(pane, "pinned"),
        }
    }

    pub fn asleep(&self) -> bool {
        self.hibernated && self.session_id.is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TreeRestore {
    Replace(Option<PaneNode>),
    KeepPlaced,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RestoredLayout {
    pub tree: TreeRestore,
    pub focused: Option<String>,
}

pub fn restore_tree(layout: &Layout, restored: &[Option<String>]) -> Option<RestoredLayout> {
    if restored.iter().all(Option::is_none) {
        return None;
    }
    let at = |i: i64| -> Option<String> {
        usize::try_from(i)
            .ok()
            .and_then(|i| restored.get(i))
            .cloned()
            .flatten()
    };
    let tree = if let Some(tree) = &layout.tree {
        TreeRestore::Replace(layout::from_json(tree, &at))
    } else if let Some(legacy) = &layout.legacy {
        match at(legacy.active.unwrap_or(0)) {
            Some(left) => TreeRestore::Replace(layout::legacy_two_slot(
                Some(left),
                legacy.split.and_then(at),
            )),
            None => TreeRestore::KeepPlaced,
        }
    } else {
        TreeRestore::Replace(None)
    };
    let first = match &tree {
        TreeRestore::Replace(Some(p)) => p.order().into_iter().next(),
        _ => None,
    };
    let focused = at(layout.focused).or(first);
    Some(RestoredLayout { tree, focused })
}
