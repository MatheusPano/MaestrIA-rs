use std::collections::HashMap;

use mx_core::model::{ClaudeStatus, TabKind};
use mx_core::ports::BoxFuture;
use mx_core::text::clip_utf16;
use mx_plugins::PluginInfo;
use mx_plugins::rpc::keys::bad_keys;
use mx_plugins::rpc::portrait::{not_claude, resolve_portrait};
use mx_plugins::rpc::{OpenClaude, RpcError, SendKeys, SendText, TabOwner};
use serde_json::{Value, json};

use super::Effect;
use crate::store::Store;
use crate::tabs_open::ClaudeSpec;

const TITLE_MAX: usize = 80;

#[derive(Default)]
pub struct SessionExtras {
    pub(crate) portraits: HashMap<String, String>,
    pub(crate) settle_tries: HashMap<String, u32>,
}

impl Store {
    pub fn portrait_of(&self, tab: &str) -> Option<&str> {
        self.plugin.extras.portraits.get(tab).map(String::as_str)
    }

    fn known_tab(&self, id: String) -> Result<String, RpcError> {
        if self.has_tab(&id) { Ok(id) } else { Err(RpcError::no_tab(&id)) }
    }

    fn live_target(&self) -> Option<String> {
        if let Some(t) = self.focused_tab().filter(|t| !t.is_passive()) {
            return Some(t.id.clone());
        }
        let open = self.open_panes();
        open.iter()
            .find(|t| !t.is_passive() && t.kind == TabKind::Claude)
            .or_else(|| open.iter().find(|t| !t.is_passive()))
            .map(|t| t.id.clone())
    }

    fn session_tab(&self, tab_id: Option<String>) -> Result<String, RpcError> {
        match tab_id {
            Some(id) => self.known_tab(id),
            None => self.live_target().ok_or_else(RpcError::no_session_on_screen),
        }
    }

    fn with_process(&self, tab_id: Option<String>, receives: &str) -> Result<String, RpcError> {
        let id = self.session_tab(tab_id)?;
        let t = self.tab(&id).ok_or_else(|| RpcError::no_tab(&id))?;
        if t.is_passive() || t.exited || t.hibernated {
            return Err(RpcError::invalid(format!("\"{}\" não tem processo pra receber {receives}", self.title(t))));
        }
        let asks_trust = t.kind == TabKind::Claude && self.terms.get(&id).is_some_and(|term| term.asks_trust());
        if asks_trust {
            return Err(RpcError::invalid(format!(
                "\"{}\" está perguntando se você confia na pasta: responda no terminal dele",
                self.title(t)
            )));
        }
        Ok(id)
    }

    pub(crate) fn plugin_send_text(&mut self, req: SendText) -> Result<(), RpcError> {
        let id = self.with_process(req.tab_id, "texto")?;
        if !req.submit {
            self.plugin.effects.push(Effect::Paste { tab_id: id, text: req.text });
        } else if req.text.trim().is_empty() {
            if let Some(term) = self.terms.get_mut(&id) {
                term.enter();
            }
        } else {
            self.submit(&id, &req.text);
        }
        Ok(())
    }

    pub(crate) fn plugin_send_keys(&mut self, req: SendKeys) -> Result<BoxFuture<'static, ()>, RpcError> {
        let id = self.with_process(req.tab_id, "teclas")?;
        let keys = req.keys.ok_or_else(bad_keys)?;
        let term = self.terms.get_mut(&id).ok_or_else(|| RpcError::no_tab(&id))?;
        Ok(term.send_keys(keys))
    }

    pub(crate) fn plugin_paste_clipboard(&mut self, tab_id: Option<String>) -> Result<(), RpcError> {
        let id = self.with_process(tab_id, "a colagem")?;
        let images_via_ctrl_v = self.tab(&id).is_some_and(|t| t.kind == TabKind::Claude);
        self.plugin.effects.push(Effect::PasteClipboard { tab_id: id, images_via_ctrl_v });
        Ok(())
    }

    pub(crate) fn plugin_open_claude(&mut self, req: OpenClaude) -> String {
        let (folder, cwd) = self.plugin_place(req.cwd);
        let mut spec = ClaudeSpec::new(&folder, &cwd);
        spec.label = req.label;
        spec.prompt = req.prompt;
        spec.feature = self.focused_feature().map(|f| f.id.clone());
        if req.background {
            return self.open_claude_placed(spec, false);
        }
        let id = self.open_claude(spec);
        self.select(&id);
        id
    }

    pub(crate) fn emit_renamed(&self, id: &str) {
        let Some(t) = self.tab(id).filter(|t| !t.is_passive()) else { return };
        self.p.plugins.emit("session.renamed", json!({"tabId": id, "title": self.title(t)}), &["onSession"], None);
    }

    pub(crate) fn plugin_rename(&mut self, id: String, title: &str) -> Result<String, RpcError> {
        let id = self.known_tab(id)?;
        self.rename_tab(&id, clip_utf16(title.trim(), TITLE_MAX));
        self.emit_renamed(&id);
        Ok(self.title_of(&id))
    }

    pub(crate) fn plugin_wake(&mut self, id: String) -> Result<bool, RpcError> {
        let id = self.known_tab(id)?;
        if !self.tab(&id).is_some_and(|t| t.hibernated) {
            return Ok(false);
        }
        self.wake(&id);
        Ok(true)
    }

    pub(crate) fn plugin_hibernate(&mut self, id: String) -> Result<bool, RpcError> {
        let id = self.known_tab(id)?;
        let t = self.tab(&id).ok_or_else(|| RpcError::no_tab(&id))?;
        if t.hibernated {
            return Ok(false);
        }
        let title = self.title(t);
        if !t.can_hibernate() {
            return Err(RpcError::invalid(if t.kind != TabKind::Claude || t.exited {
                format!("\"{title}\" não é uma sessão do claude rodando")
            } else {
                format!("\"{title}\" ainda não tem conversa pra retomar")
            }));
        }
        let status = t.status();
        if (status != ClaudeStatus::Ready && !status.at_rest()) || t.hooks.busy_forks() {
            return Err(RpcError::invalid(format!("\"{title}\" está trabalhando: hiberne quando ele parar")));
        }
        self.hibernate(&id);
        Ok(true)
    }

    pub(crate) fn plugin_portrait(&mut self, plugin: &PluginInfo, id: String, image: Option<Value>) -> Result<(), RpcError> {
        let id = self.known_tab(id)?;
        if !self.tab(&id).is_some_and(|t| t.kind == TabKind::Claude) {
            return Err(not_claude());
        }
        let Some(image) = image else {
            self.plugin.extras.portraits.remove(&id);
            return Ok(());
        };
        let real = resolve_portrait(&image, &plugin.dir, &plugin.data_dir)?;
        self.plugin.extras.portraits.insert(id, real);
        Ok(())
    }

    pub(crate) fn plugin_tab_owner(&self, id: String) -> Result<TabOwner, RpcError> {
        let t = self.tab(&id).ok_or_else(|| RpcError::no_tab(&id))?;
        Ok(TabOwner { tab_id: id.clone(), kind: t.kind, owner: t.owner.clone() })
    }
}
