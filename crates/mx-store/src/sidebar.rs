use mx_core::model::sidebar::RowKey;
use mx_core::model::{Folder, Tint, Workspace, new_workspace_id};
use mx_core::paths::expand_home_with;
use mx_core::text::last_segment;
use mx_sys::code_workspace::{self, CodeWorkspace, WorkspaceImport};

use crate::changes::Areas;
use crate::msg::Done;
use crate::restore::is_dir;
use crate::store::Store;

impl Store {
    pub fn folders(&self) -> &[Folder] {
        &self.folders
    }

    pub fn loose(&self) -> &Folder {
        &self.loose
    }

    fn expand(&self, raw: &str) -> String {
        expand_home_with(raw, self.opts.home.as_deref())
    }

    pub fn add_folder(&mut self, raw: &str) {
        let path = self.expand(raw);
        let git = self.git.clone();
        self.spawn(async move {
            let root = git.main_root(&path).await.unwrap_or_else(|| path.clone());
            Done::FolderFound { path, root }
        });
    }

    pub(crate) fn folder_found(&mut self, path: &str, root: &str) {
        match self.adopt(root) {
            None => self.show_banner_sticky(&format!("Pasta não encontrada: {path}")),
            Some(false) => {}
            Some(true) => {
                self.save();
                self.refresh_git();
                self.mark(Areas::SIDEBAR);
            }
        }
    }

    fn adopt(&mut self, root: &str) -> Option<bool> {
        if self.find_folder(root).is_some() {
            return Some(false);
        }
        if !is_dir(root) {
            return None;
        }
        self.folders.push(Folder::new(root, last_segment(root)));
        Some(true)
    }

    pub fn import_workspace(&mut self, raw: &str) {
        let raw = raw.to_owned();
        let home = self.opts.home.clone();
        let git = self.git.clone();
        self.spawn(async move {
            let ws = code_workspace::read_with(&raw, home.as_deref()).await;
            let found = match ws {
                None => None,
                Some(ws) => {
                    let mut roots = Vec::with_capacity(ws.folders.len());
                    for f in &ws.folders {
                        roots.push(git.main_root(&f.path).await.unwrap_or_else(|| f.path.clone()));
                    }
                    Some((ws, roots))
                }
            };
            Done::WorkspaceRead { raw, found }
        });
    }

    pub fn last_import(&self) -> Option<&WorkspaceImport<String>> {
        self.last_import.as_ref()
    }

    pub(crate) fn workspace_read(&mut self, raw: &str, found: Option<(CodeWorkspace, Vec<String>)>) {
        let Some((ws, roots)) = found else {
            let shown = self.expand(raw.trim());
            self.show_banner_sticky(&format!("Não consegui ler esse workspace: {shown}"));
            return;
        };
        if ws.folders.is_empty() {
            self.show_banner_sticky(&format!("O workspace \"{}\" não lista nenhuma pasta", ws.name));
            return;
        }
        let (mut added, mut already, mut missing) = (Vec::new(), Vec::new(), Vec::new());
        for (entry, root) in ws.folders.iter().zip(&roots) {
            match self.adopt(root) {
                None => missing.push(entry.path.clone()),
                Some(false) => already.push(root.clone()),
                Some(true) => {
                    if let Some(name) = &entry.name
                        && *root == entry.path
                        && let Some(f) = self.folder_mut(root)
                    {
                        f.name = name.clone();
                    }
                    added.push(root.clone());
                }
            }
        }
        let adopted: Vec<String> = added.iter().chain(&already).cloned().collect();
        if !adopted.is_empty() {
            let at = match self
                .workspaces
                .iter()
                .position(|w| w.code_workspace_path.as_deref() == Some(ws.path.as_str()))
            {
                Some(i) => i,
                None => {
                    self.pin_root_order();
                    let mut w = Workspace::new(new_workspace_id(), ws.name.clone());
                    w.code_workspace_path = Some(ws.path.clone());
                    self.root_order.push(RowKey::Workspace(w.id.clone()).key());
                    self.workspaces.push(w);
                    self.workspaces.len() - 1
                }
            };
            for root in &adopted {
                self.workspaces[at].add_root(root);
            }
        }
        let result = WorkspaceImport { workspace: ws, added, already, missing };
        self.save();
        if !result.added.is_empty() {
            self.refresh_git();
        }
        self.show_banner_with(&result.summary(), result.sticky());
        self.last_import = Some(result);
        self.mark(Areas::SIDEBAR);
    }

    fn edit_folder(&mut self, root: &str, edit: impl FnOnce(&mut Folder)) {
        let Some(f) = self.folder_mut(root) else { return };
        edit(f);
        self.save();
        self.mark(Areas::SIDEBAR);
    }

    pub fn set_folder_tint(&mut self, root: &str, tint: Option<Tint>) {
        self.edit_folder(root, |f| f.tint = tint);
    }

    pub fn rename_folder(&mut self, root: &str, name: &str) {
        self.edit_folder(root, |f| f.name = name.to_owned());
    }

    pub fn toggle_collapsed(&mut self, root: &str) {
        self.edit_folder(root, |f| f.collapsed = !f.collapsed);
    }

    pub fn remove_folder(&mut self, root: &str) {
        let Some(at) = self.folders.iter().position(|f| f.root == root) else {
            return;
        };
        self.folders.remove(at);
        for w in &mut self.workspaces {
            w.folder_roots.retain(|r| r != root);
            w.collapsed_folders.retain(|r| r != root);
        }
        let key = RowKey::Folder(root.to_owned()).key();
        self.root_order.retain(|k| *k != key);
        self.features.retain(|f| f.folder_root != root);
        let going: Vec<String> = self
            .tabs
            .iter()
            .filter(|t| t.folder_root == root && !t.loose)
            .map(|t| t.id.clone())
            .collect();
        for id in going {
            self.close_tab(&id);
        }
        self.save();
        self.mark(Areas::SIDEBAR);
    }

    pub fn focused_folder(&self) -> &Folder {
        if let Some(t) = self.focused_tab() {
            return self.folder_of(t);
        }
        self.folders.first().unwrap_or(&self.loose)
    }

    pub fn worktrees_of(&self, root: &str) -> &[mx_core::model::WorktreeInfo] {
        self.worktrees.get(root).map_or(&[], Vec::as_slice)
    }
}
