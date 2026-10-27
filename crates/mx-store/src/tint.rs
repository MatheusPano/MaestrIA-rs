use mx_core::model::Tint;
use mx_core::model::sidebar::RowKey;

use crate::store::Store;

pub const GROUP_TINTS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabTint {
    Chosen(Tint),
    Group(usize),
}

impl Store {
    pub fn chosen_tint_of(&self, id: &str) -> Option<Tint> {
        let t = self.tab(id)?;
        self.feature_of(t)
            .and_then(|f| f.tint)
            .or(t.tint)
            .or_else(|| self.find_folder(&t.folder_root).and_then(|f| f.tint))
            .or_else(|| {
                self.sidebar_rows()
                    .into_iter()
                    .find_map(|r| match r {
                        RowKey::Workspace(w) => self.workspace_by_id(&w).filter(|w| w.has(&t.folder_root)),
                        RowKey::Folder(_) => None,
                    })
                    .and_then(|w| w.tint)
            })
    }

    pub fn tint_of(&self, id: &str) -> Option<TabTint> {
        self.chosen_tint_of(id)
            .map(TabTint::Chosen)
            .or_else(|| self.group_of(id).map(|g| TabTint::Group(g.tint_index(GROUP_TINTS))))
    }
}
