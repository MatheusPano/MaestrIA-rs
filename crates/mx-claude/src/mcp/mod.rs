mod protocol;
mod sink;
mod tools;

pub use protocol::{handle, handle_body, parse_error};
pub use sink::{
    BoxFuture, MAX_PENDING, NewSuggestion, NoSink, SuggestionEnd, SuggestionQueued, SuggestionSink,
    TaskRef, Withdrawal, is_task_id, new_task_id,
};
pub use tools::{INSTRUCTIONS, tools_json};

pub const SERVER_NAME: &str = "maestria";
pub const SUGGEST_TOOL: &str = "suggest_task";
pub const DISMISS_TOOL: &str = "dismiss_task";
pub const MAX_PROMPT: usize = 32000;
pub const PROTOCOL_VERSION: &str = "2025-06-18";

pub fn tool_names() -> [String; 2] {
    [
        format!("mcp__{SERVER_NAME}__{SUGGEST_TOOL}"),
        format!("mcp__{SERVER_NAME}__{DISMISS_TOOL}"),
    ]
}
