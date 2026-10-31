use std::sync::Arc;

use mx_claude::mcp::{BoxFuture, PluginTools, tool_error};
use mx_plugins::Plugins;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use crate::msg::{StoreMsg, StoreSender};
use crate::store::Store;

/// Offers the tools of the enabled plugins on the `maestria` MCP server: the tab of the session
/// that calls gives the plugin its working folder.
pub(crate) struct StoreTools {
    sender: StoreSender,
    plugins: Plugins,
}

impl StoreTools {
    async fn cwd_of(&self, tab_id: &str) -> Option<String> {
        let (reply, answer) = oneshot::channel();
        let id = tab_id.to_owned();
        self.sender.send(StoreMsg::Run(Box::new(move |s: &mut Store| {
            let _ = reply.send(s.tab(&id).map(|t| t.cwd.clone()));
        })));
        answer.await.ok().flatten()
    }
}

impl PluginTools for StoreTools {
    fn list(&self) -> Vec<Value> {
        self.plugins
            .tools()
            .into_iter()
            .map(|t| json!({"name": t.name, "description": t.description, "inputSchema": t.input_schema}))
            .collect()
    }

    fn call(&self, tab_id: String, name: String, args: Value) -> BoxFuture<'_, Option<Value>> {
        Box::pin(async move {
            if !self.plugins.tools().iter().any(|t| t.name == name) {
                return None;
            }
            let cwd = self.cwd_of(&tab_id).await;
            Some(match self.plugins.call_tool(&tab_id, cwd.as_deref(), &name, args).await {
                Ok(answer) => answer,
                Err(e) => tool_error(e.message),
            })
        })
    }
}

impl Store {
    pub fn plugin_tools(&self, plugins: Plugins) -> Arc<dyn PluginTools> {
        Arc::new(StoreTools { sender: self.sender(), plugins })
    }
}
