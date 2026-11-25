use std::sync::Arc;

use mx_claude::mcp::{BoxFuture, PluginTools, tool_error};
use mx_plugins::Plugins;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use crate::msg::{StoreMsg, StoreSender};
use crate::store::Store;

pub(crate) struct StoreTools {
    sender: StoreSender,
    plugins: Plugins,
}

impl StoreTools {
    async fn caller(&self, tab_id: String) -> (String, Option<String>) {
        let (reply, answer) = oneshot::channel();
        let fallback = tab_id.clone();
        self.sender.send(StoreMsg::Run(Box::new(move |s: &mut Store| {
            let id = s.canonical_tab(&tab_id);
            let cwd = s.tab(&id).map(|t| t.cwd.clone());
            let _ = reply.send((id, cwd));
        })));
        answer.await.unwrap_or((fallback, None))
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
            let (tab_id, cwd) = self.caller(tab_id).await;
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
