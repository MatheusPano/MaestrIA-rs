use mx_core::json::Obj;
use mx_core::model::TabKind;
use serde_json::Value;

use super::dispatch::{need, obj};
use super::keys::parse_keys;
use super::params::{is_true, opt_text, text};
use super::{OpenClaude, PluginHostApi, RpcError, SendKeys, SendText};
use crate::permission::Permission;
use crate::state::PluginInfo;

fn tab_id(params: &Obj) -> Result<String, RpcError> {
    match params.get("tabId") {
        Some(Value::String(id)) => Ok(id.clone()),
        _ => Err(RpcError::required("tabId")),
    }
}

fn close_refused(claude: bool) -> RpcError {
    if claude {
        RpcError::forbidden("fechar uma sessão do claude pede a permissão \"sessions.close\"")
    } else {
        RpcError::not_owner()
    }
}

pub(super) async fn dispatch_sessions(
    api: &dyn PluginHostApi,
    p: &PluginInfo,
    method: &str,
    params: &Obj,
) -> Option<Result<Value, RpcError>> {
    Some(match method {
        "terminal.sendText" => send_text(api, p, params).await,
        "terminal.sendKeys" => send_keys(api, p, params).await,
        "terminal.pasteClipboard" => paste_clipboard(api, p, params).await,
        "session.openClaude" => open_claude(api, p, params).await,
        "session.rename" => rename(api, p, params).await,
        "session.wake" => wake(api, p, params).await,
        "session.hibernate" => hibernate(api, p, params).await,
        "session.portrait" => portrait(api, p, params).await,
        "session.close" => close(api, p, params).await,
        _ => return None,
    })
}

async fn send_text(api: &dyn PluginHostApi, p: &PluginInfo, params: &Obj) -> Result<Value, RpcError> {
    need(p, Permission::TerminalWrite)?;
    let request = SendText { text: text(params, "text")?, submit: is_true(params, "submit"), tab_id: opt_text(params, "tabId") };
    api.send_text(p, request).await?;
    Ok(Value::Null)
}

async fn send_keys(api: &dyn PluginHostApi, p: &PluginInfo, params: &Obj) -> Result<Value, RpcError> {
    need(p, Permission::TerminalWrite)?;
    let request = SendKeys { tab_id: opt_text(params, "tabId"), keys: parse_keys(params.get("keys")) };
    api.send_keys(p, request).await?;
    Ok(Value::Null)
}

async fn paste_clipboard(api: &dyn PluginHostApi, p: &PluginInfo, params: &Obj) -> Result<Value, RpcError> {
    need(p, Permission::TerminalWrite)?;
    api.paste_clipboard(p, opt_text(params, "tabId")).await?;
    Ok(Value::Null)
}

async fn open_claude(api: &dyn PluginHostApi, p: &PluginInfo, params: &Obj) -> Result<Value, RpcError> {
    need(p, Permission::SessionsCreate)?;
    let request = OpenClaude {
        cwd: opt_text(params, "cwd"),
        prompt: opt_text(params, "prompt"),
        label: opt_text(params, "label"),
        background: is_true(params, "background"),
    };
    Ok(obj(&[("tabId", Value::from(api.open_claude(p, request).await?))]))
}

async fn rename(api: &dyn PluginHostApi, p: &PluginInfo, params: &Obj) -> Result<Value, RpcError> {
    need(p, Permission::SessionsCreate)?;
    let id = tab_id(params)?;
    let title = text(params, "title")?;
    Ok(obj(&[("title", Value::from(api.rename_session(p, id, title).await?))]))
}

async fn wake(api: &dyn PluginHostApi, p: &PluginInfo, params: &Obj) -> Result<Value, RpcError> {
    need(p, Permission::SessionsCreate)?;
    let id = tab_id(params)?;
    Ok(obj(&[("woke", Value::from(api.wake_session(p, id).await?))]))
}

async fn hibernate(api: &dyn PluginHostApi, p: &PluginInfo, params: &Obj) -> Result<Value, RpcError> {
    need(p, Permission::SessionsClose)?;
    let id = tab_id(params)?;
    Ok(obj(&[("hibernated", Value::from(api.hibernate_session(p, id).await?))]))
}

async fn portrait(api: &dyn PluginHostApi, p: &PluginInfo, params: &Obj) -> Result<Value, RpcError> {
    let id = tab_id(params)?;
    let image = params.get("image").filter(|v| !v.is_null()).cloned();
    api.set_portrait(p, id, image).await?;
    Ok(Value::Null)
}

async fn close(api: &dyn PluginHostApi, p: &PluginInfo, params: &Obj) -> Result<Value, RpcError> {
    let id = tab_id(params)?;
    let tab = api.tab_owner(p, id).await?;
    let mine = tab.owner.as_deref() == Some(p.id.as_str());
    let claude = tab.kind == TabKind::Claude && tab.owner.is_none();
    let allowed = mine || (claude && p.allows(Permission::SessionsClose));
    if !allowed {
        return Err(close_refused(claude));
    }
    api.close_tab(p, tab.tab_id).await?;
    Ok(Value::Null)
}
