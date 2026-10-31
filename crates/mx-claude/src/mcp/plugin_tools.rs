use serde_json::Value;

use super::sink::BoxFuture;

/// Tools that enabled plugins add to the `maestria` MCP server.
///
/// `list` returns MCP tool descriptors (`name`, `description`, `inputSchema`) and is read on every
/// `tools/list`; `call` answers `None` when no enabled plugin declares `name`, otherwise the MCP tool
/// result to send back as is (a plugin failure is already an `isError` result).
pub trait PluginTools: Send + Sync {
    fn list(&self) -> Vec<Value>;
    fn call(&self, tab_id: String, name: String, args: Value) -> BoxFuture<'_, Option<Value>>;
}

pub struct NoTools;

impl PluginTools for NoTools {
    fn list(&self) -> Vec<Value> {
        Vec::new()
    }

    fn call(&self, _: String, _: String, _: Value) -> BoxFuture<'_, Option<Value>> {
        Box::pin(async { None })
    }
}
