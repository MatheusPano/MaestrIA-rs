use crate::model::{Folder, Workspace};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RowKey {
    Workspace(String),
    Folder(String),
}

impl RowKey {
    pub fn parse(key: &str) -> Option<RowKey> {
        if let Some(id) = key.strip_prefix("workspace:") {
            return Some(RowKey::Workspace(id.to_owned()));
        }
        key.strip_prefix("folder:")
            .map(|r| RowKey::Folder(r.to_owned()))
    }

    pub fn key(&self) -> String {
        match self {
            RowKey::Workspace(id) => format!("workspace:{id}"),
            RowKey::Folder(root) => format!("folder:{root}"),
        }
    }
}

pub fn folders_of<'a>(w: &Workspace, folders: &'a [Folder]) -> Vec<&'a Folder> {
    w.folder_roots
        .iter()
        .filter_map(|r| folders.iter().find(|f| &f.root == r))
        .collect()
}

pub fn workspaces_of<'a>(root: &str, workspaces: &'a [Workspace]) -> Vec<&'a Workspace> {
    workspaces.iter().filter(|w| w.has(root)).collect()
}

pub fn stands_alone(root: &str, workspaces: &[Workspace]) -> bool {
    !workspaces.iter().any(|w| w.has(root))
}

fn row_of(key: &str, folders: &[Folder], workspaces: &[Workspace]) -> Option<RowKey> {
    match RowKey::parse(key)? {
        RowKey::Workspace(id) => workspaces
            .iter()
            .any(|w| w.id == id)
            .then_some(RowKey::Workspace(id)),
        RowKey::Folder(root) => (folders.iter().any(|f| f.root == root)
            && stands_alone(&root, workspaces))
        .then_some(RowKey::Folder(root)),
    }
}

pub fn sidebar_rows(
    folders: &[Folder],
    workspaces: &[Workspace],
    root_order: &[String],
) -> Vec<RowKey> {
    let mut rows: Vec<RowKey> = Vec::new();
    let mut add = |r: RowKey| {
        if !rows.contains(&r) {
            rows.push(r);
        }
    };
    for key in root_order {
        if let Some(row) = row_of(key, folders, workspaces) {
            add(row);
        }
    }
    for f in folders {
        let ws = workspaces_of(&f.root, workspaces);
        if ws.is_empty() {
            add(RowKey::Folder(f.root.clone()));
        } else {
            for w in ws {
                add(RowKey::Workspace(w.id.clone()));
            }
        }
    }
    for w in workspaces {
        add(RowKey::Workspace(w.id.clone()));
    }
    rows
}

pub fn root_order_keys(
    folders: &[Folder],
    workspaces: &[Workspace],
    root_order: &[String],
) -> Vec<String> {
    sidebar_rows(folders, workspaces, root_order)
        .iter()
        .map(RowKey::key)
        .collect()
}
