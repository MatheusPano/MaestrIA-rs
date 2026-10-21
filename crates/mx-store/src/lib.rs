#![forbid(unsafe_code)]

mod changes;
mod drag;
mod features;
mod flows;
mod git_ops;
mod groups;
mod hibernate;
mod history_ops;
mod ingest;
mod launchers;
mod lifecycle;
mod msg;
mod notices;
mod placement;
mod plugins;
mod ports;
mod prefs;
mod readers;
mod restore;
mod save;
mod search;
mod sidebar;
mod sink;
mod store;
mod suggestions;
mod tabs_open;
mod tabs_ops;
pub mod testing;
mod timers;
mod tint;
mod workspaces;

#[cfg(test)]
mod tests;

pub use changes::{Areas, Changes};
pub use flows::{FLOW_PATIENCE, FLOW_QUIET, handoff_text, shell_label};
pub use git_ops::{GitSnapshot, NewTask};
pub use hibernate::HIBERNATED;
pub use lifecycle::AGENTS_COMMAND;
pub use msg::{StoreMsg, StoreSender, TaskDone};
pub use notices::{Bell, NoticeEntry};
pub use plugins::{DialogAnswer, DialogAsk, Effect, PluginDialog, PluginHost, PluginUi};
pub use ports::{Ports, StoreOptions};
pub use prefs::Prefs;
pub use readers::DocPlace;
pub use search::Search;
pub use sink::StoreSink;
pub use store::Store;
pub use suggestions::Tracking;
pub use tabs_open::{ClaudeSpec, HIBERNATED_AT_BIRTH};
pub use tabs_ops::Reveal;
pub use timers::TimerKey;
pub use tint::{GROUP_TINTS, TabTint};
pub use workspaces::{EmptyName, RowPlace};
