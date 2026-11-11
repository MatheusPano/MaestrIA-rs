use mx_core::layout::geometry::trailing;
use mx_core::layout::{self, DropSide, PANE_GAP, PaneNode};
use mx_core::model::Tab;
use mx_core::persist::PaneOpen;

use crate::changes::Areas;
use crate::store::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    Replace,
    Split(DropSide),
    Auto,
    Mouse,
    FromSetting,
}

impl Store {
    pub fn pane_area(&self) -> Option<(f64, f64)> {
        self.pane_area
    }

    pub fn set_pane_area(&mut self, w: f64, h: f64) {
        if self.pane_area != Some((w, h)) {
            self.pane_area = Some((w, h));
        }
    }

    pub fn pane_cursor(&self) -> Option<(f64, f64)> {
        self.pane_cursor
    }

    pub fn set_pane_cursor(&mut self, at: Option<(f64, f64)>) {
        self.pane_cursor = at;
    }

    fn resolve(&self, how: How) -> How {
        match how {
            How::FromSetting => match self.prefs.pane_open {
                PaneOpen::Replace => How::Replace,
                PaneOpen::Beside => How::Split(DropSide::Right),
                PaneOpen::Below => How::Split(DropSide::Bottom),
                PaneOpen::Auto => How::Auto,
                PaneOpen::Mouse => How::Mouse,
            },
            other => other,
        }
    }

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
        self.select_with(id, How::Replace);
    }

    pub fn select_with(&mut self, id: &str, how: How) {
        self.tracked(|s| s.select_now(id, how));
    }

    fn select_now(&mut self, id: &str, how: How) {
        let how = self.resolve(how);
        let Some(tab) = self.tab(id) else { return };
        if tab.embedded {
            return;
        }
        let (hibernated, group) = (tab.hibernated, tab.group_id.clone());
        if hibernated {
            self.wake(id);
        }
        self.enter_screen_of(id);
        let active = self.active_group().map(|g| g.id.clone());
        let any_pinned = self.open_pane_ids().iter().any(|t| self.is_pinned(t));
        match active {
            Some(g) if how == How::Replace && group.as_deref() != Some(g.as_str()) && !any_pinned => {
                self.panes = Some(PaneNode::leaf(id));
                self.focused = Some(id.to_owned());
            }
            _ => self.place_with(id, how),
        }
        self.save();
        self.mark(Areas::PANES | Areas::SIDEBAR);
    }

    pub(crate) fn place_with(&mut self, id: &str, how: How) {
        self.tracked(|s| s.place_with_now(id, how));
    }

    fn place_with_now(&mut self, id: &str, how: How) {
        let (side, mouse) = match self.resolve(how) {
            How::Split(side) => (Some(side), false),
            How::Auto => (None, false),
            How::Mouse => (None, true),
            How::Replace | How::FromSetting => return self.place(id),
        };
        self.enter_screen_of(id);
        if self.panes.as_ref().is_some_and(|p| !p.has(id))
            && let Some((target, side)) = self.split_spot(side, mouse)
        {
            self.panes = self.panes.take().map(|p| p.insert(id, &target, side));
            self.focused = Some(id.to_owned());
            self.mark(Areas::PANES);
            return;
        }
        self.place(id);
    }

    fn split_spot(&self, side: Option<DropSide>, mouse: bool) -> Option<(String, DropSide)> {
        let tree = self.panes.as_ref()?;
        let target = self
            .focused_tab_id()
            .filter(|f| self.is_open(f))
            .or_else(|| tree.order().into_iter().next())?;
        let Some((w, h)) = self.pane_area else {
            return Some((target, side.unwrap_or(DropSide::Right)));
        };
        match side {
            Some(side) => layout::rect_of(tree, &target, w, h, PANE_GAP)
                .filter(|r| r.splits(side.axis(), PANE_GAP))
                .map(|_| (target, side)),
            None => {
                let (target, side) = layout::auto_split(tree, &target, w, h, PANE_GAP)?;
                let toward = self.pane_cursor.filter(|_| mouse).and_then(|at| {
                    layout::rect_of(tree, &target, w, h, PANE_GAP).map(|r| layout::facing(&r, side, at))
                });
                Some((target, toward.unwrap_or(side)))
            }
        }
    }

    pub(crate) fn beside(&self, target: &str) -> (String, DropSide) {
        self.pane_area
            .zip(self.panes.as_ref())
            .and_then(|((w, h), tree)| layout::auto_split(tree, target, w, h, PANE_GAP))
            .unwrap_or_else(|| (target.to_owned(), DropSide::Right))
    }

    fn split_largest(&self, tree: PaneNode, id: &str) -> PaneNode {
        let largest = self.pane_area.and_then(|(w, h)| layout::largest(&tree, w, h, PANE_GAP));
        match largest {
            Some(r) => tree.insert(id, &r.id, trailing(r.longer_axis())),
            None => tree.edge(id, DropSide::Right),
        }
    }

    pub(crate) fn place(&mut self, id: &str) {
        self.tracked(|s| s.place_now(id));
    }

    fn place_now(&mut self, id: &str) {
        self.enter_screen_of(id);
        match self.panes.take() {
            None => self.panes = Some(PaneNode::leaf(id)),
            Some(tree) if tree.has(id) => self.panes = Some(tree),
            Some(tree) => {
                self.panes = Some(tree);
                let focused = self.focused_tab_id().filter(|f| self.is_open(f));
                let free = |t: &str| !self.is_pinned(t) && !self.keeps_place(t, id);
                let loose = focused.clone().filter(|f| free(f)).or_else(|| {
                    self.open_pane_ids()
                        .into_iter()
                        .find(|t| free(t))
                });
                let host = focused.filter(|f| loose.is_none() && !self.is_pinned(f) && self.keeps_place(f, id));
                if let Some(host) = host {
                    let (target, side) = self.beside(&host);
                    self.panes = self.panes.take().map(|p| p.insert(id, &target, side));
                } else if let Some(loose) = loose {
                    if let Some(p) = self.panes.as_mut() {
                        p.swap(&loose, id);
                    }
                    self.focused = Some(id.to_owned());
                    self.left_screen(&loose);
                } else if !self.fill_hole(id) {
                    self.panes = self.panes.take().map(|p| self.split_largest(p, id));
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
        self.tracked(|s| s.place_beside_now(id, beside));
    }

    fn place_beside_now(&mut self, id: &str, beside: Option<&str>) {
        self.enter_screen_of(id);
        let mut beside = beside.map(str::to_owned);
        if beside.as_deref().is_some_and(|b| self.is_pinned(b)) {
            beside = self
                .open_pane_ids()
                .into_iter()
                .find(|t| !self.is_pinned(t));
        }
        match beside.filter(|b| self.is_open(b)) {
            Some(b) if self.panes.is_some() => {
                let (target, side) = self.beside(&b);
                self.panes = self.panes.take().map(|p| p.insert(id, &target, side));
                self.focused = Some(id.to_owned());
                self.mark(Areas::PANES);
            }
            _ => self.place(id),
        }
    }

    pub(crate) fn drop_from_tree(&mut self, id: &str) {
        self.drop_parked(id);
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
