use mx_core::layout::{self, DropSide, PaneNode};
use mx_core::model::Tab;

use crate::changes::Areas;
use crate::store::Store;

impl Store {
    pub fn open_panes(&self) -> Vec<&Tab> {
        self.order().iter().filter_map(|id| self.tab(id)).collect()
    }

    pub fn open_pane_ids(&self) -> Vec<String> {
        self.order()
            .into_iter()
            .filter(|id| self.has_tab(id))
            .collect()
    }

    pub fn pane_count(&self) -> usize {
        self.open_pane_ids().len()
    }

    pub fn is_open(&self, id: &str) -> bool {
        self.panes.as_ref().is_some_and(|p| p.has(id))
    }

    pub fn is_pinned(&self, id: &str) -> bool {
        self.tab(id).is_some_and(|t| t.pinned) && self.is_open(id)
    }

    pub fn focused_tab_id(&self) -> Option<String> {
        if let Some(id) = self.focused.as_deref().filter(|id| self.has_tab(id)) {
            return Some(id.to_owned());
        }
        self.order().into_iter().next().filter(|id| self.has_tab(id))
    }

    pub fn focused_tab(&self) -> Option<&Tab> {
        self.focused_tab_id().and_then(|id| self.tab(&id))
    }

    pub fn watching(&self, id: &str) -> bool {
        self.bell.window_active && self.focused.as_deref() == Some(id) && self.is_open(id)
    }

    pub fn select(&mut self, id: &str) {
        let Some(tab) = self.tab(id) else { return };
        if tab.embedded {
            return;
        }
        let (hibernated, group) = (tab.hibernated, tab.group_id.clone());
        if hibernated {
            self.wake(id);
        }
        let active = self.active_group().map(|g| g.id.clone());
        let any_pinned = self.open_pane_ids().iter().any(|t| self.is_pinned(t));
        match active {
            Some(g) if group.as_deref() != Some(g.as_str()) && !any_pinned => {
                self.panes = Some(PaneNode::leaf(id));
                self.focused = Some(id.to_owned());
            }
            _ => self.place(id),
        }
        self.save();
        self.mark(Areas::PANES | Areas::SIDEBAR);
    }

    pub(crate) fn place(&mut self, id: &str) {
        match self.panes.take() {
            None => self.panes = Some(PaneNode::leaf(id)),
            Some(tree) if tree.has(id) => self.panes = Some(tree),
            Some(tree) => {
                self.panes = Some(tree);
                let focused = self
                    .focused_tab_id()
                    .filter(|f| self.is_open(f) && !self.is_pinned(f));
                let loose = focused.or_else(|| {
                    self.open_pane_ids()
                        .into_iter()
                        .find(|t| !self.is_pinned(t))
                });
                if let Some(loose) = loose {
                    if let Some(p) = self.panes.as_mut() {
                        p.swap(&loose, id);
                    }
                } else if !self.fill_hole(id) {
                    self.panes = self.panes.take().map(|p| p.edge(id, DropSide::Right));
                }
            }
        }
        self.focused = Some(id.to_owned());
        self.mark(Areas::PANES);
    }

    fn fill_hole(&mut self, id: &str) -> bool {
        let Some((mut tree, gone)) = self.hole.take() else {
            return false;
        };
        let without = tree.clone().remove(&gone);
        if !layout::same_shape(without.as_ref(), self.panes.as_ref()) {
            return false;
        }
        tree.swap(&gone, id);
        self.panes = Some(tree);
        true
    }

    pub(crate) fn place_beside(&mut self, id: &str, beside: Option<&str>) {
        let mut beside = beside.map(str::to_owned);
        if beside.as_deref().is_some_and(|b| self.is_pinned(b)) {
            beside = self
                .open_pane_ids()
                .into_iter()
                .find(|t| !self.is_pinned(t));
        }
        match beside.filter(|b| self.is_open(b)) {
            Some(b) if self.panes.is_some() => {
                self.panes = self.panes.take().map(|p| p.insert(id, &b, DropSide::Right));
                self.focused = Some(id.to_owned());
                self.mark(Areas::PANES);
            }
            _ => self.place(id),
        }
    }

    pub(crate) fn drop_from_tree(&mut self, id: &str) {
        let loose = self.is_open(id) && !self.is_pinned(id);
        let tree = if loose { self.panes.clone() } else { None };
        let before = self.order();
        let at = before.iter().position(|t| t == id);
        self.panes = self.panes.take().and_then(|p| p.remove(id));
        if let Some(tree) = tree
            && self.open_pane_ids().iter().any(|t| self.is_pinned(t))
        {
            self.hole = Some((tree, id.to_owned()));
        }
        if self.focused.as_deref() == Some(id) {
            let left = self.order();
            self.focused = if left.is_empty() {
                None
            } else {
                let i = at.unwrap_or(0).min(left.len() - 1);
                Some(left[i].clone())
            };
        }
        self.mark(Areas::PANES);
    }
}
