use mx_core::layout::{DropSide, PaneNode};
use mx_core::model::{Tab, TabKind, Tint};
use mx_core::model::sidebar::{RowKey, sidebar_rows};
use mx_plugins::chord::Chord;
use serde_json::json;

use crate::changes::Areas;
use crate::store::Store;
use crate::timers::{REVEAL_LIFE, TimerKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reveal {
    pub tab_id: String,
    pub within: Option<String>,
    pub scroll: bool,
}

impl Store {
    pub fn dismiss(&mut self, id: &str) {
        if !self.is_open(id) {
            return;
        }
        self.drop_from_tree(id);
        self.save();
        self.mark(Areas::PANES | Areas::SIDEBAR);
    }

    pub fn close_tab(&mut self, id: &str) -> Option<Tab> {
        let at = self.pos(id)?;
        let passive = {
            let t = &mut self.tabs[at];
            t.armed = false;
            t.is_passive()
        };
        if !passive {
            self.p
                .plugins
                .emit("session.closed", json!({"tabId": id}), &["onSession"], None);
        }
        if let Some(mut term) = self.terms.remove(id) {
            let kill = term.kill();
            self.p.rt.spawn(async move {
                kill.await;
                drop(term);
            });
        }
        let tab = self.tabs.remove(at);
        self.drop_from_tree(id);
        self.closing_view(id);
        self.timers.cancel(&TimerKey::Settle(id.to_owned()));
        if self.panes.is_none()
            && let Some(last) = self
                .tabs
                .iter()
                .rev()
                .find(|t| !t.embedded && self.screen_of(t).as_deref() == self.screen())
                .map(|t| t.id.clone())
        {
            self.place(&last);
        }
        self.save();
        self.mark(Areas::PANES | Areas::SIDEBAR);
        Some(tab)
    }

    pub fn close_focused(&mut self) {
        if let Some(id) = self.focused_tab_id() {
            self.close_tab(&id);
        }
    }

    pub fn close_settled(&mut self, root: &str) -> usize {
        let done: Vec<String> = self
            .tabs_of(root)
            .iter()
            .filter(|t| t.done)
            .map(|t| t.id.clone())
            .collect();
        for id in &done {
            self.close_tab(id);
        }
        done.len()
    }

    pub fn settled_in(&self, root: &str) -> usize {
        self.tabs_of(root).iter().filter(|t| t.done).count()
    }

    pub fn rename_tab(&mut self, id: &str, label: &str) {
        self.edit_tab(id, |t| t.custom_label = Some(label.to_owned()));
    }

    pub fn set_tab_tint(&mut self, id: &str, tint: Option<Tint>) {
        self.edit_tab(id, |t| t.tint = tint);
    }

    fn edit_tab(&mut self, id: &str, edit: impl FnOnce(&mut Tab)) {
        let Some(t) = self.tab_mut(id) else { return };
        edit(t);
        self.save();
        self.mark_tab(id);
    }

    pub fn set_done(&mut self, id: &str, done: bool) -> usize {
        let Some(t) = self.tab_mut(id) else { return 0 };
        if t.done == done {
            return 0;
        }
        t.done = done;
        let mut dropped = 0;
        if done {
            t.armed = false;
            dropped = t.follow_ups.len();
            t.follow_ups.clear();
        }
        self.save();
        self.mark_tab(id);
        dropped
    }

    pub fn move_tab(&mut self, id: &str, target: &str) {
        let (Some(from), Some(to)) = (self.pos(id), self.pos(target)) else {
            return;
        };
        if from == to {
            return;
        }
        let feature = self.tabs[to].feature_or_hotfix_id.clone();
        let mut tab = self.tabs.remove(from);
        tab.feature_or_hotfix_id = feature;
        self.tabs.insert(to, tab);
        self.save();
        self.mark(Areas::SIDEBAR);
    }

    pub fn toggle_pin(&mut self, id: &str) {
        if !self.is_open(id) {
            return;
        }
        self.edit_tab(id, |t| t.pinned = !t.pinned);
        self.mark(Areas::PANES);
    }

    pub fn drop_tab(&mut self, id: &str, target: &str, side: DropSide) {
        if id == target || !self.is_open(target) {
            self.focus_pane(id);
            return;
        }
        if !self.has_tab(id) {
            return;
        }
        if self.screen_of_id(id).as_deref() != self.screen() {
            self.select(id);
            return;
        }
        self.drop_from_tree(id);
        if side == DropSide::Center {
            let pin = self.is_pinned(target);
            if pin && let Some(t) = self.tab_mut(id) {
                t.pinned = true;
            }
            if let Some(p) = self.panes.as_mut() {
                p.swap(target, id);
            }
        } else {
            self.panes = self.panes.take().map(|p| p.insert(id, target, side));
        }
        self.focused = Some(id.to_owned());
        self.save();
        self.mark(Areas::PANES | Areas::SIDEBAR);
    }

    pub fn focus_pane(&mut self, id: &str) {
        if self.focused.as_deref() == Some(id) || !self.is_open(id) {
            return;
        }
        self.focused = Some(id.to_owned());
        self.mark(Areas::PANES | Areas::SIDEBAR);
    }

    pub fn cycle_pane(&mut self, delta: i64) {
        let order = self.order();
        if order.len() < 2 {
            return;
        }
        let at = self
            .focused
            .as_deref()
            .and_then(|f| order.iter().position(|t| t == f))
            .unwrap_or(0) as i64;
        let next = (at + delta).rem_euclid(order.len() as i64) as usize;
        self.focused = Some(order[next].clone());
        self.mark(Areas::PANES | Areas::SIDEBAR);
    }

    pub fn cycle(&mut self, delta: i64) {
        let ring: Vec<String> = self
            .tabs
            .iter()
            .filter(|t| !t.embedded && self.screen_of(t).as_deref() == self.screen())
            .map(|t| t.id.clone())
            .collect();
        if ring.is_empty() {
            return;
        }
        let focused = self.focused_tab_id();
        let current = focused
            .and_then(|f| ring.iter().position(|t| *t == f))
            .map_or(-1, |i| i as i64);
        let next = (current + delta).rem_euclid(ring.len() as i64) as usize;
        let id = ring[next].clone();
        self.select(&id);
    }

    pub fn set_hotkey(&mut self, id: &str, chord: Option<&str>) {
        if !self.has_tab(id) {
            return;
        }
        let chord = chord.and_then(valid_hotkey);
        if let Some(k) = &chord {
            let stolen: Vec<String> =
                self.tabs.iter().filter(|t| t.id != id && same_key(t.hotkey.as_ref(), k)).map(|t| t.id.clone()).collect();
            for other in stolen {
                let title = self.tab(&other).map(|t| self.title(t)).unwrap_or_default();
                if let Some(t) = self.tab_mut(&other) {
                    t.hotkey = None;
                }
                self.mark_tab(&other);
                self.show_banner(&format!("{} saiu de \"{title}\"", hotkey_label(k)));
            }
        }
        self.edit_tab(id, |t| t.hotkey = chord);
        self.mark(Areas::PANES);
    }

    pub fn release_hotkey(&mut self, chord: &str) {
        let Some(k) = valid_hotkey(chord) else { return };
        let holders: Vec<String> =
            self.tabs.iter().filter(|t| same_key(t.hotkey.as_ref(), &k)).map(|t| t.id.clone()).collect();
        for id in holders {
            let title = self.tab(&id).map(|t| self.title(t)).unwrap_or_default();
            self.edit_tab(&id, |t| t.hotkey = None);
            self.show_banner(&format!("{} saiu do painel \"{title}\"", hotkey_label(&k)));
        }
        self.mark(Areas::PANES);
    }

    pub fn retain_hotkeys(&mut self, keep: impl Fn(&str) -> bool) {
        for t in &mut self.tabs {
            if t.hotkey.as_deref().is_some_and(|k| !keep(k)) {
                t.hotkey = None;
            }
        }
    }

    pub fn panel_for(&self, accepts: impl Fn(&str) -> bool) -> Option<String> {
        self.tabs.iter().find(|t| t.hotkey.as_deref().is_some_and(&accepts)).map(|t| t.id.clone())
    }

    pub fn resize_split(&mut self, path: &[usize], gutter: isize, delta: f64, total: f64) {
        let Some(split) = self.panes.as_mut().and_then(|p| p.split_at_mut(path)) else {
            return;
        };
        split.resize(gutter, delta, total);
        self.save();
        self.mark(Areas::PANES);
    }

    pub fn tabs_of(&self, root: &str) -> Vec<&Tab> {
        self.tabs
            .iter()
            .filter(|t| {
                t.folder_root == root
                    && !t.is_plugin_view()
                    && t.owner.as_ref().is_none_or(|o| !self.installed_plugins.contains(o))
            })
            .collect()
    }

    pub fn needing_human(&self, root: &str) -> usize {
        self.tabs_of(root)
            .iter()
            .filter(|t| !t.done && t.status().needs_human())
            .count()
    }

    pub fn waiting_sessions(&self) -> Vec<&Tab> {
        self.tabs
            .iter()
            .filter(|t| t.kind == TabKind::Claude && !t.done && t.status().needs_human())
            .collect()
    }

    pub fn tab_at(&self, path: &str) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.cwd == path && !t.exited)
    }

    pub fn reveal_tab(&mut self, id: &str) {
        let Some(tab) = self.tab(id) else { return };
        let (root, loose, feature) = (tab.folder_root.clone(), tab.loose, tab.feature_or_hotfix_id.clone());
        let hidden = !self.matches(tab);
        self.prefs.sidebar_hidden = false;
        self.prefs.sidebar_view = None;
        if self.filtering() && hidden {
            self.clear_search();
        }
        let within = if loose {
            None
        } else {
            sidebar_rows(&self.folders, &self.workspaces, &self.root_order)
                .into_iter()
                .find_map(|r| match r {
                    RowKey::Workspace(w) if self.workspace_by_id(&w).is_some_and(|w| w.has(&root)) => Some(w),
                    _ => None,
                })
        };
        if let Some(w) = within.as_deref().and_then(|w| self.workspaces.iter_mut().find(|x| x.id == w)) {
            w.collapsed = false;
            w.set_folder_collapsed(&root, false);
        } else if !loose && let Some(f) = self.folder_mut(&root) {
            f.collapsed = false;
        }
        if let Some(f) = feature.and_then(|f| self.features.iter_mut().find(|x| x.id == f)) {
            f.collapsed = false;
        }
        self.reveal = Some(Reveal { tab_id: id.to_owned(), within, scroll: true });
        let at = self.now() + REVEAL_LIFE;
        self.timers.set(TimerKey::Reveal, at);
        self.save();
        self.mark(Areas::SIDEBAR | Areas::PREFS);
    }

    pub fn revealed(&self) -> Option<&Reveal> {
        self.reveal.as_ref()
    }

    pub fn take_reveal_scroll(&mut self, id: &str, within: Option<&str>) -> bool {
        match self.reveal.as_mut() {
            Some(r) if r.scroll && r.tab_id == id && r.within.as_deref() == within => {
                r.scroll = false;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn end_reveal(&mut self) {
        if self.reveal.take().is_some() {
            self.mark(Areas::SIDEBAR);
        }
    }

    pub fn look(&mut self, id: &str) {
        self.panes = Some(PaneNode::leaf(id));
        self.focused = Some(id.to_owned());
        self.mark(Areas::PANES);
    }
}

const LINUX: bool = cfg!(target_os = "linux");

pub(crate) fn valid_hotkey(id: &str) -> Option<String> {
    Chord::parse(id).filter(|c| c.rejection(LINUX).is_none()).map(|_| id.to_owned())
}

fn same_key(held: Option<&String>, chord: &str) -> bool {
    held.is_some_and(|h| h == chord || (Chord::parse(h).is_some() && Chord::parse(h) == Chord::parse(chord)))
}

pub fn hotkey_label(id: &str) -> String {
    Chord::parse(id).map(|c| c.label(LINUX)).unwrap_or_else(|| id.to_owned())
}
