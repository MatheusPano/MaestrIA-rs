use std::path::Path;

use mx_plugins::catalog::PluginUpdates;
use mx_sys::updater::Updater;

use crate::changes::Areas;
use crate::store::Store;

pub const APP_VERSION: &str = match option_env!("MAESTRIA_VERSION") {
    Some(v) => v,
    None => "",
};

pub(crate) struct Updates {
    pub app: Updater,
    pub generation: u64,
    pub plugins: Option<PluginUpdates>,
}

impl Updates {
    pub fn new(state_dir: &Path) -> Updates {
        Updates { app: Updater::new(APP_VERSION, state_dir), generation: 0, plugins: None }
    }
}

impl Store {
    pub fn app_updater(&self) -> &Updater {
        &self.updates.app
    }

    pub fn start_updates(&mut self) {
        self.start_plugin_updates();
        if let Some(delay) = self.updates.app.start() {
            self.schedule_app_check(delay);
        }
    }

    pub fn set_auto_update(&mut self, on: bool) {
        if on == self.updates.app.active() {
            return;
        }
        self.updates.generation += 1;
        if let Some(delay) = self.updates.app.set_active(on) {
            self.schedule_app_check(delay);
        }
        self.save();
        self.mark(Areas::PREFS);
    }

    pub fn relaunch_after_quit(&mut self) {
        self.updates.app.relaunch_after_quit();
    }

    pub fn install_update(&mut self) -> Option<tokio::sync::oneshot::Receiver<Result<(), String>>> {
        let staged = self.updates.app.begin_install()?;
        self.mark(Areas::PREFS);
        self.run_install(staged)
    }

    fn run_install(&mut self, staged: std::path::PathBuf) -> Option<tokio::sync::oneshot::Receiver<Result<(), String>>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let sender = self.sender();
        self.p.rt.spawn(async move {
            let outcome = mx_sys::updater::install::install(&staged).await;
            sender.send(crate::msg::StoreMsg::Run(Box::new(move |s: &mut Store| {
                let outcome = s.updates.app.installed(outcome);
                if let Err(e) = &outcome {
                    s.show_banner_with(e, true);
                }
                s.mark(Areas::PREFS);
                let _ = tx.send(outcome);
            })));
        });
        Some(rx)
    }

    pub fn on_quit(&self) {
        if let Some(plan) = self.updates.app.quit_plan() {
            mx_sys::updater::install::on_quit(self.opts.pid, &plan);
        }
    }

    fn schedule_app_check(&mut self, delay: std::time::Duration) {
        self.updates.generation += 1;
        let generation = self.updates.generation;
        let sender = self.sender();
        self.p.rt.spawn(async move {
            tokio::time::sleep(delay).await;
            sender.send(crate::msg::StoreMsg::Run(Box::new(move |s: &mut Store| s.app_check_due(generation))));
        });
    }

    fn app_check_due(&mut self, generation: u64) {
        use mx_sys::updater::fetch::{Latest, fetch, latest, latest_url};
        if generation != self.updates.generation {
            return;
        }
        self.schedule_app_check(mx_sys::updater::EVERY);
        if !self.updates.app.can_check() {
            return;
        }
        let current = self.updates.app.current().to_owned();
        let etag = self.updates.app.etag().map(str::to_owned);
        let dir = self.updates.app.dir().to_path_buf();
        let bundle = self.updates.app.target().map(std::path::Path::to_path_buf);
        let sender = self.sender();
        self.p.rt.spawn(async move {
            let found = latest(&latest_url(), &current, etag.as_deref()).await;
            let Ok(Latest::Found { release, etag }) = found else { return };
            let (tx, rx) = tokio::sync::oneshot::channel();
            let wanted = release.clone();
            sender.send(crate::msg::StoreMsg::Run(Box::new(move |s: &mut Store| {
                s.updates.app.set_etag(etag);
                let _ = tx.send(s.updates.app.begin_fetch(wanted.as_ref()));
            })));
            let (Ok(true), Some(release)) = (rx.await, release) else { return };
            let staged = fetch(&current, &release, &dir, bundle.as_deref()).await;
            sender.send(crate::msg::StoreMsg::Run(Box::new(move |s: &mut Store| {
                let _ = s.updates.app.fetched(release, staged);
                s.mark(Areas::PREFS);
            })));
        });
    }
}
