mod plugin_tools;
mod protocol;
mod sink;
mod tools;

pub use plugin_tools::{NoTools, PluginTools};
pub use protocol::{handle, handle_body, handle_with, parse_error};
pub use sink::{
    BoxFuture, MAX_PENDING, NewSuggestion, NoSink, SuggestionEnd, SuggestionQueued, SuggestionSink,
    TaskRef, Withdrawal, is_task_id, new_task_id,
};
pub use tools::{INSTRUCTIONS, tool_error, tools_json};

pub const SERVER_NAME: &str = "maestria";
pub const SUGGEST_TOOL: &str = "suggest_task";
pub const DISMISS_TOOL: &str = "dismiss_task";
pub const MAX_PROMPT: usize = 32000;
pub const PROTOCOL_VERSION: &str = "2025-06-18";

pub fn tool_names() -> [String; 2] {
    [tool_name(SUGGEST_TOOL), tool_name(DISMISS_TOOL)]
}

pub fn tool_name(tool: &str) -> String {
    format!("mcp__{SERVER_NAME}__{tool}")
}
