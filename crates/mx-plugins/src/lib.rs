#![forbid(unsafe_code)]

pub mod chord;
pub mod commands;
pub mod floats;
pub mod host_json;
pub mod manifest;
pub mod permission;
pub mod process;
pub mod rail;
pub mod registry;
pub mod rpc;
pub mod state;
pub mod suggestions;
pub mod ui;

pub use manifest::{CommandTarget, ManifestError, PluginCommand, PluginManifest, PluginSetting, SettingType};
pub use permission::Permission;
pub use registry::{InstallError, MAX_SIDEBAR_COMMANDS, PluginListener, Plugins, StagedPlugin};
pub use registry::{CAPABILITIES, MAESTRIA_COMPAT};
pub use rpc::{HostDispatcher, PluginCalls, PluginHostApi, RpcError};
pub use state::{PluginInfo, PluginState};

pub const PLUGIN_API: i64 = 1;
pub const BLOCKS_VERSION: i64 = 2;
pub const MANIFEST_NAME: &str = "maestria-plugin.json";
pub const SIDEBAR_VIEW_ID: &str = "sidebar";
pub const LOG_CAP: usize = state::LOG_CAP;
pub mod catalog;
