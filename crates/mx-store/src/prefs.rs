use mx_core::json::Obj;
use mx_core::persist::{
    DEFAULT_HIBERNATE_MINUTES, DEFAULT_SIDEBAR_WIDTH, MAX_HIBERNATE_MINUTES, MAX_SIDEBAR_WIDTH,
    MIN_SIDEBAR_WIDTH, Typography,
};

use crate::changes::Areas;
use crate::store::Store;

#[derive(Debug, Clone, PartialEq)]
pub struct Prefs {
    pub theme: String,
    pub typography: Typography,
    pub sidebar_width: f64,
    pub sidebar_hidden: bool,
    pub status_bar_top: bool,
    pub rail_order: Vec<String>,
    pub sidebar_view: Option<String>,
    pub do_not_disturb: bool,
    pub suggestions_to_plugin: bool,
    pub groups_collapsed: bool,
    pub hibernate_minutes: i64,
    pub shortcuts: Obj,
    pub plugins: Obj,
    pub floats: Obj,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            theme: mx_theme::DEFAULT_THEME_ID.to_owned(),
            typography: Typography::default(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            sidebar_hidden: false,
            status_bar_top: false,
            rail_order: Vec::new(),
            sidebar_view: None,
            do_not_disturb: false,
            suggestions_to_plugin: false,
            groups_collapsed: false,
            hibernate_minutes: DEFAULT_HIBERNATE_MINUTES,
            shortcuts: Obj::new(),
            plugins: Obj::new(),
            floats: Obj::new(),
        }
    }
}

impl Store {
    pub fn prefs(&self) -> &Prefs {
        &self.prefs
    }

    fn set_pref<T: PartialEq>(&mut self, value: T, field: impl Fn(&mut Prefs) -> &mut T) -> bool {
        let slot = field(&mut self.prefs);
        if *slot == value {
            return false;
        }
        *slot = value;
        self.save();
        self.mark(Areas::PREFS);
        true
    }

    pub fn set_theme(&mut self, id: &str) {
        self.set_pref(id.to_owned(), |p| &mut p.theme);
    }

    pub fn set_typography(&mut self, t: Typography) {
        if self.set_pref(t, |p| &mut p.typography) {
            self.mark(Areas::PANES);
        }
    }

    pub fn zoom_focused(&mut self, by: i64) {
        let Some(id) = self.focused_tab_id() else { return };
        let Some(tab) = self.tab(&id) else { return };
        let zoom = self.prefs.typography.clamp_zoom(i64::from(tab.zoom) + by) as i32;
        if zoom == tab.zoom {
            return;
        }
        self.set_zoom(&id, zoom);
    }

    pub fn reset_zoom_focused(&mut self) {
        let Some(id) = self.focused_tab_id() else { return };
        if self.tab(&id).is_some_and(|t| t.zoom != 0) {
            self.set_zoom(&id, 0);
        }
    }

    fn set_zoom(&mut self, id: &str, zoom: i32) {
        if let Some(t) = self.tab_mut(id) {
            t.zoom = zoom;
        }
        self.save();
        self.mark_tab(id);
        self.mark(Areas::PANES);
    }

    pub fn set_sidebar_width(&mut self, w: f64) {
        self.set_pref(w.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH), |p| &mut p.sidebar_width);
    }

    pub fn toggle_sidebar(&mut self) {
        let hidden = !self.prefs.sidebar_hidden;
        self.set_sidebar_hidden(hidden);
    }

    pub fn set_sidebar_hidden(&mut self, hidden: bool) {
        self.set_pref(hidden, |p| &mut p.sidebar_hidden);
    }

    pub fn set_status_bar_top(&mut self, top: bool) {
        self.set_pref(top, |p| &mut p.status_bar_top);
    }

    pub fn set_do_not_disturb(&mut self, on: bool) {
        if self.set_pref(on, |p| &mut p.do_not_disturb) && on {
            self.clear_toasts();
        }
    }

    pub fn toggle_do_not_disturb(&mut self) {
        let on = !self.prefs.do_not_disturb;
        self.set_do_not_disturb(on);
    }

    pub fn set_suggestions_to_plugin(&mut self, on: bool) {
        self.set_pref(on, |p| &mut p.suggestions_to_plugin);
    }

    pub fn set_hibernate_minutes(&mut self, m: i64) {
        self.set_pref(m.clamp(0, MAX_HIBERNATE_MINUTES), |p| &mut p.hibernate_minutes);
    }

    pub fn toggle_groups_collapsed(&mut self) {
        let c = !self.prefs.groups_collapsed;
        self.set_pref(c, |p| &mut p.groups_collapsed);
        self.mark(Areas::GROUPS);
    }

    pub fn set_sidebar_view(&mut self, plugin: Option<&str>) {
        self.set_pref(plugin.map(str::to_owned), |p| &mut p.sidebar_view);
    }

    pub fn set_rail_order(&mut self, order: Vec<String>) {
        self.set_pref(order, |p| &mut p.rail_order);
    }

    pub fn set_shortcuts(&mut self, shortcuts: Obj) {
        self.set_pref(shortcuts, |p| &mut p.shortcuts);
    }

    pub fn set_plugins_config(&mut self, plugins: Obj) {
        self.set_pref(plugins, |p| &mut p.plugins);
    }

    pub fn set_floats(&mut self, floats: Obj) {
        self.set_pref(floats, |p| &mut p.floats);
    }
}
