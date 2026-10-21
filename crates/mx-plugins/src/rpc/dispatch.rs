use std::sync::Arc;

use chrono::NaiveDate;
use mx_core::json::Obj;
use mx_core::ports::BoxFuture;
use serde_json::{Map, Value};

use super::params::{badge, blocks_from, blocks_if_present, is_true, opt_num, opt_text, text};
use super::{
    FloatContent, OpenClaude, OpenShell, PluginHostApi, RpcError, SendText, SidebarUpdate, SuggestionCreated,
    ViewOpen, ViewUpdate, parse_rfw,
};
use crate::SIDEBAR_VIEW_ID;
use crate::floats::FloatCorner;
use crate::permission::Permission;
use crate::state::PluginInfo;
use crate::ui::dialogs::{InputRequest, PickRequest};
use crate::ui::form::PluginForm;

pub trait PluginCalls: Send + Sync {
    fn call(&self, plugin: PluginInfo, method: String, params: Obj) -> BoxFuture<'static, Result<Value, RpcError>>;
}

pub struct HostDispatcher {
    api: Arc<dyn PluginHostApi>,
    can_ask: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl HostDispatcher {
    pub fn new(api: Arc<dyn PluginHostApi>) -> Self {
        Self { api, can_ask: Arc::new(|| true) }
    }

    pub fn with_window_probe(mut self, can_ask: Arc<dyn Fn() -> bool + Send + Sync>) -> Self {
        self.can_ask = can_ask;
        self
    }

    pub async fn dispatch(&self, p: &PluginInfo, method: &str, params: &Obj) -> Result<Value, RpcError> {
        dispatch(&*self.api, &*self.can_ask, p, method, params).await
    }
}

impl PluginCalls for HostDispatcher {
    fn call(&self, plugin: PluginInfo, method: String, params: Obj) -> BoxFuture<'static, Result<Value, RpcError>> {
        let api = self.api.clone();
        let can_ask = self.can_ask.clone();
        Box::pin(async move { dispatch(&*api, &*can_ask, &plugin, &method, &params).await })
    }
}

fn need(p: &PluginInfo, perm: Permission) -> Result<(), RpcError> {
    if p.allows(perm) { Ok(()) } else { Err(RpcError::missing_permission(perm)) }
}

fn obj(pairs: &[(&str, Value)]) -> Value {
    Value::Object(pairs.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect::<Map<_, _>>())
}

fn title_or_name(params: &Obj, p: &PluginInfo) -> String {
    opt_text(params, "title").unwrap_or_else(|| p.name().to_owned())
}

fn parse_day(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .ok()
        .or_else(|| mx_core::text::parse_iso8601(raw).map(|t| t.date_naive()))
}

fn float_content(params: &Obj, corner: Option<FloatCorner>) -> Result<FloatContent, RpcError> {
    Ok(FloatContent {
        id: text(params, "id")?,
        width: opt_num(params, "width"),
        height: opt_num(params, "height"),
        corner,
        blocks: blocks_if_present(params),
        rfw: parse_rfw(params)?,
    })
}

async fn dispatch(
    api: &dyn PluginHostApi,
    can_ask: &(dyn Fn() -> bool + Send + Sync),
    p: &PluginInfo,
    method: &str,
    params: &Obj,
) -> Result<Value, RpcError> {
    let null = Ok(Value::Null);
    match method {
        "window.showBanner" => {
            let t = text(params, "text")?;
            api.show_banner(p, format!("{}: {t}", p.name()), is_true(params, "sticky")).await?;
            null
        }
        "window.notify" => {
            need(p, Permission::Notifications)?;
            let title = title_or_name(params, p);
            api.notify(p, title, text(params, "body")?).await?;
            null
        }
        "window.openUrl" => {
            let url = text(params, "url")?;
            if !url.starts_with("https://") && !url.starts_with("http://") {
                return Err(RpcError::invalid("só dá pra abrir http(s)"));
            }
            api.open_url(p, url).await?;
            null
        }
        "window.pick" => {
            if !can_ask() {
                return Err(RpcError::no_window());
            }
            let request = PickRequest::parse(params, p.name());
            if request.items.is_empty() {
                return null;
            }
            Ok(api.pick(p, request).await?.map_or(Value::Null, Value::from))
        }
        "window.input" => {
            if !can_ask() {
                return Err(RpcError::no_window());
            }
            let request = InputRequest::parse(params, p.name());
            Ok(api.input(p, request).await?.map_or(Value::Null, Value::from))
        }
        "window.form" => {
            if !can_ask() {
                return Err(RpcError::no_window());
            }
            let form = PluginForm::parse(params, p.name()).map_err(RpcError::invalid)?;
            Ok(api.form(p, form).await?.map_or(Value::Null, |a| a.to_json()))
        }
        "clipboard.write" => {
            api.clipboard_write(p, text(params, "text")?).await?;
            null
        }
        "sessions.list" => {
            let hooks = p.allows(Permission::Hooks);
            let all = api.sessions(p).await?;
            Ok(all.iter().map(|s| s.to_json(&p.id, hooks)).collect())
        }
        "sessions.focused" => {
            let hooks = p.allows(Permission::Hooks);
            Ok(api.focused_session(p).await?.map_or(Value::Null, |s| s.to_json(&p.id, hooks)))
        }
        "session.focus" => {
            api.focus_session(p, opt_text(params, "tabId")).await?;
            null
        }
        "folders.list" => Ok(api.folders(p).await?.iter().map(|f| f.to_json()).collect()),
        "featuresOrHotfixes.list" | "projects.list" => {
            Ok(api.features_or_hotfixes(p).await?.iter().map(|f| f.to_json()).collect())
        }
        "workspaces.list" => Ok(api.workspaces(p).await?.iter().map(|w| w.to_json()).collect()),
        "chats.list" => {
            need(p, Permission::Hooks)?;
            let day = parse_day(&text(params, "day")?).ok_or_else(|| RpcError::invalid("\"day\" é AAAA-MM-DD"))?;
            Ok(api.chats(p, day).await?.iter().map(|c| c.to_json()).collect())
        }
        "editor.open" => {
            let path = text(params, "path")?;
            let line = opt_num(params, "line").map(|n| n.trunc() as i64);
            api.open_in_editor(p, path, line).await?;
            null
        }
        "terminal.sendText" => {
            need(p, Permission::TerminalWrite)?;
            let request = SendText {
                text: text(params, "text")?,
                submit: is_true(params, "submit"),
                tab_id: opt_text(params, "tabId"),
            };
            api.send_text(p, request).await?;
            null
        }
        "session.openClaude" => {
            need(p, Permission::SessionsCreate)?;
            let request = OpenClaude {
                cwd: opt_text(params, "cwd"),
                prompt: opt_text(params, "prompt"),
                label: opt_text(params, "label"),
            };
            let tab = api.open_claude(p, request).await?;
            Ok(obj(&[("tabId", Value::from(tab))]))
        }
        "session.openShell" => {
            need(p, Permission::SessionsCreate)?;
            let embedded = is_true(params, "embedded");
            let request = OpenShell {
                cwd: opt_text(params, "cwd"),
                command: opt_text(params, "command"),
                label: opt_text(params, "label").filter(|l| !l.is_empty()),
                owned: is_true(params, "owned") || embedded,
                tag: opt_text(params, "tag"),
                embedded,
            };
            let tab = api.open_shell(p, request).await?;
            Ok(obj(&[("tabId", Value::from(tab))]))
        }
        "session.close" => {
            let tab = api.tab_owner(p, opt_text(params, "tabId")).await?;
            if tab.owner.as_deref() != Some(p.id.as_str()) {
                return Err(RpcError::not_owner());
            }
            api.close_tab(p, tab.tab_id).await?;
            null
        }
        "suggestion.created" => {
            if p.manifest.as_ref().is_none_or(|m| m.suggestions.is_none()) {
                return Err(RpcError::forbidden(
                    "só quem declara \"contributes.suggestions\" recebe tarefas sugeridas",
                ));
            }
            let (Some(Value::String(id)), Some(Value::String(reference))) =
                (params.get("suggestionId"), params.get("ref"))
            else {
                return Err(RpcError::invalid("faltam \"suggestionId\" e \"ref\""));
            };
            if reference.trim().is_empty() {
                return Err(RpcError::invalid("faltam \"suggestionId\" e \"ref\""));
            }
            let request = SuggestionCreated {
                suggestion_id: id.clone(),
                reference: reference.trim().to_owned(),
                intro: opt_text(params, "intro").unwrap_or_default(),
                branch: opt_text(params, "branch"),
                dir: opt_text(params, "dir"),
            };
            Ok(match api.suggestion_created(p, request).await? {
                Some(s) => obj(&[("tabId", Value::from(s.tab_id)), ("cwd", Value::from(s.cwd))]),
                None => Value::Null,
            })
        }
        "pane.openMarkdown" => {
            let title = title_or_name(params, p);
            let tab = api.open_markdown(p, title, text(params, "markdown")?).await?;
            Ok(obj(&[("tabId", Value::from(tab))]))
        }
        _ => dispatch_views(api, p, method, params).await,
    }
}

async fn dispatch_views(
    api: &dyn PluginHostApi,
    p: &PluginInfo,
    method: &str,
    params: &Obj,
) -> Result<Value, RpcError> {
    match method {
        "sidebar.update" => {
            if !p.manifest.as_ref().is_some_and(|m| m.sidebar) {
                return Err(RpcError::invalid("o manifesto não declara \"contributes.sidebar\""));
            }
            let request =
                SidebarUpdate { blocks: blocks_if_present(params), rfw: parse_rfw(params)?, badge: badge(params) };
            let shown = api.sidebar_update(p, request).await?;
            Ok(obj(&[("shown", Value::from(shown))]))
        }
        "view.open" => {
            let view_id = text(params, "viewId")?;
            if view_id == SIDEBAR_VIEW_ID {
                return Err(RpcError::invalid("\"sidebar\" é o id da aba da lateral — use sidebar.update"));
            }
            let request = ViewOpen {
                view_id,
                title: title_or_name(params, p),
                blocks: blocks_from(params.get("blocks")),
                rfw: parse_rfw(params)?,
                modal: is_true(params, "modal"),
            };
            let tab = api.view_open(p, request).await?;
            Ok(obj(&[("tabId", Value::from(tab))]))
        }
        "view.update" => {
            let request = ViewUpdate {
                view_id: text(params, "viewId")?,
                title: opt_text(params, "title"),
                blocks: blocks_if_present(params),
                rfw: parse_rfw(params)?,
            };
            let open = api.view_update(p, request).await?;
            Ok(obj(&[("open", Value::from(open))]))
        }
        "view.appendLines" => {
            let (view, console) = (text(params, "viewId")?, text(params, "id")?);
            let lines = params.get("lines").cloned().unwrap_or(Value::Null);
            let open = api.view_append_lines(p, view, console, lines).await?;
            Ok(obj(&[("open", Value::from(open))]))
        }
        "view.clearLines" => {
            let (view, console) = (text(params, "viewId")?, text(params, "id")?);
            api.view_clear_lines(p, view, console).await?;
            Ok(Value::Null)
        }
        "view.close" => {
            api.view_close(p, text(params, "viewId")?).await?;
            Ok(Value::Null)
        }
        "float.show" => {
            let corner = match params.get("corner") {
                None | Some(Value::Null) => None,
                Some(raw) => Some(FloatCorner::parse(raw).ok_or_else(|| {
                    RpcError::invalid("\"corner\" é topLeft, topRight, bottomLeft ou bottomRight")
                })?),
            };
            api.float_show(p, float_content(params, corner)?).await?;
            Ok(obj(&[("shown", Value::Bool(true))]))
        }
        "float.update" => {
            let shown = api.float_update(p, float_content(params, None)?).await?;
            Ok(obj(&[("shown", Value::from(shown))]))
        }
        "float.hide" => {
            api.float_hide(p, text(params, "id")?).await?;
            Ok(Value::Null)
        }
        _ => Err(RpcError::method_not_found(method)),
    }
}
