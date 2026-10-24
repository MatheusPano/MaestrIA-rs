use std::path::Path;

use mx_core::model::Doc;
use mx_core::persist::{
    Config, Layout, LoadOutcome, PaneEntry, PaneKind, TreeRestore, load, restore_tree,
};

use crate::changes::Areas;
use crate::prefs::Prefs;
use crate::store::Store;
use crate::tabs_open::ClaudeSpec;

pub(crate) fn is_dir(path: &str) -> bool {
    Path::new(path).is_dir()
}

impl Store {
    pub fn load(&mut self) -> bool {
        let loaded = match load(&self.opts.config_path()) {
            LoadOutcome::Loaded { config, .. } => {
                self.apply_config(*config);
                true
            }
            LoadOutcome::Missing | LoadOutcome::Unreadable { .. } => false,
        };
        self.refresh_git();
        self.mark(Areas::ALL);
        loaded
    }

    fn apply_config(&mut self, c: Config) {
        self.folders = c.folders;
        self.workspaces = c.workspaces;
        self.root_order = c.root_order;
        self.features = c.features_or_hotfixes;
        self.groups = c.groups;
        self.launchers = c.launchers;
        self.prefs = Prefs {
            theme: c.theme.filter(|t| !t.is_empty()).unwrap_or_else(|| mx_theme::DEFAULT_THEME_ID.to_owned()),
            typography: c.typography,
            sidebar_width: c.sidebar_width,
            sidebar_hidden: c.sidebar_hidden,
            status_bar_top: c.status_bar_top,
            rail_order: c.rail_order,
            sidebar_view: c.sidebar_view,
            do_not_disturb: c.do_not_disturb,
            suggestions_to_plugin: c.suggestions_to_plugin,
            groups_collapsed: c.groups_collapsed,
            hibernate_minutes: c.hibernate_minutes,
            shortcuts: c.shortcuts,
            plugins: c.plugins,
            floats: c.floats,
        };
        self.plugin.floats.read_json(&serde_json::Value::Object(self.prefs.floats.clone()));
        if let Some(r) = &self.plugin.registry {
            r.load(&serde_json::Value::Object(self.prefs.plugins.clone()));
        }
        self.settle_theme();
        if let Some(layout) = c.layout {
            self.restore_layout(&layout);
        }
    }

    pub(crate) fn settle_theme(&mut self) {
        let themes = self.plugin.registry.as_ref().map(mx_plugins::Plugins::themes);
        let known = |id: &str| {
            mx_theme::built_in(id).is_some() || themes.as_ref().is_some_and(|t| t.extra().iter().any(|p| p.id == id))
        };
        if known(&self.prefs.theme) {
            return;
        }
        if let Some(to) = mx_theme::retired_alias(&self.prefs.theme).filter(|to| known(to)) {
            self.prefs.theme = to.to_owned();
            self.mark(Areas::PREFS);
        }
    }

    pub fn restore_layout(&mut self, layout: &Layout) {
        if layout.panes.is_empty() {
            return;
        }
        let now = self.now();
        self.restoring = true;
        let mut restored = Vec::with_capacity(layout.panes.len());
        for pane in &layout.panes {
            let entry = PaneEntry::parse(pane, now);
            let id = self.open_pane(&entry);
            if let Some(id) = &id {
                let known_group = entry
                    .group
                    .clone()
                    .filter(|g| self.groups.iter().any(|x| &x.id == g));
                if let Some(tab) = self.tab_mut(id) {
                    tab.hooks.touched.extend(entry.touched);
                    tab.hooks.plans.extend(entry.plans);
                    tab.hooks.suggestions.extend(entry.suggestions);
                    tab.done = entry.done;
                    tab.pinned = entry.pinned;
                    if known_group.is_some() {
                        tab.group_id = known_group;
                    }
                }
            }
            restored.push(id);
        }
        self.restoring = false;
        let Some(r) = restore_tree(layout, &restored) else {
            return;
        };
        if let TreeRestore::Replace(tree) = r.tree {
            self.panes = tree;
        }
        self.focused = r.focused.or_else(|| self.order().into_iter().next());
        self.mark(Areas::ALL);
    }

    pub(crate) fn open_pane(&mut self, e: &PaneEntry) -> Option<String> {
        let folder = if e.loose {
            self.loose.clone()
        } else {
            self.find_folder(e.folder_root.as_deref()?)?.clone()
        };
        let cwd = e.cwd.clone()?;
        if !is_dir(&cwd) {
            return None;
        }
        let feature = e
            .feature_or_hotfix_id
            .clone()
            .filter(|id| self.feature_by_id(id).is_some());
        let id = match e.kind {
            PaneKind::Browser => return None,
            PaneKind::Reader => {
                let doc: Doc = e.doc.clone()?;
                self.new_reader(doc, &folder, Some(&cwd), feature.as_deref())
            }
            PaneKind::Setup => {
                let setup = e.setup.clone()?;
                if !is_dir(&setup.root) {
                    return None;
                }
                self.new_setup(setup, &folder, Some(&cwd), feature.as_deref())
            }
            PaneKind::Claude => self.open_claude(ClaudeSpec {
                folder,
                cwd: cwd.clone(),
                label: e.label.clone(),
                resume_id: e.session_id.clone(),
                feature,
                prompt: None,
                start: !e.asleep(),
            }),
            PaneKind::Shell => {
                let launcher = e
                    .launcher
                    .clone()
                    .filter(|l| self.launcher_by_id(l).is_some());
                self.open_shell(&folder, Some(&cwd), None, feature.as_deref(), launcher.as_deref())
            }
        };
        let zoom = self.prefs.typography.clamp_zoom(e.zoom) as i32;
        if let Some(tab) = self.tab_mut(&id) {
            if let Some(label) = &e.label {
                tab.custom_label = Some(label.clone());
            }
            tab.tint = e.tint;
            tab.zoom = zoom;
        }
        Some(id)
    }
}
