use serde_json::Value;

use super::sink::BoxFuture;

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
