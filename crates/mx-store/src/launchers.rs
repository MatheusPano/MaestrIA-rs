use mx_core::model::{Launcher, LauncherIcon, ids};

use crate::changes::Areas;
use crate::store::Store;

impl Store {
    pub fn launchers(&self) -> &[Launcher] {
        &self.launchers
    }

    fn launchers_changed(&mut self) {
        self.save();
        self.mark(Areas::LAUNCHERS | Areas::SIDEBAR);
    }

    pub fn add_launcher(&mut self, name: &str, command: &str, icon: LauncherIcon) -> String {
        let id = self.fresh_id(ids::launcher_id, |id| self.launcher_by_id(id).is_some());
        self.launchers.push(Launcher {
            id: id.clone(),
            name: name.trim().to_owned(),
            command: command.trim().to_owned(),
            icon,
        });
        self.launchers_changed();
        id
    }

    pub fn edit_launcher(&mut self, id: &str, name: Option<&str>, command: Option<&str>, icon: Option<LauncherIcon>) {
        let Some(l) = self.launchers.iter_mut().find(|l| l.id == id) else {
            return;
        };
        if let Some(n) = name.map(str::trim).filter(|n| !n.is_empty()) {
            l.name = n.to_owned();
        }
        if let Some(c) = command.map(str::trim).filter(|c| !c.is_empty()) {
            l.command = c.to_owned();
        }
        if let Some(i) = icon {
            l.icon = i;
        }
        self.launchers_changed();
    }

    pub fn remove_launcher(&mut self, id: &str) {
        self.launchers.retain(|l| l.id != id);
        for t in &mut self.tabs {
            if t.launcher_id.as_deref() == Some(id) {
                t.launcher_id = None;
            }
        }
        self.launchers_changed();
    }
}
