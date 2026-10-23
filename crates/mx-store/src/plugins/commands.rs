use mx_core::model::{SuggestionChoice, TabKind};
use mx_plugins::commands::{self, CommandRun, CommandValues};
use mx_plugins::suggestions::{SuggestionOrigin, start_params};
use serde_json::Value;

use crate::changes::Areas;
use crate::msg::Done;
use crate::store::Store;

impl Store {
    pub fn run_plugin_command(&mut self, full_id: &str, on: Option<&str>) {
        let Some(r) = self.plugin.registry.clone() else { return };
        let Some(cmd) = r.command_by_id(full_id) else { return };
        let tab = on.map(str::to_owned).or_else(|| self.focused_tab_id());
        let t = tab.as_deref().and_then(|id| self.tab(id));
        let folder = t.map_or_else(|| self.focused_folder().clone(), |t| self.folder_of(t).clone());
        let values = CommandValues {
            cwd: t.map_or_else(|| folder.root.clone(), |t| t.cwd.clone()),
            folder: folder.root.clone(),
            session_id: t.and_then(|t| t.resume_id()).unwrap_or_default().to_owned(),
            title: t.map(|t| self.title(t)).unwrap_or_default(),
            plugin_dir: r.get(&cmd.plugin_id).map(|p| p.dir.display().to_string()).unwrap_or_default(),
        };
        let title = cmd.title.clone();
        match commands::plan(&cmd, &values, tab.as_deref()) {
            CommandRun::Terminal { command } => {
                let id = self.open_shell(&folder, Some(&values.cwd), Some(&command), None, None);
                self.select(&id);
            }
            CommandRun::Background { command } => {
                let (runner, cwd) = (self.p.runner.clone(), values.cwd.clone());
                self.spawn(async move {
                    let out = runner.run(command, Some(cwd)).await;
                    let (text, sticky) = commands::background_banner(&title, out.code, &out.stdout, &out.stderr);
                    Done::Banner { text, sticky }
                });
            }
            CommandRun::Session { text } => {
                let live = t.filter(|t| t.kind == TabKind::Claude && !t.exited && !t.hibernated).map(|t| t.id.clone());
                match live {
                    Some(id) => self.submit(&id, &text),
                    None => self.show_banner(&commands::no_session_banner(&title)),
                }
            }
            CommandRun::Plugin { context } => {
                self.spawn(async move {
                    match r.invoke(&cmd, context).await {
                        Ok(_) => Done::PluginsChanged,
                        Err(e) => Done::Banner { text: commands::failure_banner(&title, &e.message), sticky: true },
                    }
                });
            }
        }
    }

    pub fn set_plugin_enabled(&mut self, id: &str, on: bool) {
        let Some(r) = self.plugin.registry.clone() else { return };
        if !on {
            self.drop_plugin_windows(id);
            if self.screen() == Some(id) {
                self.enter(None);
            }
        }
        let plugin = id.to_owned();
        self.spawn(async move {
            r.set_enabled(&plugin, on).await;
            Done::PluginsChanged
        });
        self.after_plugin_change();
    }

    pub fn plugin_installed(&mut self, id: &str, banner: &str) {
        self.drop_plugin_windows(id);
        self.plugins_changed();
        self.after_plugin_change();
        self.show_banner(banner);
    }

    pub fn restart_plugin(&mut self, id: &str) {
        let Some(r) = self.plugin.registry.clone() else { return };
        let plugin = id.to_owned();
        self.spawn(async move {
            r.restart(&plugin).await;
            Done::PluginsChanged
        });
    }

    pub fn uninstall_plugin(&mut self, id: &str) {
        let Some(r) = self.plugin.registry.clone() else { return };
        self.drop_plugin_windows(id);
        let plugin = id.to_owned();
        self.spawn(async move {
            r.uninstall(&plugin).await;
            Done::PluginsChanged
        });
    }

    pub fn set_plugin_setting(&mut self, id: &str, key: &str, value: Option<Value>) {
        let Some(r) = self.plugin.registry.clone() else { return };
        r.set_setting(id, key, value);
        self.after_plugin_change();
    }

    fn after_plugin_change(&mut self) {
        self.sync_plugin_prefs();
        self.save();
        self.mark(Areas::PLUGINS);
    }

    pub fn suggestion_taker(&self) -> Option<String> {
        self.plugin.registry.as_ref()?.suggestion_takers().into_iter().next().map(|p| p.id)
    }

    pub fn start_suggestion_in_plugin(&mut self, tab: &str, task: &str, choice: SuggestionChoice) -> bool {
        let (Some(r), Some(plugin)) = (self.plugin.registry.clone(), self.suggestion_taker()) else {
            return false;
        };
        let Some(t) = self.tab(tab) else { return false };
        let Some(s) = self.pending_suggestion(tab, task).cloned() else { return false };
        let folder = self.folder_of(t);
        let origin = SuggestionOrigin {
            tab_id: t.id.clone(),
            cwd: t.cwd.clone(),
            root: folder.root.clone(),
            folder: folder.name.clone(),
            is_repo: folder.is_repo,
            branch: if t.branch.is_empty() { folder.branch.clone() } else { t.branch.clone() },
        };
        let params = start_params(&s, choice, &origin);
        self.track_suggestion(task, &plugin, tab, choice);
        let task = task.to_owned();
        self.spawn(async move {
            match r.offer_suggestion(&plugin, params).await {
                Ok(()) => Done::PluginsChanged,
                Err(e) => Done::OfferFailed { plugin, task, message: e.message },
            }
        });
        true
    }

    pub(crate) fn offer_failed(&mut self, plugin: &str, task: &str, message: &str) {
        self.untrack_suggestion(task);
        let name = self.plugin_info(plugin).map_or_else(|| plugin.to_owned(), |p| p.name().to_owned());
        self.show_banner_sticky(&format!("{name}: {message}"));
    }
}
