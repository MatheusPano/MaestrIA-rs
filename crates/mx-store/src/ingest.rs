use mx_core::hook;
use mx_core::model::{AgentInfo, ClaudeStatus, ContextUsage, NoticeKind};
use serde_json::{Map, Value, json};

use crate::changes::Areas;
use crate::store::Store;

impl Store {
    pub fn apply_status(&mut self, tab: &str, payload: &Value) {
        let usage = ContextUsage::from_statusline(payload);
        let Some(t) = self.tab_mut(tab) else { return };
        if t.context == usage {
            return;
        }
        t.context = usage;
        self.mark_tab(tab);
    }

    pub fn apply_hook(&mut self, tab: &str, name: &str, payload: Map<String, Value>) {
        let now = self.now();
        let home = self.opts.home.clone();
        let payload = Value::Object(payload);
        let Some(t) = self.tab_mut(tab) else { return };
        let before = t.hooks.status;
        let produced = t.hooks.touched.len();
        hook::apply_with_home(&mut t.hooks, name, &payload, now, home.as_deref());
        let after = t.hooks.status;
        let mut save = false;
        if t.done && name == "UserPromptSubmit" {
            t.done = false;
            save = true;
        }
        if let Some(sid) = t.hooks.session_id.clone()
            && t.session_id.as_deref() != Some(sid.as_str())
        {
            t.session_id = Some(sid);
            save = true;
        }
        save |= t.hooks.touched.len() != produced;
        let activations = [format!("onHook:{name}"), "onHook:*".to_owned()];
        let activations: Vec<&str> = activations.iter().map(String::as_str).collect();
        self.p.plugins.emit(
            "hook",
            json!({"tabId": tab, "name": name, "payload": payload}),
            &activations,
            Some("hooks"),
        );
        if after != before {
            self.p.plugins.emit(
                "session.status",
                json!({"tabId": tab, "status": after.name(), "previous": before.name()}),
                &["onSession"],
                None,
            );
        }
        if save {
            self.save();
        }
        if after != before {
            self.status_notice(tab, before, after);
        }
        self.rest_edge(tab, before, after);
        self.check_alerts();
        self.mark_tab(tab);
    }

    fn status_notice(&mut self, tab: &str, before: ClaudeStatus, after: ClaudeStatus) {
        match after {
            ClaudeStatus::WaitingAnswer => {
                self.push_notice(tab, NoticeKind::Question, None, None);
            }
            ClaudeStatus::WaitingPermission => {
                self.push_notice(tab, NoticeKind::Permission, None, None);
            }
            _ if !before.at_rest() && after.at_rest() => {
                self.push_notice(tab, NoticeKind::Finished, None, None);
            }
            _ if !after.needs_human() && !after.at_rest() => {
                self.read_notices_of(tab, false, false);
            }
            _ => {}
        }
    }

    fn rest_edge(&mut self, tab: &str, before: ClaudeStatus, after: ClaudeStatus) {
        let now = self.now();
        let watching = self.watching(tab);
        let Some(t) = self.tab_mut(tab) else { return };
        if !before.at_rest() && after.at_rest() {
            if !t.follow_ups.is_empty() {
                t.armed = true;
            }
            t.rested_at = Some(now);
            t.unseen = !watching;
        } else if before.at_rest() && !after.at_rest() {
            t.rested_at = None;
            t.unseen = false;
        }
    }

    pub fn apply_agents(&mut self, list: Vec<AgentInfo>) {
        let mut save = false;
        for row in &list {
            let Some(pid) = row.pid else { continue };
            let Some(id) = self
                .terms
                .iter()
                .find(|(_, term)| term.pid().map(i64::from) == Some(pid))
                .map(|(id, _)| id.clone())
            else {
                continue;
            };
            let Some(t) = self.tab_mut(&id) else { continue };
            let mut changed = false;
            if let Some(name) = row.name.as_deref().filter(|n| !n.is_empty())
                && t.agent_name.as_deref() != Some(name)
            {
                t.agent_name = Some(name.to_owned());
                changed = true;
            }
            if let Some(sid) = &row.session_id
                && t.session_id.as_ref() != Some(sid)
            {
                t.session_id = Some(sid.clone());
                changed = true;
                save = true;
            }
            if changed {
                self.mark_tab(&id);
            }
        }
        self.agents = list;
        if save {
            self.save();
        }
        self.check_alerts();
    }

    pub(crate) fn check_alerts(&mut self) {
        let waiting: Vec<(String, String)> = self
            .tabs
            .iter()
            .filter(|t| !t.done && t.status().needs_human())
            .map(|t| {
                let text = format!("{} · {}", self.title(t), t.status().call_to_action());
                (format!("tab:{}", t.id), text)
            })
            .collect();
        for (key, text) in &waiting {
            if !self.alerted.contains(key) {
                self.alerted.push(key.clone());
                self.p.notifier.alert("MaestrIA", text);
            }
        }
        self.alerted.retain(|k| waiting.iter().any(|(w, _)| w == k));
        let badge = (!waiting.is_empty()).then(|| waiting.len().to_string());
        if self.badge.as_ref() != Some(&badge) {
            self.p.notifier.badge(badge.as_deref());
            self.badge = Some(badge);
            self.mark(Areas::NOTICES);
        }
    }
}
