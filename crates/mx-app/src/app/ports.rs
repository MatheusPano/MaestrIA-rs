use std::sync::Arc;

use mx_core::ports::{BoxFuture, CommandRunner, HookServer, Notifier};
use mx_sys::LoginShell;
use mx_sys::shell::q;
use tokio::runtime::Handle;

pub struct SysNotifier {
    runner: Arc<dyn CommandRunner>,
    rt: Handle,
}

impl SysNotifier {
    pub fn new(rt: Handle) -> SysNotifier {
        SysNotifier {
            runner: Arc::new(LoginShell),
            rt,
        }
    }

    fn run(&self, command: String) {
        let runner = self.runner.clone();
        self.rt.spawn(async move {
            runner.run(command, None).await;
        });
    }
}

impl Notifier for SysNotifier {
    fn alert(&self, title: &str, body: &str) {
        let notifier = mx_sys::notify::Notifier::new(self.runner.clone());
        let (title, body) = (title.to_owned(), body.to_owned());
        self.rt.spawn(async move { notifier.alert(&title, &body).await });
    }

    fn badge(&self, _label: Option<&str>) {}

    fn open_link(&self, url: &str) {
        self.run(format!("xdg-open {}", q(url)));
    }

    fn quick_look(&self, path: &str) {
        let runner = self.runner.clone();
        let path = path.to_owned();
        self.rt.spawn(async move {
            mx_sys::reveal::quick_look(runner.as_ref(), &path).await;
        });
    }

    fn reveal(&self, path: &str) {
        let runner = self.runner.clone();
        let path = path.to_owned();
        self.rt.spawn(async move {
            mx_sys::reveal::reveal(runner.as_ref(), &path).await;
        });
    }

    fn choose_markdown(&self, _start_in: Option<&str>) -> BoxFuture<'static, Option<String>> {
        Box::pin(async { None })
    }
}

pub struct Hooks {
    pub server: Option<mx_claude::server::HookServer>,
    pub home: Option<String>,
}

impl HookServer for Hooks {
    fn port(&self) -> u16 {
        self.server.as_ref().map_or(0, |s| s.port())
    }

    fn mcp_config_for(&self, tab_id: &str) -> String {
        self.server
            .as_ref()
            .map(|s| s.mcp_config_for(tab_id))
            .unwrap_or_default()
    }

    fn settings_for(&self, tab_id: &str, cwd: &str) -> String {
        self.server
            .as_ref()
            .map(|s| s.settings_for(tab_id, Some(cwd), self.home.as_deref()))
            .unwrap_or_default()
    }
}
