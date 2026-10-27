use mx_core::model::sidebar::{self, RowKey, sidebar_rows};
use mx_core::model::{Folder, Tint, Workspace, new_workspace_id};
use mx_core::paths::expand_home_with;

use crate::changes::Areas;
use crate::store::Store;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowPlace {
    pub row: RowKey,
    pub within: Option<String>,
}

impl RowPlace {
    pub fn root(row: RowKey) -> RowPlace {
        RowPlace { row, within: None }
    }

    pub fn inside(root: &str, workspace: &str) -> RowPlace {
        RowPlace {
            row: RowKey::Folder(root.to_owned()),
            within: Some(workspace.to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyName;

impl std::fmt::Display for EmptyName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("não pode ser vazio")
    }
}

impl std::error::Error for EmptyName {}

impl Store {
    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    pub fn workspace_by_id(&self, id: &str) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.id == id)
    }

    fn ws_mut(&mut self, id: &str) -> Option<&mut Workspace> {
        self.workspaces.iter_mut().find(|w| w.id == id)
    }

    pub fn folders_of(&self, ws: &str) -> Vec<&Folder> {
        self.workspace_by_id(ws)
            .map(|w| sidebar::folders_of(w, &self.folders))
            .unwrap_or_default()
    }

    pub fn workspaces_of(&self, root: &str) -> Vec<&Workspace> {
        sidebar::workspaces_of(root, &self.workspaces)
    }

    pub fn stands_alone(&self, root: &str) -> bool {
        sidebar::stands_alone(root, &self.workspaces)
    }

    pub fn sidebar_rows(&self) -> Vec<RowKey> {
        sidebar_rows(&self.folders, &self.workspaces, &self.root_order)
    }

    pub(crate) fn pin_root_order(&mut self) {
        self.root_order = self.sidebar_rows().iter().map(RowKey::key).collect();
    }

    fn changed(&mut self) {
        self.save();
        self.mark(Areas::SIDEBAR);
    }

    pub fn create_workspace(&mut self, name: &str, roots: &[&str], code_workspace: Option<&str>) -> Result<String, EmptyName> {
        let label = name.trim();
        if label.is_empty() {
            return Err(EmptyName);
        }
        self.pin_root_order();
        let mut w = Workspace::new(new_workspace_id(), label);
        w.code_workspace_path = code_workspace.map(str::to_owned);
        for r in roots {
            w.add_root(r);
        }
        let id = w.id.clone();
        self.root_order.push(RowKey::Workspace(id.clone()).key());
        self.workspaces.push(w);
        self.changed();
        Ok(id)
    }

    pub fn add_to_workspace(&mut self, root: &str, ws: &str, before: Option<&str>) {
        if self.workspace_by_id(ws).is_none_or(|w| w.has(root)) {
            return;
        }
        self.pin_root_order();
        let Some(w) = self.ws_mut(ws) else { return };
        match before.and_then(|b| w.folder_roots.iter().position(|r| r == b)) {
            Some(i) => w.folder_roots.insert(i, root.to_owned()),
            None => w.folder_roots.push(root.to_owned()),
        }
        self.changed();
    }

    pub fn remove_from_workspace(&mut self, root: &str, ws: &str, at: Option<&RowKey>) {
        if !self.workspace_by_id(ws).is_some_and(|w| w.has(root)) {
            return;
        }
        self.pin_root_order();
        if let Some(w) = self.ws_mut(ws) {
            w.folder_roots.retain(|r| r != root);
            w.collapsed_folders.retain(|r| r != root);
        }
        if self.stands_alone(root) {
            let key = RowKey::Folder(root.to_owned()).key();
            let target = at.and_then(|a| self.root_order.iter().position(|k| *k == a.key()));
            let i = target.unwrap_or_else(|| {
                let wk = RowKey::Workspace(ws.to_owned()).key();
                self.root_order.iter().position(|k| *k == wk).map_or(self.root_order.len(), |i| i + 1)
            });
            self.root_order.insert(i.min(self.root_order.len()), key);
        }
        self.changed();
    }

    pub fn rename_workspace(&mut self, ws: &str, name: &str) {
        let label = name.trim().to_owned();
        let Some(w) = self.ws_mut(ws) else { return };
        if label.is_empty() || w.name == label {
            return;
        }
        w.name = label;
        self.changed();
    }

    pub fn set_workspace_tint(&mut self, ws: &str, tint: Option<Tint>) {
        let Some(w) = self.ws_mut(ws) else { return };
        w.tint = tint;
        self.changed();
    }

    pub fn link_code_workspace(&mut self, ws: &str, path: Option<&str>) {
        let home = self.opts.home.clone();
        let clean = path
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(|p| expand_home_with(p, home.as_deref()));
        let Some(w) = self.ws_mut(ws) else { return };
        w.code_workspace_path = clean;
        self.changed();
    }

    pub fn dissolve_workspace(&mut self, ws: &str) {
        let Some(at) = self.workspaces.iter().position(|w| w.id == ws) else {
            return;
        };
        self.pin_root_order();
        let key = RowKey::Workspace(ws.to_owned()).key();
        let slot = self.root_order.iter().position(|k| *k == key);
        self.root_order.retain(|k| *k != key);
        let w = self.workspaces.remove(at);
        let freed: Vec<String> = sidebar::folders_of(&w, &self.folders)
            .into_iter()
            .filter(|f| self.stands_alone(&f.root))
            .map(|f| RowKey::Folder(f.root.clone()).key())
            .collect();
        let i = slot.unwrap_or(self.root_order.len()).min(self.root_order.len());
        self.root_order.splice(i..i, freed);
        self.changed();
    }

    pub fn closing_with(&self, ws: &str) -> Vec<&Folder> {
        self.folders_of(ws)
            .into_iter()
            .filter(|f| self.workspaces_of(&f.root).len() == 1)
            .collect()
    }

    pub fn close_workspace(&mut self, ws: &str) {
        let Some(name) = self.workspace_by_id(ws).map(|w| w.name.clone()) else {
            return;
        };
        let going: Vec<String> = self.closing_with(ws).iter().map(|f| f.root.clone()).collect();
        for root in &going {
            self.remove_folder(root);
        }
        self.pin_root_order();
        let key = RowKey::Workspace(ws.to_owned()).key();
        self.root_order.retain(|k| *k != key);
        self.workspaces.retain(|w| w.id != ws);
        self.changed();
        let text = if going.len() == 1 {
            format!("Workspace \"{name}\" fechado — 1 pasta saiu da lateral")
        } else {
            format!("Workspace \"{name}\" fechado — {} pastas saíram da lateral", going.len())
        };
        self.show_banner(&text);
    }

    pub fn toggle_workspace_collapsed(&mut self, ws: &str) {
        let Some(w) = self.ws_mut(ws) else { return };
        w.collapsed = !w.collapsed;
        self.changed();
    }

    pub fn is_folder_collapsed(&self, root: &str, within: Option<&str>) -> bool {
        match within {
            None => self.find_folder(root).is_some_and(|f| f.collapsed),
            Some(ws) => self.workspace_by_id(ws).is_some_and(|w| w.is_folder_collapsed(root)),
        }
    }

    pub fn toggle_folder_collapsed(&mut self, root: &str, within: Option<&str>) {
        let Some(ws) = within else {
            self.toggle_collapsed(root);
            return;
        };
        let Some(w) = self.ws_mut(ws) else { return };
        let now = !w.is_folder_collapsed(root);
        w.set_folder_collapsed(root, now);
        self.changed();
    }
}
