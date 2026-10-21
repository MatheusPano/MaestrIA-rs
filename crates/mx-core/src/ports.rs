use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

use crate::text::Time;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn ok(&self) -> bool {
        self.code == 0
    }

    pub fn spawn_failure(message: impl Into<String>) -> Self {
        CommandOutput {
            code: -1,
            stdout: String::new(),
            stderr: message.into(),
        }
    }
}

pub trait CommandRunner: Send + Sync {
    fn run(&self, command: String, cwd: Option<String>) -> BoxFuture<'_, CommandOutput>;
}

pub trait Clock: Send + Sync {
    fn now(&self) -> Time;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Time {
        chrono::Local::now()
    }
}

pub trait Term: Send {
    fn start_shell(&mut self, cwd: &str);
    fn start_command(&mut self, command: &str, cwd: &str, display: Option<&str>);
    fn park(&mut self);
    fn remark(&mut self, text: &str);
    fn kill(&mut self) -> BoxFuture<'static, ()>;
    fn hang_up(&mut self);
    fn relaunch(&mut self, command: &str, cwd: &str);
    fn submit(&mut self, text: &str);
    fn pid(&self) -> Option<u32>;
    fn exited(&self) -> bool;
    fn exit_code(&self) -> Option<i32>;
}

pub trait TermFactory: Send + Sync {
    fn create(&self, tab_id: &str) -> Box<dyn Term>;
}

pub trait Notifier: Send + Sync {
    fn alert(&self, title: &str, body: &str);
    fn badge(&self, label: Option<&str>);
    fn open_link(&self, url: &str);
    fn quick_look(&self, path: &str);
    fn reveal(&self, path: &str);
    fn choose_markdown(&self, start_in: Option<&str>) -> BoxFuture<'static, Option<String>>;
}

pub trait PluginBus: Send + Sync {
    fn emit(&self, event: &str, data: Value, activations: &[&str], permission: Option<&str>);
}

pub trait HookServer: Send + Sync {
    fn port(&self) -> u16;
    fn mcp_config_for(&self, tab_id: &str) -> String;
    fn settings_for(&self, tab_id: &str, cwd: &str) -> String;
}
