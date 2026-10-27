use mx_plugins::catalog::{OFFICIAL_CATALOG, PluginUpdates};

use crate::changes::Areas;
use crate::msg::StoreMsg;
use crate::store::Store;

pub const CATALOG_ENV: &str = "MAESTRIA_PLUGIN_CATALOG";

pub fn catalog_urls() -> Vec<String> {
    let custom = std::env::var(CATALOG_ENV).ok().filter(|u| !u.trim().is_empty());
    vec![custom.unwrap_or_else(|| OFFICIAL_CATALOG.to_owned())]
}

impl Store {
    pub fn plugin_updates(&self) -> Option<&PluginUpdates> {
        self.updates.plugins.as_ref()
    }

    pub fn attach_plugin_updates(&mut self, catalogs: Vec<String>) -> Option<PluginUpdates> {
        let registry = self.plugin.registry.clone()?;
        let updates = PluginUpdates::new(registry, catalogs);
        let changed = self.sender();
        updates.set_listener(move || changed.send(StoreMsg::Run(Box::new(|s: &mut Store| s.mark(Areas::PLUGINS)))));
        let installed = self.sender();
        updates.set_installed(move |info| {
            let id = info.id.clone();
            installed.send(StoreMsg::Run(Box::new(move |s: &mut Store| s.plugin_updated(&id))));
        });
        if let Some(old) = self.updates.plugins.replace(updates.clone()) {
            old.stop();
        }
        Some(updates)
    }

    pub(crate) fn start_plugin_updates(&mut self) {
        if let Some(updates) = self.attach_plugin_updates(catalog_urls()) {
            updates.start();
        }
    }

    fn plugin_updated(&mut self, id: &str) {
        self.drop_plugin_windows(id);
        self.plugins_changed();
        self.plugin_prefs_changed();
    }

    fn plugin_prefs_changed(&mut self) {
        self.sync_plugin_prefs();
        self.save();
        self.mark(Areas::PLUGINS);
    }

    pub fn set_plugin_auto_update(&mut self, on: bool) {
        let Some(r) = self.plugin.registry.clone() else { return };
        if r.auto_update() == on {
            return;
        }
        r.set_auto_update(on);
        self.plugin_prefs_changed();
        if on {
            self.check_plugin_updates();
        }
    }

    pub fn set_plugin_manual_update(&mut self, id: &str, manual: bool) {
        let Some(r) = self.plugin.registry.clone() else { return };
        r.set_manual_update(id, manual);
        self.plugin_prefs_changed();
    }

    pub fn check_plugin_updates(&mut self) {
        if let Some(u) = self.updates.plugins.clone() {
            self.p.rt.spawn(async move { u.check().await });
        }
    }
}
