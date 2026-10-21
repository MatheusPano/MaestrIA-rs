use chrono::NaiveDate;
use mx_plugins::host_json::{ChatJson, FeatureJson, FolderJson, SessionJson, WorkspaceJson};
use mx_plugins::rpc::{
    CreatedSession, FloatContent, HostResult, OpenClaude, OpenShell, PluginHostApi, RpcError, SendText,
    SidebarUpdate, SuggestionCreated, TabOwner, ViewOpen, ViewUpdate,
};
use mx_plugins::ui::dialogs::{FormAnswer, InputRequest, PickRequest};
use mx_plugins::ui::form::PluginForm;
use mx_plugins::PluginInfo;
use serde_json::Value;
use tokio::sync::oneshot;

use super::{DialogAsk, Reply};
use crate::msg::{StoreMsg, StoreSender};
use crate::store::Store;

pub struct PluginHost {
    sender: StoreSender,
}

impl PluginHost {
    pub fn new(sender: StoreSender) -> PluginHost {
        PluginHost { sender }
    }

    fn ask<T, F>(&self, job: F) -> HostResult<'_, T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Store, Reply<T>) + Send + 'static,
    {
        let (reply, answer) = oneshot::channel();
        self.sender.send(StoreMsg::Run(Box::new(move |s: &mut Store| job(s, reply))));
        Box::pin(async move { answer.await.unwrap_or_else(|_| Err(RpcError::window_not_ready())) })
    }

    fn run<T, F>(&self, job: F) -> HostResult<'_, T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Store) -> Result<T, RpcError> + Send + 'static,
    {
        self.ask(move |s, reply| {
            let _ = reply.send(job(s));
        })
    }
}

impl PluginHostApi for PluginHost {
    fn show_banner(&self, _: &PluginInfo, text: String, sticky: bool) -> HostResult<'_, ()> {
        self.run(move |s| {
            s.show_banner_with(&text, sticky);
            Ok(())
        })
    }

    fn notify(&self, _: &PluginInfo, title: String, body: String) -> HostResult<'_, ()> {
        self.run(move |s| {
            s.p.notifier.alert(&title, &body);
            Ok(())
        })
    }

    fn open_url(&self, _: &PluginInfo, url: String) -> HostResult<'_, ()> {
        self.run(move |s| {
            s.p.notifier.open_link(&url);
            Ok(())
        })
    }

    fn pick(&self, plugin: &PluginInfo, request: PickRequest) -> HostResult<'_, Option<String>> {
        let p = plugin.clone();
        self.ask(move |s, reply| s.ask_text(p, DialogAsk::Pick(request), reply))
    }

    fn input(&self, plugin: &PluginInfo, request: InputRequest) -> HostResult<'_, Option<String>> {
        let p = plugin.clone();
        self.ask(move |s, reply| s.ask_text(p, DialogAsk::Input(request), reply))
    }

    fn form(&self, plugin: &PluginInfo, form: PluginForm) -> HostResult<'_, Option<FormAnswer>> {
        let p = plugin.clone();
        self.ask(move |s, reply| s.ask_form(p, form, reply))
    }

    fn clipboard_write(&self, _: &PluginInfo, text: String) -> HostResult<'_, ()> {
        self.run(move |s| {
            s.plugin.effects.push(super::Effect::Clipboard(text));
            Ok(())
        })
    }

    fn sessions(&self, _: &PluginInfo) -> HostResult<'_, Vec<SessionJson>> {
        self.run(|s| Ok(s.session_list()))
    }

    fn focused_session(&self, _: &PluginInfo) -> HostResult<'_, Option<SessionJson>> {
        self.run(|s| Ok(s.focused_tab_id().and_then(|id| s.tab(&id).map(|t| s.session_json(t)))))
    }

    fn focus_session(&self, _: &PluginInfo, tab_id: Option<String>) -> HostResult<'_, ()> {
        self.run(move |s| s.plugin_focus(tab_id))
    }

    fn folders(&self, _: &PluginInfo) -> HostResult<'_, Vec<FolderJson>> {
        self.run(|s| Ok(s.folder_list()))
    }

    fn features_or_hotfixes(&self, _: &PluginInfo) -> HostResult<'_, Vec<FeatureJson>> {
        self.run(|s| Ok(s.feature_list()))
    }

    fn workspaces(&self, _: &PluginInfo) -> HostResult<'_, Vec<WorkspaceJson>> {
        self.run(|s| Ok(s.workspace_list()))
    }

    fn chats(&self, _: &PluginInfo, day: NaiveDate) -> HostResult<'_, Vec<ChatJson>> {
        let (reply, answer) = oneshot::channel();
        self.sender.send(StoreMsg::Run(Box::new(move |s: &mut Store| {
            let _ = reply.send(s.chat_lister());
        })));
        Box::pin(async move {
            let lister = answer.await.map_err(|_| RpcError::window_not_ready())?;
            Ok(lister.list(day).await)
        })
    }

    fn open_in_editor(&self, _: &PluginInfo, path: String, line: Option<i64>) -> HostResult<'_, ()> {
        self.run(move |s| {
            s.plugin_editor(&path, line);
            Ok(())
        })
    }

    fn send_text(&self, _: &PluginInfo, request: SendText) -> HostResult<'_, ()> {
        self.run(move |s| s.plugin_send_text(request))
    }

    fn open_claude(&self, _: &PluginInfo, request: OpenClaude) -> HostResult<'_, String> {
        self.run(move |s| Ok(s.plugin_open_claude(request)))
    }

    fn open_shell(&self, plugin: &PluginInfo, request: OpenShell) -> HostResult<'_, String> {
        let p = plugin.id.clone();
        self.run(move |s| Ok(s.plugin_open_shell(&p, request)))
    }

    fn tab_owner(&self, _: &PluginInfo, tab_id: Option<String>) -> HostResult<'_, TabOwner> {
        self.run(move |s| s.plugin_tab_owner(tab_id))
    }

    fn close_tab(&self, _: &PluginInfo, tab_id: String) -> HostResult<'_, ()> {
        self.run(move |s| {
            s.close_tab(&tab_id);
            Ok(())
        })
    }

    fn suggestion_created(&self, plugin: &PluginInfo, request: SuggestionCreated) -> HostResult<'_, Option<CreatedSession>> {
        let p = plugin.id.clone();
        self.ask(move |s, reply| s.plugin_suggestion_created(&p, request, reply))
    }

    fn open_markdown(&self, plugin: &PluginInfo, title: String, markdown: String) -> HostResult<'_, String> {
        let name = plugin.name().to_owned();
        self.run(move |s| Ok(s.plugin_markdown(&name, title, markdown)))
    }

    fn view_open(&self, plugin: &PluginInfo, request: ViewOpen) -> HostResult<'_, String> {
        let p = plugin.clone();
        self.run(move |s| Ok(s.view_open(&p, request)))
    }

    fn view_update(&self, plugin: &PluginInfo, request: ViewUpdate) -> HostResult<'_, bool> {
        let p = plugin.id.clone();
        self.run(move |s| Ok(s.view_update(&p, request)))
    }

    fn view_append_lines(&self, plugin: &PluginInfo, view_id: String, console_id: String, lines: Value) -> HostResult<'_, bool> {
        let p = plugin.id.clone();
        self.run(move |s| Ok(s.view_lines(&p, &view_id, &console_id, Some(&lines))))
    }

    fn view_clear_lines(&self, plugin: &PluginInfo, view_id: String, console_id: String) -> HostResult<'_, ()> {
        let p = plugin.id.clone();
        self.run(move |s| {
            s.view_lines(&p, &view_id, &console_id, None);
            Ok(())
        })
    }

    fn view_close(&self, plugin: &PluginInfo, view_id: String) -> HostResult<'_, ()> {
        let p = plugin.id.clone();
        self.run(move |s| {
            s.view_close(&p, &view_id);
            Ok(())
        })
    }

    fn sidebar_update(&self, plugin: &PluginInfo, request: SidebarUpdate) -> HostResult<'_, bool> {
        let p = plugin.clone();
        self.run(move |s| Ok(s.sidebar_update(&p, request)))
    }

    fn float_show(&self, plugin: &PluginInfo, request: FloatContent) -> HostResult<'_, ()> {
        let p = plugin.clone();
        self.run(move |s| {
            s.float_show(&p, &request);
            Ok(())
        })
    }

    fn float_update(&self, plugin: &PluginInfo, request: FloatContent) -> HostResult<'_, bool> {
        let p = plugin.id.clone();
        self.run(move |s| Ok(s.float_update(&p, &request)))
    }

    fn float_hide(&self, plugin: &PluginInfo, id: String) -> HostResult<'_, ()> {
        let p = plugin.id.clone();
        self.run(move |s| {
            s.float_hide(&p, &id);
            Ok(())
        })
    }
}
