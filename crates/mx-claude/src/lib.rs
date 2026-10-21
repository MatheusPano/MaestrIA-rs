#![forbid(unsafe_code)]

pub mod context;
pub mod docs;
pub mod launch;
pub mod markdown;
pub mod mcp;
pub mod server;
pub mod setup;
pub mod sh;
pub mod usage;

pub use context::{ChipLevel, ContextUsage, format_tokens};
pub use launch::{ClaudeLaunch, mcp_config_for, settings_for, user_status_line};
pub use server::{HookServer, STATUS_LINE_SCRIPT, ServerEvent, port_file, status_line_file};
pub use sh::sh_quote;
