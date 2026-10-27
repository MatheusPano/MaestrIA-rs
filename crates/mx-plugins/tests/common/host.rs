use std::sync::Mutex;

use chrono::NaiveDate;
use mx_plugins::host_json::{ChatJson, FeatureJson, FolderJson, SessionJson, WorkspaceJson};
use mx_core::model::TabKind;
use mx_plugins::rpc::{
    CreatedSession, FloatContent, HostResult, OpenClaude, OpenShell, SendKeys, SendText, SidebarUpdate,
    SuggestionCreated, TabOwner, ViewOpen, ViewUpdate,
};
use mx_plugins::ui::dialogs::{FormAnswer, InputRequest, PickRequest};
use mx_plugins::ui::form::PluginForm;
use mx_plugins::{PluginHostApi, PluginInfo, RpcError};
use serde_json::Value;

#[derive(Default)]
pub struct FakeHost {
    pub log: Mutex<Vec<String>>,
    pub sessions: Vec<SessionJson>,
    pub workspaces: Vec<WorkspaceJson>,
    pub features: Vec<FeatureJson>,
    pub folders: Vec<FolderJson>,
    pub owners: Vec<(String, Option<String>)>,
    pub claude: Vec<String>,
    pub pick_last: bool,
    pub input_answer: Option<String>,
    pub float_shown: bool,
}

fn done<'a, T: Send + 'a>(v: Result<T, RpcError>) -> HostResult<'a, T> {
    Box::pin(async move { v })
}

impl FakeHost {
    fn rec(&self, line: String) {
        self.log.lock().unwrap().push(line);
    }

    pub fn calls(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

impl PluginHostApi for FakeHost {
    fn show_banner(&self, _: &PluginInfo, text: String, sticky: bool) -> HostResult<'_, ()> {
        self.rec(format!("banner {text} {sticky}"));
        done(Ok(()))
    }
    fn notify(&self, _: &PluginInfo, title: String, body: String) -> HostResult<'_, ()> {
        self.rec(format!("notify {title}|{body}"));
        done(Ok(()))
    }
    fn open_url(&self, _: &PluginInfo, url: String) -> HostResult<'_, ()> {
        self.rec(format!("url {url}"));
        done(Ok(()))
    }
    fn pick(&self, _: &PluginInfo, r: PickRequest) -> HostResult<'_, Option<String>> {
        let items: Vec<String> =
            r.items.iter().map(|i| format!("{}|{}", i.label, i.detail.as_deref().unwrap_or("null"))).collect();
        self.rec(format!("pick {} {}", r.title, items.join(",")));
        done(Ok(if self.pick_last { r.items.last().map(|i| i.value.clone()) } else { None }))
    }
    fn input(&self, _: &PluginInfo, r: InputRequest) -> HostResult<'_, Option<String>> {
        self.rec(format!("input {:?}", r));
        done(Ok(self.input_answer.clone()))
    }
    fn form(&self, _: &PluginInfo, f: PluginForm) -> HostResult<'_, Option<FormAnswer>> {
        let values = f.initial_values();
        let button = f.enter_button().cloned().unwrap();
        done(Ok(Some(FormAnswer { action: button.action, values })))
    }
    fn clipboard_write(&self, _: &PluginInfo, text: String) -> HostResult<'_, ()> {
        self.rec(format!("clipboard {text}"));
        done(Ok(()))
    }
    fn sessions(&self, _: &PluginInfo) -> HostResult<'_, Vec<SessionJson>> {
        done(Ok(self.sessions.clone()))
    }
    fn focused_session(&self, _: &PluginInfo) -> HostResult<'_, Option<SessionJson>> {
        done(Ok(self.sessions.first().cloned()))
    }
    fn focus_session(&self, _: &PluginInfo, tab_id: Option<String>) -> HostResult<'_, ()> {
        done(match tab_id {
            None => Err(RpcError::no_focused_tab()),
            Some(id) if !self.owners.iter().any(|(t, _)| *t == id) => Err(RpcError::no_tab(&id)),
            Some(_) => Ok(()),
        })
    }
    fn folders(&self, _: &PluginInfo) -> HostResult<'_, Vec<FolderJson>> {
        done(Ok(self.folders.clone()))
    }
    fn features_or_hotfixes(&self, _: &PluginInfo) -> HostResult<'_, Vec<FeatureJson>> {
        done(Ok(self.features.clone()))
    }
    fn workspaces(&self, _: &PluginInfo) -> HostResult<'_, Vec<WorkspaceJson>> {
        done(Ok(self.workspaces.clone()))
    }
    fn chats(&self, _: &PluginInfo, day: NaiveDate) -> HostResult<'_, Vec<ChatJson>> {
        self.rec(format!("chats {day}"));
        done(Ok(Vec::new()))
    }
    fn open_in_editor(&self, _: &PluginInfo, path: String, line: Option<i64>) -> HostResult<'_, ()> {
        self.rec(format!("editor {path} {line:?}"));
        done(Ok(()))
    }
    fn send_text(&self, _: &PluginInfo, r: SendText) -> HostResult<'_, ()> {
        self.rec(format!("send {:?}", r));
        done(Ok(()))
    }
    fn open_claude(&self, _: &PluginInfo, r: OpenClaude) -> HostResult<'_, String> {
        self.rec(format!("claude {:?}", r));
        done(Ok("tab7".into()))
    }
    fn open_shell(&self, _: &PluginInfo, r: OpenShell) -> HostResult<'_, String> {
        self.rec(format!("shell {:?}", r));
        done(Ok("tab8".into()))
    }
    fn send_keys(&self, _: &PluginInfo, r: SendKeys) -> HostResult<'_, ()> {
        self.rec(format!("keys {:?}", r));
        done(Ok(()))
    }
    fn paste_clipboard(&self, _: &PluginInfo, tab_id: Option<String>) -> HostResult<'_, ()> {
        self.rec(format!("paste {tab_id:?}"));
        done(Ok(()))
    }
    fn rename_session(&self, _: &PluginInfo, tab_id: String, title: String) -> HostResult<'_, String> {
        self.rec(format!("rename {tab_id} {title}"));
        done(Ok(title.trim().to_owned()))
    }
    fn wake_session(&self, _: &PluginInfo, tab_id: String) -> HostResult<'_, bool> {
        self.rec(format!("wake {tab_id}"));
        done(Ok(true))
    }
    fn hibernate_session(&self, _: &PluginInfo, tab_id: String) -> HostResult<'_, bool> {
        self.rec(format!("hibernate {tab_id}"));
        done(Ok(true))
    }
    fn set_portrait(&self, _: &PluginInfo, tab_id: String, image: Option<Value>) -> HostResult<'_, ()> {
        self.rec(format!("portrait {tab_id} {image:?}"));
        done(Ok(()))
    }
    fn tab_owner(&self, _: &PluginInfo, id: String) -> HostResult<'_, TabOwner> {
        let kind = if self.claude.contains(&id) { TabKind::Claude } else { TabKind::Shell };
        done(match self.owners.iter().find(|(t, _)| *t == id) {
            Some((t, o)) => Ok(TabOwner { tab_id: t.clone(), kind, owner: o.clone() }),
            None => Err(RpcError::no_tab(&id)),
        })
    }
    fn close_tab(&self, _: &PluginInfo, tab_id: String) -> HostResult<'_, ()> {
        self.rec(format!("close {tab_id}"));
        done(Ok(()))
    }
    fn suggestion_created(&self, _: &PluginInfo, r: SuggestionCreated) -> HostResult<'_, Option<CreatedSession>> {
        self.rec(format!("created {:?}", r));
        done(Ok(Some(CreatedSession { tab_id: "a".into(), cwd: "/repo".into() })))
    }
    fn open_markdown(&self, _: &PluginInfo, title: String, markdown: String) -> HostResult<'_, String> {
        self.rec(format!("markdown {title}|{markdown}"));
        done(Ok("tab9".into()))
    }
    fn view_open(&self, _: &PluginInfo, r: ViewOpen) -> HostResult<'_, String> {
        self.rec(format!("view.open {} {} {} {}", r.view_id, r.title, r.blocks.len(), r.modal));
        if r.from.is_some() || r.width.is_some() || r.height.is_some() {
            self.rec(format!("view.open from {:?} {:?}x{:?}", r.from, r.width, r.height));
        }
        done(Ok("tab3".into()))
    }
    fn view_update(&self, _: &PluginInfo, r: ViewUpdate) -> HostResult<'_, bool> {
        self.rec(format!("view.update {} {:?}", r.view_id, r.rfw));
        done(Ok(r.view_id == "x"))
    }
    fn view_append_lines(&self, _: &PluginInfo, v: String, c: String, lines: Value) -> HostResult<'_, bool> {
        self.rec(format!("append {v} {c} {lines}"));
        done(Ok(true))
    }
    fn view_clear_lines(&self, _: &PluginInfo, v: String, c: String) -> HostResult<'_, ()> {
        self.rec(format!("clear {v} {c}"));
        done(Ok(()))
    }
    fn view_close(&self, _: &PluginInfo, v: String) -> HostResult<'_, ()> {
        self.rec(format!("view.close {v}"));
        done(Ok(()))
    }
    fn sidebar_update(&self, _: &PluginInfo, r: SidebarUpdate) -> HostResult<'_, bool> {
        self.rec(format!("sidebar {:?} {}", r.badge, r.blocks.map_or(0, |b| b.len())));
        done(Ok(false))
    }
    fn float_show(&self, _: &PluginInfo, r: FloatContent) -> HostResult<'_, ()> {
        self.rec(format!("float.show {} {:?}", r.id, r.corner));
        done(Ok(()))
    }
    fn float_update(&self, _: &PluginInfo, r: FloatContent) -> HostResult<'_, bool> {
        self.rec(format!("float.update {}", r.id));
        done(Ok(self.float_shown))
    }
    fn float_hide(&self, _: &PluginInfo, id: String) -> HostResult<'_, ()> {
        self.rec(format!("float.hide {id}"));
        done(Ok(()))
    }
}
