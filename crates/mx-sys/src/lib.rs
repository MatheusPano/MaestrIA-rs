#![forbid(unsafe_code)]

pub mod agents;
pub mod code_workspace;
pub mod editor;
pub mod git;
pub mod history;
pub mod notify;
mod paths;
pub mod reveal;
pub mod runner;
pub mod shell;

pub use runner::{BoxFuture, CommandOutput, CommandRunner, FakeRunner, LoginShell};
pub use shell::{ShResult, output};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    MacOs,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Platform::MacOs
        } else {
            Platform::Linux
        }
    }
}
