use mx_core::json::Obj;
use mx_core::layout;
use mx_core::persist::{Config, Layout, write_atomic};
use mx_core::model::Tab;

use crate::changes::Areas;
use crate::store::Store;
use crate::timers::{SAVE_DEBOUNCE, TimerKey};

impl Store {
    pub fn save(&mut self) {
        if self.restoring || self.gone || self.quitting {
            return;
        }
        let stale: Vec<String> = self
            .tabs
            .iter()
            .filter(|t| t.pinned && !self.placed(t))
            .map(|t| t.id.clone())
            .collect();
        for id in stale {
            if let Some(t) = self.tab_mut(&id) {
                t.pinned = false;
            }
        }
        let at = self.now() + SAVE_DEBOUNCE;
        self.timers.set(TimerKey::Save, at);
    }

    pub fn touch(&mut self) {
        self.save();
        self.mark(Areas::SIDEBAR | Areas::PANES);
    }

    pub fn save_pending(&self) -> bool {
        self.timers.is_set(&TimerKey::Save)
    }

    pub fn flush(&mut self) {
        if self.timers.cancel(&TimerKey::Save) {
            self.write_config();
        }
    }

    fn saved_tabs(&self) -> Vec<&Tab> {
        self.tabs
            .iter()
            .filter(|t| !t.is_plugin_view() && t.owner.is_none() && (!t.exited || t.resumable()))
            .collect()
    }

    pub fn config(&self) -> Config {
        let open = self.saved_tabs();
        let index_of = |id: &str| open.iter().position(|t| t.id == id).map_or(-1, |i| i as i64);
        let panes: Vec<Obj> = open
            .iter()
            .filter_map(|t| t.to_json().as_object().cloned())
            .collect();
        let mut prefs = self.prefs.clone();
        if let Some(serde_json::Value::Object(m)) = self.plugins().map(mx_plugins::Plugins::to_json) {
            prefs.plugins = m;
        }
        if let serde_json::Value::Object(m) = self.plugin.floats.to_json() {
            prefs.floats = m;
        }
        let p = &prefs;
        Config {
            folders: self.folders.clone(),
            workspaces: self.workspaces.clone(),
            root_order: self.root_order.clone(),
            features_or_hotfixes: self.features.clone(),
            groups: self.groups.clone(),
            launchers: self.launchers.clone(),
            sidebar_width: p.sidebar_width,
            sidebar_hidden: p.sidebar_hidden,
            status_bar_top: p.status_bar_top,
            rail_order: p.rail_order.clone(),
            sidebar_view: p.sidebar_view.clone(),
            do_not_disturb: p.do_not_disturb,
            suggestions_to_plugin: p.suggestions_to_plugin,
            floats: p.floats.clone(),
            groups_collapsed: p.groups_collapsed,
            hibernate_minutes: p.hibernate_minutes,
            theme: Some(p.theme.clone()),
            typography: p.typography.clone(),
            shortcuts: p.shortcuts.clone(),
            plugins: p.plugins.clone(),
            layout: Some(Layout {
                tree: layout::to_json(self.tree_of(None), &index_of),
                focused: self.sessions_focus().map_or(-1, index_of),
                panes,
                legacy: None,
            }),
        }
    }

    pub(crate) fn write_config(&self) {
        let text = self.config().to_json_string();
        let _ = write_atomic(&self.opts.config_path(), &text, self.opts.pid);
    }
}
