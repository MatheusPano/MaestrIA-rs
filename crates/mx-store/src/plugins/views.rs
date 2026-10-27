use mx_core::json::Obj;
use mx_core::model::{PluginViewRef, Tab, TabKind};
use mx_plugins::floats::FloatCorner;
use mx_plugins::rpc::{BadgeUpdate, FloatContent, SidebarUpdate, ViewOpen, ViewUpdate};
use mx_plugins::ui::view::PluginView;
use mx_plugins::{PluginInfo, SIDEBAR_VIEW_ID};
use serde_json::Value;

use crate::changes::Areas;
use crate::store::Store;

enum Slot {
    Modal,
    Tab(String),
}

fn view_ref(v: &PluginView) -> PluginViewRef {
    PluginViewRef { title: v.title.clone(), plugin_name: v.plugin_name.clone() }
}

impl Store {
    fn find_view(&self, plugin: &str, view: &str) -> Option<Slot> {
        if let Some((_, v)) = &self.plugin.modal
            && v.plugin_id == plugin
            && v.id == view
        {
            return Some(Slot::Modal);
        }
        self.plugin
            .views
            .iter()
            .find(|(_, v)| v.plugin_id == plugin && v.id == view)
            .map(|(id, _)| Slot::Tab(id.clone()))
    }

    fn view_mut(&mut self, slot: &Slot) -> Option<&mut PluginView> {
        match slot {
            Slot::Modal => self.plugin.modal.as_mut().map(|(_, v)| v),
            Slot::Tab(id) => self.plugin.views.get_mut(id),
        }
    }

    fn sync_view_title(&mut self, slot: &Slot) {
        let Some(r) = self.view_mut(slot).map(|v| view_ref(v)) else { return };
        match slot {
            Slot::Modal => {
                if let Some((t, _)) = self.plugin.modal.as_mut() {
                    t.view = Some(r);
                }
            }
            Slot::Tab(id) => {
                if let Some(t) = self.tab_mut(id) {
                    t.view = Some(r);
                }
                self.mark_tab(&id.clone());
            }
        }
        self.mark(Areas::PLUGINS | Areas::PANES);
    }

    pub(crate) fn view_open(&mut self, p: &PluginInfo, req: ViewOpen) -> String {
        if let Some(slot) = self.find_view(&p.id, &req.view_id) {
            let modal = matches!(slot, Slot::Modal);
            if let Some(v) = self.view_mut(&slot) {
                v.apply_open(&req.title, req.blocks, req.rfw.as_ref());
                if modal {
                    v.modal_from = req.from.or(v.modal_from.take());
                } else {
                    v.set_modal(req.from, req.width, req.height);
                }
            }
            self.sync_view_title(&slot);
            return match slot {
                Slot::Modal => self.plugin.modal.as_ref().map(|(t, _)| t.id.clone()).unwrap_or_default(),
                Slot::Tab(id) => {
                    let beside = self.focused_tab_id();
                    self.enter_screen_of(&id);
                    if !self.is_open(&id) {
                        self.place_beside(&id, beside.as_deref());
                    }
                    self.focused = Some(id.clone());
                    id
                }
            };
        }
        let id = self.next_tab_id();
        let loose = self.loose.clone();
        let mut view = PluginView::new(&p.id, p.name(), &req.view_id, &req.title, req.blocks);
        view.set_modal(req.from, req.width, req.height);
        if let Some(r) = &req.rfw {
            view.set_rfw(r);
        }
        let mut tab = Tab::new(&id, &loose, TabKind::Plugin, &loose.root, self.now());
        tab.view = Some(view_ref(&view));
        if req.modal {
            self.plugin.modal = Some((tab, view));
        } else {
            self.tabs.push(tab);
            self.plugin.views.insert(id.clone(), view);
            let beside = self.focused_tab_id();
            self.place_beside(&id, beside.as_deref());
        }
        self.mark(Areas::PLUGINS | Areas::PANES | Areas::SIDEBAR);
        id
    }

    pub(crate) fn view_update(&mut self, plugin: &str, req: ViewUpdate) -> bool {
        let Some(slot) = self.find_view(plugin, &req.view_id) else {
            return false;
        };
        let mut seen = false;
        if let Some(v) = self.view_mut(&slot) {
            let retitled = req.title.as_ref().is_some_and(|t| *t != v.title);
            seen = retitled || v.switches_to(req.blocks.is_some(), req.rfw.as_ref());
            v.apply_update(req.title.as_deref(), req.blocks, req.rfw.as_ref());
        }
        if seen {
            self.sync_view_title(&slot);
        } else {
            self.mark(Areas::PLUGINS);
        }
        true
    }

    pub(crate) fn view_lines(&mut self, plugin: &str, view: &str, console: &str, lines: Option<&Value>) -> bool {
        let target = if view == SIDEBAR_VIEW_ID {
            self.plugin.sidebars.get_mut(plugin)
        } else {
            match self.find_view(plugin, view) {
                Some(slot) => self.view_mut(&slot),
                None => None,
            }
        };
        let Some(v) = target else { return false };
        match lines {
            Some(l) => v.console(console).append(l),
            None => v.console(console).clear(),
        }
        self.mark(Areas::PLUGINS);
        true
    }

    pub(crate) fn view_close(&mut self, plugin: &str, view: &str) {
        match self.find_view(plugin, view) {
            Some(Slot::Modal) => self.close_modal(),
            Some(Slot::Tab(id)) => {
                self.close_tab(&id);
            }
            None => {}
        }
    }

    pub(crate) fn closing_view(&mut self, tab: &str) {
        let Some(view) = self.plugin.views.remove(tab) else { return };
        for t in view.terminal_tabs() {
            let owned = self
                .tab(&t)
                .is_some_and(|x| x.embedded && x.owner.as_deref() == Some(view.plugin_id.as_str()));
            if owned {
                self.close_tab(&t);
            }
        }
    }

    pub fn close_modal(&mut self) {
        let Some((_, view)) = self.plugin.modal.take() else { return };
        if let Some(r) = &self.plugin.registry {
            r.view_closed(&view.plugin_id, &view.id);
        }
        self.mark(Areas::PLUGINS);
    }

    pub fn modal(&self) -> Option<(&Tab, &PluginView)> {
        self.plugin.modal.as_ref().map(|(t, v)| (t, v))
    }

    pub fn dock_modal(&mut self) {
        let Some((tab, view)) = self.plugin.modal.take() else { return };
        let id = tab.id.clone();
        self.tabs.push(tab);
        self.plugin.views.insert(id.clone(), view);
        let beside = self.focused_tab_id();
        self.place_beside(&id, beside.as_deref());
        self.focused = Some(id);
        self.mark(Areas::PLUGINS | Areas::PANES | Areas::SIDEBAR);
    }

    pub(crate) fn sidebar_update(&mut self, p: &PluginInfo, req: SidebarUpdate) -> bool {
        match req.badge {
            BadgeUpdate::Unchanged => {}
            BadgeUpdate::Clear => {
                self.plugin.badges.remove(&p.id);
            }
            BadgeUpdate::Set(b) => {
                self.plugin.badges.insert(p.id.clone(), b);
            }
        }
        if req.blocks.is_some() || req.rfw.is_some() {
            let name = p.name().to_owned();
            let v = self
                .plugin
                .sidebars
                .entry(p.id.clone())
                .or_insert_with(|| PluginView::new(&p.id, &name, SIDEBAR_VIEW_ID, &name, Vec::new()));
            if let Some(r) = &req.rfw {
                v.set_rfw(r);
            }
            if let Some(b) = req.blocks {
                v.rfw_library = None;
                v.set_blocks(b);
            }
        }
        self.mark(Areas::PLUGINS);
        self.sidebar_shown(&p.id)
    }

    pub(crate) fn float_show(&mut self, p: &PluginInfo, c: &FloatContent) {
        self.plugin.floats.show(&p.id, p.name(), c);
        self.mark(Areas::PLUGINS);
    }

    pub(crate) fn float_update(&mut self, plugin: &str, c: &FloatContent) -> bool {
        let shown = self.plugin.floats.update(plugin, c);
        self.mark(Areas::PLUGINS);
        shown
    }

    pub(crate) fn float_hide(&mut self, plugin: &str, id: &str) {
        self.plugin.floats.hide(plugin, id);
        self.mark(Areas::PLUGINS);
    }

    pub fn float_start(&self, plugin: &str, id: &str) -> Option<FloatCorner> {
        self.plugin.floats.of(plugin, id).map(|f| f.start)
    }

    pub(crate) fn drop_plugin_windows(&mut self, plugin: &str) {
        if self.plugin.modal.as_ref().is_some_and(|(_, v)| v.plugin_id == plugin) {
            self.plugin.modal = None;
        }
        let tabs: Vec<String> = self
            .plugin
            .views
            .iter()
            .filter(|(_, v)| v.plugin_id == plugin)
            .map(|(id, _)| id.clone())
            .collect();
        for id in tabs {
            self.close_tab(&id);
        }
        self.plugin.floats.drop_plugin(plugin);
        self.plugin.sidebars.remove(plugin);
        self.plugin.badges.remove(plugin);
        self.mark(Areas::PLUGINS);
    }

    pub fn view_action(&mut self, tab: &str, action: &str, values: Obj) {
        let target = self
            .plugin_view(tab)
            .or_else(|| self.plugin.floats.shown().iter().find(|f| f.tab_id == tab).map(|f| &f.view))
            .or_else(|| self.plugin.sidebars.values().find(|v| format!("sidebar:{}", v.plugin_id) == tab))
            .map(|v| (v.plugin_id.clone(), v.id.clone()));
        if let (Some((plugin, view)), Some(r)) = (target, &self.plugin.registry) {
            r.view_action(&plugin, &view, action, values);
        }
    }
}
