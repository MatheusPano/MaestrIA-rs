use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use crate::json::{Obj, get_bool, get_str, strings};
use crate::model::Tint;
use crate::model::ids::micros_now;
use crate::text::base36;

static WORKSPACE_SEQ: AtomicU64 = AtomicU64::new(0);

pub fn new_workspace_id() -> String {
    let seq = WORKSPACE_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("ws{}{}", base36(micros_now()), base36(seq))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub collapsed: bool,
    pub tint: Option<Tint>,
    pub code_workspace_path: Option<String>,
    pub folder_roots: Vec<String>,
    pub collapsed_folders: Vec<String>,
}

impl Workspace {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Workspace {
            id: id.into(),
            name: name.into(),
            collapsed: false,
            tint: None,
            code_workspace_path: None,
            folder_roots: Vec::new(),
            collapsed_folders: Vec::new(),
        }
    }

    pub fn has(&self, root: &str) -> bool {
        self.folder_roots.iter().any(|r| r == root)
    }

    pub fn add_root(&mut self, root: &str) -> bool {
        if self.has(root) {
            return false;
        }
        self.folder_roots.push(root.to_owned());
        true
    }

    pub fn is_folder_collapsed(&self, root: &str) -> bool {
        self.collapsed_folders.iter().any(|r| r == root)
    }

    pub fn set_folder_collapsed(&mut self, root: &str, collapsed: bool) {
        let at = self.collapsed_folders.iter().position(|r| r == root);
        match (collapsed, at) {
            (true, None) => self.collapsed_folders.push(root.to_owned()),
            (false, Some(i)) => {
                self.collapsed_folders.remove(i);
            }
            _ => {}
        }
    }

    pub fn to_json(&self) -> Value {
        let mut m = Obj::new();
        m.insert("id".into(), json!(self.id));
        m.insert("name".into(), json!(self.name));
        if self.collapsed {
            m.insert("collapsed".into(), json!(true));
        }
        if let Some(t) = self.tint {
            m.insert("tint".into(), json!(t.name()));
        }
        if let Some(p) = &self.code_workspace_path {
            m.insert("codeWorkspacePath".into(), json!(p));
        }
        m.insert("folderRoots".into(), json!(self.folder_roots));
        if !self.collapsed_folders.is_empty() {
            m.insert("collapsedFolders".into(), json!(self.collapsed_folders));
        }
        Value::Object(m)
    }

    pub fn from_json(v: &Value) -> Option<Workspace> {
        let m = v.as_object()?;
        let name = get_str(m, "name").typed()?.unwrap_or("").trim().to_owned();
        let id = get_str(m, "id").typed()?.unwrap_or("").trim().to_owned();
        let legacy_path = get_str(m, "path").typed()?.unwrap_or("").trim().to_owned();
        if name.is_empty() || (id.is_empty() && legacy_path.is_empty()) {
            return None;
        }
        let code_workspace_path = match get_str(m, "codeWorkspacePath").typed()? {
            Some(p) => Some(p.to_owned()),
            None => (!legacy_path.is_empty()).then_some(legacy_path),
        };
        let mut collapsed_folders: Vec<String> = Vec::new();
        for root in strings(m.get("collapsedFolders")) {
            if !collapsed_folders.contains(&root) {
                collapsed_folders.push(root);
            }
        }
        Some(Workspace {
            id: if id.is_empty() {
                new_workspace_id()
            } else {
                id
            },
            name,
            collapsed: get_bool(m, "collapsed").typed()?.unwrap_or(false),
            tint: Tint::by_name(get_str(m, "tint").typed()?),
            code_workspace_path,
            folder_roots: strings(m.get("folderRoots")),
            collapsed_folders,
        })
    }
}

pub const CODE_WORKSPACE_EXT: &str = ".code-workspace";

pub fn code_workspace_name(path: &str) -> String {
    let base = crate::text::last_segment(path);
    let lower = base.to_lowercase();
    let cut = if lower.ends_with(CODE_WORKSPACE_EXT) && lower.len() == base.len() {
        &base[..base.len() - CODE_WORKSPACE_EXT.len()]
    } else {
        base
    };
    if cut.is_empty() {
        base.to_owned()
    } else {
        cut.to_owned()
    }
}
