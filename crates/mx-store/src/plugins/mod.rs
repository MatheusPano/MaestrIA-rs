mod commands;
mod host;
mod screen;
mod sessions;
mod views;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use mx_core::model::Tab;
use mx_plugins::floats::{FloatSpot, PluginFloats};
use mx_plugins::rail::{self, RailBadge};
use mx_plugins::rpc::RpcError;
use mx_plugins::ui::dialogs::{FormAnswer, InputRequest, PickRequest};
use mx_plugins::ui::form::PluginForm;
use mx_plugins::ui::view::PluginView;
use mx_plugins::{HostDispatcher, PluginCommand, PluginInfo, PluginListener, Plugins};
use serde_json::Value;
use tokio::sync::oneshot;

pub use host::PluginHost;

use crate::changes::Areas;
use crate::msg::{StoreMsg, StoreSender};
use crate::store::Store;

pub(crate) type Reply<T> = oneshot::Sender<Result<T, RpcError>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Clipboard(String),
    Paste { tab_id: String, text: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DialogAsk {
    Pick(PickRequest),
    Input(InputRequest),
    Form(PluginForm),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DialogAnswer {
    Text(Option<String>),
    Form(Option<FormAnswer>),
}

pub struct PluginDialog {
    pub id: u64,
    pub plugin: PluginInfo,
    pub ask: DialogAsk,
    text: Option<Reply<Option<String>>>,
    form: Option<Reply<Option<FormAnswer>>>,
}

#[derive(Default)]
pub struct PluginUi {
    pub registry: Option<Plugins>,
    pub views: HashMap<String, PluginView>,
    pub modal: Option<(Tab, PluginView)>,
    pub sidebars: HashMap<String, PluginView>,
    pub badges: HashMap<String, String>,
    pub floats: PluginFloats,
    pub dialogs: VecDeque<PluginDialog>,
    pub effects: Vec<Effect>,
    pub(crate) created: HashMap<u64, Reply<Option<mx_plugins::rpc::CreatedSession>>>,
    pub(crate) next_reply: u64,
    pub(crate) screens: screen::Screens,
}

struct Listener(StoreSender);

impl PluginListener for Listener {
    fn changed(&self) {
        self.0.send(StoreMsg::Run(Box::new(|s: &mut Store| s.plugins_changed())));
    }

    fn ready(&self, plugin_id: &str) {
        let id = plugin_id.to_owned();
        self.0.send(StoreMsg::Run(Box::new(move |s: &mut Store| s.plugin_ready(&id))));
    }

    fn logged(&self, _plugin_id: &str) {
        self.0.send(StoreMsg::Run(Box::new(|s: &mut Store| s.mark(Areas::PLUGINS))));
    }
}

impl Store {
    pub fn plugin_host(&self) -> Arc<PluginHost> {
        Arc::new(PluginHost::new(self.sender()))
    }

    pub fn attach_plugins(&mut self, plugins: Plugins) {
        plugins.set_runtime(self.p.rt.clone());
        plugins.set_env(mx_sys::shell::env().iter().map(|(k, v)| (k.clone(), v.clone())).collect());
        plugins.set_listener(Arc::new(Listener(self.sender())));
        plugins.set_calls(Arc::new(HostDispatcher::new(self.plugin_host())));
        plugins.load(&Value::Object(self.prefs.plugins.clone()));
        plugins.scan();
        self.plugin.registry = Some(plugins);
        self.settle_theme();
        self.mark(Areas::PLUGINS);
    }

    pub fn start_plugins(&mut self) {
        let Some(r) = self.plugin.registry.clone() else { return };
        r.startup();
        self.land_screen();
        let live = self.live_sidebar();
        self.announce_sidebar(live, true);
    }

    pub fn plugins(&self) -> Option<&Plugins> {
        self.plugin.registry.as_ref()
    }

    pub fn plugin_ui(&self) -> &PluginUi {
        &self.plugin
    }

    pub(crate) fn plugin_info(&self, id: &str) -> Option<PluginInfo> {
        self.plugin.registry.as_ref()?.get(id)
    }

    pub(crate) fn plugins_changed(&mut self) {
        if let Some(r) = &self.plugin.registry {
            self.installed_plugins = r.all().into_iter().map(|p| p.id).collect();
        }
        self.sync_screen();
        self.mark(Areas::PLUGINS | Areas::SIDEBAR);
    }

    fn plugin_ready(&mut self, id: &str) {
        if self.sidebar_shown(id) && let Some(r) = &self.plugin.registry {
            r.tell(id, "sidebar.shown", Default::default());
        }
        self.mark(Areas::PLUGINS);
    }

    pub(crate) fn sync_plugin_prefs(&mut self) {
        if let Some(Value::Object(m)) = self.plugin.registry.as_ref().map(Plugins::to_json) {
            self.prefs.plugins = m;
        }
        if let Value::Object(m) = self.plugin.floats.to_json() {
            self.prefs.floats = m;
        }
    }

    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.plugin.effects)
    }

    pub fn plugin_dialog(&self) -> Option<&PluginDialog> {
        self.plugin.dialogs.front()
    }

    pub(crate) fn ask_text(&mut self, plugin: PluginInfo, ask: DialogAsk, reply: Reply<Option<String>>) {
        let id = self.next_dialog_id();
        self.plugin.dialogs.push_back(PluginDialog { id, plugin, ask, text: Some(reply), form: None });
        self.mark(Areas::PLUGINS);
    }

    pub(crate) fn ask_form(&mut self, plugin: PluginInfo, form: PluginForm, reply: Reply<Option<FormAnswer>>) {
        let ask = DialogAsk::Form(form);
        let id = self.next_dialog_id();
        self.plugin.dialogs.push_back(PluginDialog { id, plugin, ask, text: None, form: Some(reply) });
        self.mark(Areas::PLUGINS);
    }

    fn next_dialog_id(&mut self) -> u64 {
        self.plugin.next_reply += 1;
        self.plugin.next_reply
    }

    pub fn answer_dialog(&mut self, answer: DialogAnswer) {
        let Some(d) = self.plugin.dialogs.pop_front() else { return };
        match answer {
            DialogAnswer::Text(t) => {
                if let Some(r) = d.text {
                    let _ = r.send(Ok(t));
                }
            }
            DialogAnswer::Form(f) => {
                if let Some(r) = d.form {
                    let _ = r.send(Ok(f));
                }
            }
        }
        self.mark(Areas::PLUGINS);
    }

    fn has_windows(&self, plugin: &str) -> bool {
        self.plugin.views.values().any(|v| v.plugin_id == plugin)
            || self.tabs.iter().any(|t| t.owner.as_deref() == Some(plugin))
    }

    pub fn open_windows(&self, plugin: &str) -> usize {
        self.plugin.views.values().filter(|v| v.plugin_id == plugin).count()
            + self.tabs.iter().filter(|t| t.owner.as_deref() == Some(plugin)).count()
    }

    pub fn rail_plugins(&self) -> Vec<String> {
        let Some(r) = &self.plugin.registry else { return Vec::new() };
        rail::rail_plugins(&r.all(), &self.prefs.rail_order, |id| self.has_windows(id))
    }

    pub fn move_rail_plugin(&mut self, id: &str, to: usize) {
        let order = rail::move_rail_plugin(&self.rail_plugins(), &self.prefs.rail_order, id, to);
        self.set_rail_order(order);
    }

    pub fn footer_commands(&self) -> Vec<PluginCommand> {
        let Some(r) = &self.plugin.registry else { return Vec::new() };
        rail::footer_commands(&r.sidebar_commands(), &self.rail_plugins())
    }

    pub fn rail_badge(&self, plugin: &str) -> Option<RailBadge> {
        let busy = self.plugin.registry.as_ref().is_some_and(|r| r.plugin_busy(plugin));
        let said = self.plugin.badges.get(plugin).map(String::as_str);
        let current = self.prefs.sidebar_view.as_deref() == Some(plugin);
        rail::rail_badge(busy, said, self.open_windows(plugin), current)
    }

    pub(crate) fn sidebar_shown(&self, plugin: &str) -> bool {
        !self.prefs.sidebar_hidden && self.prefs.sidebar_view.as_deref() == Some(plugin)
    }

    pub fn show_sidebar_view(&mut self, plugin: Option<&str>) {
        let live = self.live_sidebar();
        if !self.prefs.sidebar_hidden && self.shown_plugin().as_deref() == plugin {
            self.set_sidebar_hidden(true);
            self.announce_sidebar(live, false);
            return;
        }
        self.prefs.sidebar_hidden = false;
        self.prefs.sidebar_view = plugin.map(str::to_owned);
        self.land_screen();
        self.save();
        self.announce_sidebar(None, true);
        if let Some(old) = live.filter(|o| Some(o.as_str()) != plugin)
            && let Some(r) = &self.plugin.registry
        {
            r.tell(&old, "sidebar.hidden", Default::default());
        }
        self.mark(Areas::PLUGINS | Areas::SIDEBAR | Areas::PREFS);
    }

    pub fn sidebar_view_of(&self, plugin: &str) -> Option<&PluginView> {
        self.plugin.sidebars.get(plugin)
    }

    pub fn plugin_view(&self, tab: &str) -> Option<&PluginView> {
        match &self.plugin.modal {
            Some((t, v)) if t.id == tab => Some(v),
            _ => self.plugin.views.get(tab),
        }
    }

    pub fn move_float(&mut self, key: &str, spot: FloatSpot) {
        self.plugin.floats.move_to(key, spot);
        self.sync_plugin_prefs();
        self.save();
        self.mark(Areas::PLUGINS);
    }

}
