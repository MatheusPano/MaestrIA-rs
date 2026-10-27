use std::collections::HashMap;

use mx_core::layout::{DropSide, PaneNode};
use mx_core::model::Tab;

use crate::changes::Areas;
use crate::store::Store;

type Parked = (Option<PaneNode>, Option<String>);

#[derive(Default)]
pub struct Screens {
    pub(crate) current: Option<String>,
    pub(crate) parked: HashMap<Option<String>, Parked>,
}

impl Store {
    pub fn screen(&self) -> Option<&str> {
        self.plugin.screens.current.as_deref()
    }

    pub fn shown_plugin(&self) -> Option<String> {
        let id = self.prefs.sidebar_view.as_ref()?;
        self.rail_plugins().into_iter().find(|p| p == id)
    }

    pub fn has_screen(&self, plugin: &str) -> bool {
        self.plugin_info(plugin)
            .is_some_and(|p| p.active() && p.manifest.as_ref().is_some_and(|m| m.screen))
    }

    pub fn view_plugin_of(&self, tab: &str) -> Option<&str> {
        self.plugin_view(tab).map(|v| v.plugin_id.as_str())
    }

    pub fn screen_of(&self, t: &Tab) -> Option<String> {
        if t.on_sessions {
            return None;
        }
        let id = self.view_plugin_of(&t.id).or(t.owner.as_deref())?;
        self.has_screen(id).then(|| id.to_owned())
    }

    pub fn home_screen_of(&self, tab: &str) -> Option<String> {
        self.view_plugin_of(tab).filter(|id| self.has_screen(id)).map(str::to_owned)
    }

    pub(crate) fn screen_of_id(&self, tab: &str) -> Option<String> {
        self.tab(tab).and_then(|t| self.screen_of(t))
    }

    pub(crate) fn tree_of(&self, screen: Option<&str>) -> Option<&PaneNode> {
        if screen == self.screen() {
            return self.panes.as_ref();
        }
        self.plugin.screens.parked.get(&screen.map(str::to_owned)).and_then(|(t, _)| t.as_ref())
    }

    pub(crate) fn placed(&self, t: &Tab) -> bool {
        self.tree_of(self.screen_of(t).as_deref()).is_some_and(|tree| tree.has(&t.id))
    }

    pub(crate) fn sessions_focus(&self) -> Option<&str> {
        if self.screen().is_none() {
            return self.focused.as_deref();
        }
        self.plugin.screens.parked.get(&None).and_then(|(_, f)| f.as_deref())
    }

    pub(crate) fn drop_parked(&mut self, id: &str) {
        for (tree, focus) in self.plugin.screens.parked.values_mut() {
            if !tree.as_ref().is_some_and(|t| t.has(id)) {
                continue;
            }
            *tree = tree.take().and_then(|t| t.remove(id));
            if focus.as_deref() == Some(id) {
                *focus = None;
            }
        }
    }

    pub(crate) fn enter_screen_of(&mut self, tab: &str) {
        if self.has_tab(tab) {
            let screen = self.screen_of_id(tab);
            self.enter(screen);
        }
    }

    pub(crate) fn enter(&mut self, screen: Option<String>) {
        if screen == self.plugin.screens.current {
            return;
        }
        let live = self.live_sidebar();
        let current = self.plugin.screens.current.take();
        let here = (self.panes.take(), self.focused.take());
        self.plugin.screens.parked.insert(current, here);
        let (tree, focus) = self.plugin.screens.parked.remove(&screen).unwrap_or_default();
        self.panes = tree;
        self.focused = focus;
        self.hole = None;
        self.plugin.screens.current = screen.clone();
        if screen.is_some() {
            self.prefs.sidebar_view = screen;
        } else if self.shown_plugin().is_some_and(|p| self.has_screen(&p)) {
            self.prefs.sidebar_view = None;
        }
        self.announce_sidebar(live, false);
        self.mark(Areas::PANES | Areas::SIDEBAR | Areas::PLUGINS | Areas::PREFS);
    }

    pub(crate) fn sync_screen(&mut self) {
        let want = self.shown_plugin().filter(|p| self.has_screen(p));
        if want != self.plugin.screens.current {
            self.enter(want);
            self.save();
        }
        let gone: Vec<String> = self.plugin.screens.parked.keys().flatten().filter(|id| !self.has_screen(id)).cloned().collect();
        for id in gone {
            self.plugin.screens.parked.remove(&Some(id));
        }
    }

    pub(crate) fn land_screen(&mut self) {
        self.sync_screen();
        if self.screen().is_some() && self.panes.is_none() {
            self.open_screen_home();
        }
    }

    pub fn screen_home(&self) -> Option<mx_plugins::PluginCommand> {
        let id = self.screen()?;
        let home = self.plugin_info(id)?.manifest.as_ref()?.screen_home.clone()?;
        let r = self.plugin.registry.as_ref()?;
        r.commands().into_iter().find(|c| c.plugin_id == id && c.id == home)
    }

    pub fn open_screen_home(&mut self) {
        if let Some(cmd) = self.screen_home() {
            self.run_plugin_command(&cmd.full_id(), None);
        }
    }

    pub fn release_view(&mut self, tab: &str) {
        let on_sessions = self.tab(tab).is_none_or(|t| t.on_sessions);
        if on_sessions || self.home_screen_of(tab).is_none() {
            return;
        }
        self.drop_from_tree(tab);
        if let Some(t) = self.tab_mut(tab) {
            t.on_sessions = true;
        }
        self.enter(None);
        self.land_beside_focus(tab);
    }

    pub fn return_view(&mut self, tab: &str) {
        let home = self.home_screen_of(tab);
        let on_sessions = self.tab(tab).is_some_and(|t| t.on_sessions);
        if !on_sessions || home.is_none() {
            return;
        }
        self.drop_from_tree(tab);
        if let Some(t) = self.tab_mut(tab) {
            t.on_sessions = false;
        }
        self.enter(home);
        self.land_beside_focus(tab);
    }

    fn land_beside_focus(&mut self, tab: &str) {
        let beside = self.focused_tab_id();
        self.panes = match (self.panes.take(), beside) {
            (Some(tree), Some(b)) => Some(tree.insert(tab, &b, DropSide::Right)),
            _ => Some(PaneNode::leaf(tab)),
        };
        self.focused = Some(tab.to_owned());
        self.save();
        self.mark(Areas::PANES | Areas::SIDEBAR);
    }

    pub(crate) fn keeps_place(&self, focused: &str, arriving: &str) -> bool {
        let view = |id: &str| self.tab(id).is_some_and(Tab::is_plugin_view);
        self.screen().is_some() && view(focused) && !view(arriving)
    }

    pub(crate) fn live_sidebar(&self) -> Option<String> {
        if self.prefs.sidebar_hidden {
            return None;
        }
        self.shown_plugin()
    }

    pub(crate) fn announce_sidebar(&mut self, before: Option<String>, always: bool) {
        let now = self.live_sidebar();
        let Some(r) = self.plugin.registry.clone() else { return };
        if let Some(old) = before.as_ref().filter(|o| now.as_ref() != Some(*o)) {
            r.tell(old, "sidebar.hidden", Default::default());
        }
        let Some(id) = now.filter(|n| always || before.as_ref() != Some(n)) else { return };
        if !r.get(&id).is_some_and(|p| p.manifest.as_ref().is_some_and(|m| m.sidebar)) {
            return;
        }
        if r.is_up(&id) {
            r.tell(&id, "sidebar.shown", Default::default());
        } else {
            r.wake(&id);
        }
    }
}
