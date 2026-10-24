use std::future::Future;

use mx_core::model::{AgentInfo, ClaudeStatus};
use mx_core::text::Time;

use crate::changes::Areas;
use crate::msg::Done;
use crate::store::Store;
use crate::timers::{AGENTS_EAGER, AGENTS_IDLE, TICK_AGES, TICK_FAST, TICK_GIT, TimerKey};

pub const AGENTS_COMMAND: &str = "claude agents --json";

impl Store {
    pub fn start(&mut self) {
        let now = self.now();
        self.timers.set(TimerKey::TickFast, now + TICK_FAST);
        self.timers.set(TimerKey::TickGit, now + TICK_GIT);
        self.timers.set(TimerKey::TickAges, now + TICK_AGES);
        self.timers.set(TimerKey::Agents, now);
        self.sync_eager();
        self.mark(Areas::ALL);
        self.fire_due();
    }

    pub fn next_deadline(&self) -> Option<Time> {
        self.timers.next()
    }

    pub fn fire_due(&mut self) {
        if self.gone {
            return;
        }
        let now = self.now();
        for key in self.timers.take_due(now) {
            self.fire(key, now);
        }
    }

    fn fire(&mut self, key: TimerKey, now: Time) {
        match key {
            TimerKey::Save => self.write_config(),
            TimerKey::Banner => self.clear_banner(),
            TimerKey::Reveal => self.end_reveal(),
            TimerKey::Toast(id) => self.dismiss_toast(id),
            TimerKey::Settle(id) => self.settle_launch(&id),
            TimerKey::TickFast => {
                self.tick_fast(now);
                self.timers.set(TimerKey::TickFast, now + TICK_FAST);
            }
            TimerKey::TickGit => {
                self.refresh_git();
                self.timers.set(TimerKey::TickGit, now + TICK_GIT);
            }
            TimerKey::TickAges => {
                self.tick_ages(now);
                self.timers.set(TimerKey::TickAges, now + TICK_AGES);
            }
            TimerKey::Agents => {
                self.poll_agents();
                let gap = if self.eager == Some(true) { AGENTS_EAGER } else { AGENTS_IDLE };
                self.timers.set(TimerKey::Agents, now + gap);
            }
        }
    }

    fn tick_fast(&mut self, now: Time) {
        self.pump_flows();
        self.hibernate_idle(now);
        self.see_focused();
        let live: Vec<String> = self
            .tabs
            .iter()
            .filter(|t| t.status() == ClaudeStatus::Tool || t.armed)
            .map(|t| t.id.clone())
            .collect();
        for id in live {
            self.mark_tab(&id);
        }
    }

    fn tick_ages(&mut self, now: Time) {
        let ages: Vec<(String, String)> = self
            .tabs
            .iter()
            .filter_map(|t| t.rested_ago(&now).map(|a| (t.id.clone(), a)))
            .collect();
        if ages == self.ages {
            return;
        }
        let changed: Vec<String> = ages
            .iter()
            .filter(|a| !self.ages.contains(a))
            .map(|(id, _)| id.clone())
            .collect();
        self.ages = ages;
        for id in changed {
            self.mark_tab(&id);
        }
    }

    pub fn poll_agents(&mut self) {
        if self.bg.agents_busy {
            return;
        }
        self.bg.agents_busy = true;
        let runner = self.p.runner.clone();
        self.spawn(async move {
            let r = runner.run(AGENTS_COMMAND.to_owned(), None).await;
            Done::Agents(if r.ok() { AgentInfo::list_from_stdout(&r.stdout) } else { None })
        });
    }

    pub(crate) fn agents_polled(&mut self, list: Option<Vec<AgentInfo>>) {
        self.bg.agents_busy = false;
        if let Some(list) = list {
            self.apply_agents(list);
        }
    }

    pub fn agents(&self) -> &[AgentInfo] {
        &self.agents
    }

    pub fn shutdown(&mut self) -> impl Future<Output = ()> + Send + 'static {
        self.quitting = true;
        self.flush();
        self.timers.clear();
        let mut kills = Vec::new();
        for tab in &mut self.tabs {
            tab.armed = false;
        }
        for term in self.terms.values_mut() {
            kills.push(self.p.rt.spawn(term.kill()));
        }
        if let Some(plugins) = self.plugin.registry.clone() {
            kills.push(self.p.rt.spawn(async move { plugins.stop_all().await }));
        }
        async move {
            for k in kills {
                let _ = k.await;
            }
        }
    }

    pub fn dispose(&mut self) {
        self.gone = true;
        self.timers.clear();
        self.bell.toasts.clear();
        for tab in &mut self.tabs {
            tab.armed = false;
        }
        for term in self.terms.values_mut() {
            term.hang_up();
        }
    }

    pub fn is_gone(&self) -> bool {
        self.gone
    }
}
