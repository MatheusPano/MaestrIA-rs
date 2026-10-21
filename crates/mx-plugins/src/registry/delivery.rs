use mx_core::json::Obj;
use mx_core::ports::{BoxFuture, PluginBus};
use serde_json::{Map, Value};

use super::Plugins;
use super::lifecycle::REQUEST_TIMEOUT;
use crate::manifest::PluginCommand;
use crate::permission::Permission;
use crate::rpc::RpcError;

fn text_of(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

impl Plugins {
    pub(crate) fn call(&self, id: &str, method: String, params: Obj) -> BoxFuture<'static, Result<Value, RpcError>> {
        let ready = |r: Result<Value, RpcError>| -> BoxFuture<'static, Result<Value, RpcError>> {
            Box::pin(async move { r })
        };
        match method.as_str() {
            "log" => {
                self.note(id, &text_of(params.get("message")));
                return ready(Ok(Value::Null));
            }
            "settings.get" => return ready(Ok(Value::Object(self.settings_of(id)))),
            "command.busy" => return ready(self.command_busy(id, &params)),
            _ => {}
        }
        let calls = self.inner.calls.read().unwrap().clone();
        let (Some(calls), Some(info)) = (calls, self.get(id)) else {
            return ready(Err(RpcError::window_not_ready()));
        };
        calls.call(info, method, params)
    }

    fn command_busy(&self, id: &str, params: &Obj) -> Result<Value, RpcError> {
        let Some(Value::String(cmd)) = params.get("command") else {
            return Err(RpcError::required("command"));
        };
        let declared = self
            .reg()
            .entry(id)
            .and_then(|e| e.manifest.clone())
            .is_some_and(|m| m.commands.iter().any(|c| &c.id == cmd));
        if !declared {
            return Err(RpcError::invalid(format!("o plugin não tem o comando \"{cmd}\"")));
        }
        self.set_busy(&format!("{id}/{cmd}"), params.get("busy") == Some(&Value::Bool(true)));
        Ok(Value::Null)
    }

    async fn deliver(&self, id: &str, method: &str, params: Value) {
        if let Some(conn) = self.start(id).await
            && conn.ready()
        {
            conn.notify(method, Some(params));
        }
    }

    fn deliver_detached(&self, id: &str, method: &'static str, params: Value) {
        let me = self.clone();
        let id = id.to_owned();
        self.spawn(async move { me.deliver(&id, method, params).await });
    }

    pub fn emit(&self, kind: &str, data: Obj, activations: &[&str], needs: Option<Permission>) {
        let targets: Vec<String> = self
            .reg()
            .all
            .iter()
            .filter(|e| {
                let Some(m) = e.manifest.as_ref().filter(|m| e.enabled && m.has_process()) else {
                    return false;
                };
                if needs.is_some_and(|p| !m.allows(p)) {
                    return false;
                }
                let up = e.live_conn().is_some() || e.starting;
                up || (e.crash.is_none() && activations.iter().any(|a| m.activates_on(a)))
            })
            .map(|e| e.id())
            .collect();
        for id in targets {
            self.deliver_detached(&id, "event", event_params(kind, &data));
        }
    }

    pub fn wake(&self, id: &str) {
        {
            let mut reg = self.reg();
            let Some(e) = reg.entry_mut(id) else { return };
            if !e.active() || !e.manifest.as_ref().is_some_and(|m| m.has_process()) {
                return;
            }
            e.crash = None;
        }
        self.start_detached(id);
    }

    pub fn tell(&self, id: &str, kind: &str, data: Obj) {
        if self.is_up(id) {
            self.deliver_detached(id, "event", event_params(kind, &data));
        }
    }

    pub fn view_action(&self, plugin_id: &str, view_id: &str, action: &str, values: Obj) {
        if !self.get(plugin_id).is_some_and(|p| p.active()) {
            return;
        }
        let mut p = Map::new();
        p.insert("viewId".into(), Value::from(view_id));
        p.insert("action".into(), Value::from(action));
        p.insert("values".into(), Value::Object(values));
        self.deliver_detached(plugin_id, "view.action", Value::Object(p));
    }

    async fn request_revived(&self, id: &str, log: String, method: &str, params: Value) -> Result<Value, RpcError> {
        {
            let mut reg = self.reg();
            if let Some(e) = reg.entry_mut(id) {
                e.crash = None;
            }
        }
        self.note(id, &log);
        let conn = self.start(id).await.filter(|c| c.ready());
        let Some(conn) = conn else {
            let crash = self.get(id).and_then(|p| p.crash);
            return Err(RpcError::unavailable(crash.unwrap_or_else(|| "o plugin não subiu".into())));
        };
        conn.request(method, Some(params), REQUEST_TIMEOUT).await
    }

    pub async fn invoke(&self, command: &PluginCommand, context: Obj) -> Result<Value, RpcError> {
        let id = &command.plugin_id;
        if !self.get(id).is_some_and(|p| p.active()) {
            return Err(RpcError::not_enabled());
        }
        let mut params = Map::new();
        params.insert("command".into(), Value::from(command.id.clone()));
        params.insert("context".into(), Value::Object(context));
        let log = format!("comando {}", command.id);
        let out = self.request_revived(id, log, "command.invoke", Value::Object(params)).await;
        if let Err(e) = &out {
            self.note(id, &format!("comando {} falhou: {}", command.id, e.message));
        }
        out
    }

    pub async fn offer_suggestion(&self, id: &str, params: Value) -> Result<(), RpcError> {
        let taker = self.get(id).filter(|p| p.active() && p.manifest.as_ref().is_some_and(|m| m.suggestions.is_some()));
        if taker.is_none() {
            return Err(RpcError::not_enabled());
        }
        let sid = params.get("suggestion").map(|s| text_of(s.get("id")));
        let log = format!("sugestão {}", sid.filter(|s| !s.is_empty()).unwrap_or_else(|| "null".into()));
        self.request_revived(id, log, "suggestion.start", params).await.map(|_| ())
    }

}

fn event_params(kind: &str, data: &Obj) -> Value {
    let mut p = Map::new();
    p.insert("type".into(), Value::from(kind));
    for (k, v) in data {
        p.insert(k.clone(), v.clone());
    }
    Value::Object(p)
}

impl PluginBus for Plugins {
    fn emit(&self, event: &str, data: Value, activations: &[&str], permission: Option<&str>) {
        let data = match data {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        let needs = permission.and_then(Permission::by_id);
        Plugins::emit(self, event, data, activations, needs);
    }
}
