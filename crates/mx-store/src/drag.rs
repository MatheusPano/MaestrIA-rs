use mx_core::model::sidebar::RowKey;

use crate::changes::Areas;
use crate::store::Store;
use crate::workspaces::RowPlace;

fn slide(list: &mut Vec<String>, from: Option<usize>, to: Option<usize>) -> bool {
    let (Some(from), Some(to)) = (from, to) else {
        return false;
    };
    if from == to || from >= list.len() || to >= list.len() {
        return false;
    }
    let item = list.remove(from);
    list.insert(to, item);
    true
}

impl Store {
    pub fn move_root_row(&mut self, row: &RowKey, target: &RowKey) {
        self.pin_root_order();
        let idx = |k: &RowKey| self.root_order.iter().position(|x| *x == k.key());
        let (from, to) = (idx(row), idx(target));
        if slide(&mut self.root_order, from, to) {
            self.save();
            self.mark(Areas::SIDEBAR);
        }
    }

    pub fn move_in_workspace(&mut self, ws: &str, root: &str, target: &str) {
        let Some(w) = self.workspaces.iter_mut().find(|w| w.id == ws) else {
            return;
        };
        let idx = |r: &str| w.folder_roots.iter().position(|x| x == r);
        let (from, to) = (idx(root), idx(target));
        if slide(&mut w.folder_roots, from, to) {
            self.save();
            self.mark(Areas::SIDEBAR);
        }
    }

    pub fn move_folder(&mut self, root: &str, from: Option<&str>, to: &str, before: Option<&str>) {
        self.add_to_workspace(root, to, before);
        if let Some(from) = from.filter(|f| *f != to) {
            self.remove_from_workspace(root, from, None);
        }
    }

    pub fn can_drop(&self, from: &RowPlace, onto: &RowPlace, into: bool) -> bool {
        if into {
            return matches!(from.row, RowKey::Folder(_))
                && matches!(&onto.row, RowKey::Workspace(w) if from.within.as_deref() != Some(w.as_str()));
        }
        if from.row == onto.row && from.within == onto.within {
            return false;
        }
        if matches!(from.row, RowKey::Workspace(_)) {
            return onto.within.is_none();
        }
        true
    }

    pub fn drop_row(&mut self, from: &RowPlace, onto: &RowPlace, into: bool) {
        if !self.can_drop(from, onto, into) {
            return;
        }
        let RowKey::Folder(root) = &from.row else {
            self.move_root_row(&from.row, &onto.row);
            return;
        };
        if into {
            if let RowKey::Workspace(to) = &onto.row {
                self.move_folder(root, from.within.as_deref(), to, None);
            }
            return;
        }
        match (&onto.within, &from.within) {
            (None, None) => self.move_root_row(&from.row, &onto.row),
            (None, Some(w)) => self.remove_from_workspace(root, w, Some(&onto.row)),
            (Some(lane), within) => {
                let RowKey::Folder(target) = &onto.row else { return };
                if within.as_deref() == Some(lane.as_str()) {
                    self.move_in_workspace(lane, root, target);
                } else {
                    self.move_folder(root, within.as_deref(), lane, Some(target));
                }
            }
        }
    }

    pub fn has_hits_in_workspace(&self, ws: &str) -> bool {
        self.folders_of(ws).iter().any(|f| self.has_hits(&f.root))
    }
}
