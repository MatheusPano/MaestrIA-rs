use chrono::TimeDelta;
use mx_core::model::ClaudeStatus;
use mx_core::text::Time;

use crate::msg::Done;
use crate::store::Store;
use crate::timers::TimerKey;

pub const HIBERNATED: &str =
    "Hibernada — processo desligado pra liberar memória; a conversa volta com um clique";

impl Store {
    pub fn auto_hibernate(&self) -> bool {
        self.prefs.hibernate_minutes > 0
    }

    pub fn hibernate(&mut self, id: &str) {
        let Some(t) = self.tab_mut(id) else { return };
        if !t.can_hibernate() {
            return;
        }
        t.hibernated = true;
        t.armed = false;
        t.rested_at = None;
        t.unseen = false;
        t.hooks.status = ClaudeStatus::Ended;
        t.hooks.active_tool = None;
        t.hooks.tool_started_at = None;
        if let Some(term) = self.terms.get_mut(id) {
            term.remark(HIBERNATED);
            if term.pid().is_none() {
                term.park();
            } else {
                self.p.rt.spawn(term.kill());
            }
        }
        self.sync_term(id);
        self.timers.cancel(&TimerKey::Settle(id.to_owned()));
        self.save();
        self.mark_tab(id);
    }

    pub fn wake(&mut self, id: &str) {
        let Some(t) = self.tab_mut(id) else { return };
        if !t.hibernated {
            return;
        }
        t.hibernated = false;
        t.hooks.status = ClaudeStatus::Starting;
        let exited = t.exited;
        self.mark_tab(id);
        if exited {
            self.relaunch_woken(id);
            return;
        }
        let Some(term) = self.terms.get_mut(id) else { return };
        let kill = term.kill();
        let tab_id = id.to_owned();
        self.spawn(async move {
            kill.await;
            Done::Killed { tab_id }
        });
    }

    pub(crate) fn relaunch_woken(&mut self, id: &str) {
        let Some(resume) = self.tab(id).map(|t| t.resume_id().map(str::to_owned)) else {
            return;
        };
        self.launch_claude(id, resume, None);
        self.save();
        self.mark_tab(id);
    }

    pub fn hibernate_idle(&mut self, now: Time) {
        if !self.auto_hibernate() {
            return;
        }
        let limit = TimeDelta::minutes(self.prefs.hibernate_minutes);
        let due: Vec<String> = self
            .tabs
            .iter()
            .filter(|t| t.can_hibernate() && !self.placed(t))
            .filter(|t| {
                let st = t.status();
                st == ClaudeStatus::Ready || st.at_rest()
            })
            .filter(|t| !t.armed && t.follow_ups.is_empty() && !t.hooks.busy_forks())
            .filter(|t| {
                !self.tabs.iter().any(|o| {
                    o.follow_ups
                        .iter()
                        .any(|f| f.target_tab_id.as_deref() == Some(t.id.as_str()))
                })
            })
            .filter(|t| {
                let since = t.rested_at.or(t.hooks.last_event_at).unwrap_or(t.started_at);
                now - since >= limit
            })
            .map(|t| t.id.clone())
            .collect();
        for id in due {
            self.hibernate(&id);
        }
    }
}
