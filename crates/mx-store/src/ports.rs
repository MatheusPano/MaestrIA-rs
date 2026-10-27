use std::path::PathBuf;
use std::sync::Arc;

use mx_core::ports::{Clock, CommandRunner, HookServer, Notifier, PluginBus, TermFactory};
use mx_sys::Platform;

#[derive(Clone)]
pub struct Ports {
    pub clock: Arc<dyn Clock>,
    pub terms: Arc<dyn TermFactory>,
    pub runner: Arc<dyn CommandRunner>,
    pub notifier: Arc<dyn Notifier>,
    pub plugins: Arc<dyn PluginBus>,
    pub hooks: Arc<dyn HookServer>,
    pub rt: tokio::runtime::Handle,
}

#[derive(Debug, Clone)]
pub struct StoreOptions {
    pub state_dir: PathBuf,
    pub chat_home: PathBuf,
    pub home: Option<String>,
    pub platform: Platform,
    pub pid: u32,
}

impl StoreOptions {
    pub fn from_env() -> StoreOptions {
        let home = mx_core::paths::home();
        StoreOptions {
            state_dir: mx_core::paths::state_dir(),
            chat_home: mx_sys::history::home_root(home.as_deref().unwrap_or("")),
            home,
            platform: Platform::current(),
            pid: std::process::id(),
        }
    }

    pub fn at(state_dir: impl Into<PathBuf>, chat_home: impl Into<PathBuf>, home: Option<&str>) -> Self {
        StoreOptions {
            state_dir: state_dir.into(),
            chat_home: chat_home.into(),
            home: home.map(str::to_owned),
            platform: Platform::current(),
            pid: std::process::id(),
        }
    }

    pub fn config_path(&self) -> PathBuf {
        mx_core::paths::config_path(&self.state_dir)
    }
}
