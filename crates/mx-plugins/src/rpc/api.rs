use chrono::NaiveDate;
use mx_core::json::Obj;
use mx_core::model::TabKind;
use mx_core::ports::{BoxFuture, TermKey};
use serde_json::Value;

use super::{BadgeUpdate, RfwUpdate, RpcError};
use crate::floats::FloatCorner;
use crate::host_json::{ChatJson, FeatureJson, FolderJson, SessionJson, WorkspaceJson};
use crate::state::PluginInfo;
use crate::ui::dialogs::{FormAnswer, InputRequest, PickRequest};
use crate::ui::form::PluginForm;

pub type HostResult<'a, T> = BoxFuture<'a, Result<T, RpcError>>;

#[derive(Debug, Clone, PartialEq)]
pub struct SendText {
    pub text: String,
    pub submit: bool,
    pub tab_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SendKeys {
    pub tab_id: Option<String>,
    pub keys: Option<Vec<TermKey>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenClaude {
    pub cwd: Option<String>,
    pub prompt: Option<String>,
    pub label: Option<String>,
    pub background: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenShell {
    pub cwd: Option<String>,
    pub command: Option<String>,
    pub label: Option<String>,
    pub owned: bool,
    pub tag: Option<String>,
    pub embedded: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SuggestionCreated {
    pub suggestion_id: String,
    pub reference: String,
    pub intro: String,
    pub branch: Option<String>,
    pub dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreatedSession {
    pub tab_id: String,
    pub cwd: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ViewOpen {
    pub view_id: String,
    pub title: String,
    pub blocks: Vec<Obj>,
    pub rfw: Option<RfwUpdate>,
    pub modal: bool,
    pub from: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewUpdate {
    pub view_id: String,
    pub title: Option<String>,
    pub blocks: Option<Vec<Obj>>,
    pub rfw: Option<RfwUpdate>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SidebarUpdate {
    pub blocks: Option<Vec<Obj>>,
    pub rfw: Option<RfwUpdate>,
    pub badge: BadgeUpdate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FloatContent {
    pub id: String,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub corner: Option<FloatCorner>,
    pub blocks: Option<Vec<Obj>>,
    pub rfw: Option<RfwUpdate>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TabOwner {
    pub tab_id: String,
    pub kind: TabKind,
    pub owner: Option<String>,
}

pub trait PluginHostApi: Send + Sync {
    fn show_banner(&self, plugin: &PluginInfo, text: String, sticky: bool) -> HostResult<'_, ()>;
    fn notify(&self, plugin: &PluginInfo, title: String, body: String) -> HostResult<'_, ()>;
    fn open_url(&self, plugin: &PluginInfo, url: String) -> HostResult<'_, ()>;
    fn pick(&self, plugin: &PluginInfo, request: PickRequest) -> HostResult<'_, Option<String>>;
    fn input(&self, plugin: &PluginInfo, request: InputRequest) -> HostResult<'_, Option<String>>;
    fn form(&self, plugin: &PluginInfo, form: PluginForm) -> HostResult<'_, Option<FormAnswer>>;
    fn clipboard_write(&self, plugin: &PluginInfo, text: String) -> HostResult<'_, ()>;
    fn sessions(&self, plugin: &PluginInfo) -> HostResult<'_, Vec<SessionJson>>;
    fn focused_session(&self, plugin: &PluginInfo) -> HostResult<'_, Option<SessionJson>>;
    fn focus_session(&self, plugin: &PluginInfo, tab_id: Option<String>) -> HostResult<'_, ()>;
    fn folders(&self, plugin: &PluginInfo) -> HostResult<'_, Vec<FolderJson>>;
    fn features_or_hotfixes(&self, plugin: &PluginInfo) -> HostResult<'_, Vec<FeatureJson>>;
    fn workspaces(&self, plugin: &PluginInfo) -> HostResult<'_, Vec<WorkspaceJson>>;
    fn chats(&self, plugin: &PluginInfo, day: NaiveDate) -> HostResult<'_, Vec<ChatJson>>;
    fn open_in_editor(&self, plugin: &PluginInfo, path: String, line: Option<i64>) -> HostResult<'_, ()>;
    fn send_text(&self, plugin: &PluginInfo, request: SendText) -> HostResult<'_, ()>;
    fn open_claude(&self, plugin: &PluginInfo, request: OpenClaude) -> HostResult<'_, String>;
    fn open_shell(&self, plugin: &PluginInfo, request: OpenShell) -> HostResult<'_, String>;
    fn send_keys(&self, plugin: &PluginInfo, request: SendKeys) -> HostResult<'_, ()>;
    fn paste_clipboard(&self, plugin: &PluginInfo, tab_id: Option<String>) -> HostResult<'_, ()>;
    fn rename_session(&self, plugin: &PluginInfo, tab_id: String, title: String) -> HostResult<'_, String>;
    fn wake_session(&self, plugin: &PluginInfo, tab_id: String) -> HostResult<'_, bool>;
    fn hibernate_session(&self, plugin: &PluginInfo, tab_id: String) -> HostResult<'_, bool>;
    fn set_portrait(&self, plugin: &PluginInfo, tab_id: String, image: Option<Value>) -> HostResult<'_, ()>;
    fn tab_owner(&self, plugin: &PluginInfo, tab_id: String) -> HostResult<'_, TabOwner>;
    fn close_tab(&self, plugin: &PluginInfo, tab_id: String) -> HostResult<'_, ()>;
    fn suggestion_created(
        &self,
        plugin: &PluginInfo,
        request: SuggestionCreated,
    ) -> HostResult<'_, Option<CreatedSession>>;
    fn open_markdown(&self, plugin: &PluginInfo, title: String, markdown: String) -> HostResult<'_, String>;
    fn view_open(&self, plugin: &PluginInfo, request: ViewOpen) -> HostResult<'_, String>;
    fn view_update(&self, plugin: &PluginInfo, request: ViewUpdate) -> HostResult<'_, bool>;
    fn view_append_lines(
        &self,
        plugin: &PluginInfo,
        view_id: String,
        console_id: String,
        lines: Value,
    ) -> HostResult<'_, bool>;
    fn view_clear_lines(&self, plugin: &PluginInfo, view_id: String, console_id: String) -> HostResult<'_, ()>;
    fn view_close(&self, plugin: &PluginInfo, view_id: String) -> HostResult<'_, ()>;
    fn sidebar_update(&self, plugin: &PluginInfo, request: SidebarUpdate) -> HostResult<'_, bool>;
    fn float_show(&self, plugin: &PluginInfo, request: FloatContent) -> HostResult<'_, ()>;
    fn float_update(&self, plugin: &PluginInfo, request: FloatContent) -> HostResult<'_, bool>;
    fn float_hide(&self, plugin: &PluginInfo, id: String) -> HostResult<'_, ()>;
}
