use chrono::TimeDelta;
use mx_core::model::{FlowHold, FollowUp, FollowUpKind, Folder};
use mx_core::text::{clip_utf16, utf16_len};

use crate::store::Store;
use crate::tabs_open::ClaudeSpec;

pub const FLOW_PATIENCE: TimeDelta = TimeDelta::minutes(3);
pub const FLOW_QUIET: TimeDelta = TimeDelta::seconds(5);
const HANDOFF_MAX: usize = 4000;
const LABEL_MAX: usize = 28;

pub fn shell_label(command: &str) -> String {
    let one = command.split_whitespace().collect::<Vec<_>>().join(" ");
    if utf16_len(&one) <= LABEL_MAX {
        return one;
    }
    format!("{}…", clip_utf16(&one, LABEL_MAX - 1).trim_end())
}

fn tail_utf16(s: &str, max: usize) -> &str {
    let mut units = 0;
    for (i, c) in s.char_indices().rev() {
        units += c.len_utf16();
        if units > max {
            return &s[i + c.len_utf8()..];
        }
    }
    s
}

pub fn handoff_text(title: &str, said: Option<&str>, note: &str) -> String {
    let mut out = format!("[MaestrIA] o painel \"{title}\" acabou de terminar.");
    if let Some(said) = said.filter(|s| !s.is_empty()) {
        let said = if utf16_len(said) <= HANDOFF_MAX {
            said.to_owned()
        } else {
            format!("…{}", tail_utf16(said, HANDOFF_MAX))
        };
        out.push_str(&format!("\n\nO que ele disse ao terminar:\n\"\"\"\n{said}\n\"\"\""));
    }
    if !note.trim().is_empty() {
        out.push_str(&format!("\n\n{}", note.trim()));
    }
    out
}

impl Store {
    pub fn queue(&mut self, id: &str, steps: Vec<FollowUp>, now: bool) {
        let Some(t) = self.tab_mut(id) else { return };
        t.armed = !steps.is_empty() && (now || t.armed);
        t.follow_ups = steps;
        self.mark_tab(id);
    }

    pub fn start_flow(
        &mut self,
        folder: &Folder,
        prompt: &str,
        steps: Vec<FollowUp>,
        cwd: Option<&str>,
        label: Option<&str>,
        feature: Option<&str>,
    ) -> String {
        let mut spec = ClaudeSpec::new(folder, cwd.unwrap_or(&folder.root));
        spec.label = label.map(str::to_owned);
        spec.feature = feature.map(str::to_owned);
        spec.prompt = Some(prompt.to_owned());
        let id = self.open_claude(spec);
        let n = steps.len();
        self.queue(&id, steps, false);
        let title = self.title_of(&id);
        let text = match n {
            0 => format!("{title}: sessão aberta"),
            1 => format!("{title}: fluxo de 1 passo armado"),
            n => format!("{title}: fluxo de {n} passos armado"),
        };
        self.show_banner(&text);
        id
    }

    pub fn hold_for(&self, id: &str) -> FlowHold {
        let Some(t) = self.tab(id) else { return FlowHold::Working };
        if t.exited || !t.status().at_rest() {
            return FlowHold::Working;
        }
        let silence = t
            .hooks
            .last_event_at
            .map_or(FLOW_PATIENCE, |last| self.now() - last);
        if silence >= FLOW_PATIENCE {
            return FlowHold::Go;
        }
        if t.hooks.busy_forks() {
            return FlowHold::Forks;
        }
        if silence >= FLOW_QUIET {
            return FlowHold::Go;
        }
        FlowHold::Quiet
    }

    pub fn pump_flows(&mut self) {
        let ready: Vec<String> = self
            .tabs
            .iter()
            .filter(|t| t.armed && !t.exited && !t.follow_ups.is_empty())
            .map(|t| t.id.clone())
            .collect();
        for id in ready {
            if self.hold_for(&id) == FlowHold::Go {
                self.advance(&id);
            }
        }
    }

    pub fn advance(&mut self, id: &str) {
        let Some(t) = self.tab_mut(id) else { return };
        if t.exited || t.follow_ups.is_empty() {
            return;
        }
        let step = t.follow_ups.remove(0);
        t.armed = !step.kind.hands_back() && !t.follow_ups.is_empty();
        let late = t.hooks.busy_forks();
        if late {
            t.hooks.fork_ids.clear();
            t.hooks.forks_out = 0;
        }
        self.mark_tab(id);
        self.run_follow_up(step, id, late);
    }

    fn run_follow_up(&mut self, step: FollowUp, from: &str, late: bool) {
        let Some(tab) = self.tab(from) else { return };
        let title = self.title(tab);
        let (folder, cwd, feature) = (
            self.folder_of(tab).clone(),
            tab.cwd.clone(),
            tab.feature_or_hotfix_id.clone(),
        );
        let (exited, said) = (tab.exited, tab.hooks.last_message_full.clone());
        if late {
            self.show_banner_sticky(&format!(
                "{title}: os agentes dela não avisaram que terminaram — o fluxo seguiu assim mesmo"
            ));
        }
        match step.kind {
            FollowUpKind::KeepGoing => {
                if exited {
                    return;
                }
                self.submit(from, &step.text);
                self.show_banner(&format!("{title}: próximo passo enviado"));
            }
            FollowUpKind::NewSession => {
                let mut spec = ClaudeSpec::new(&folder, &cwd);
                spec.label = Some(format!("{title} ▸ depois"));
                spec.feature = feature;
                spec.prompt = Some(step.text.clone());
                let id = self.open_claude(spec);
                self.descend(&id, from);
                let next = self.title_of(&id);
                self.show_banner(&format!("{title} terminou — abri {next} em seguida"));
            }
            FollowUpKind::Command => {
                let id = self.open_shell(&folder, Some(&cwd), Some(&step.text), feature.as_deref(), None);
                if let Some(t) = self.tab_mut(&id) {
                    t.custom_label = Some(shell_label(&step.text));
                }
                self.descend(&id, from);
                self.show_banner(&format!("{title} terminou — rodando {}", step.text));
            }
            FollowUpKind::Handoff => {
                let target = step
                    .target_tab_id
                    .as_deref()
                    .and_then(|t| self.tab(t))
                    .filter(|t| !t.exited)
                    .map(|t| t.id.clone());
                let Some(target) = target else {
                    self.show_banner_sticky(&format!(
                        "{title} terminou, mas o painel que ia receber não está mais aberto"
                    ));
                    return;
                };
                self.submit(&target, &handoff_text(&title, said.as_deref(), &step.text));
                let to = self.title_of(&target);
                self.show_banner(&format!("{title} passou a bola pra {to}"));
            }
        }
    }

    pub(crate) fn submit(&mut self, id: &str, text: &str) {
        if let Some(term) = self.terms.get_mut(id) {
            term.submit(text);
        }
    }

    pub fn descends_from(&self, id: &str, root: &str) -> bool {
        let mut cur = self.tab(id).and_then(|t| t.born_of.clone());
        for _ in 0..8 {
            match cur {
                None => return false,
                Some(p) if p == root => return true,
                Some(p) => cur = self.tab(&p).and_then(|t| t.born_of.clone()),
            }
        }
        false
    }

    fn descend(&mut self, child: &str, parent: &str) {
        let Some(from) = self.pos(child) else { return };
        let mut tab = self.tabs.remove(from);
        tab.born_of = Some(parent.to_owned());
        let Some(p) = self.pos(parent) else {
            self.tabs.insert(from.min(self.tabs.len()), tab);
            return;
        };
        let mut at = p + 1;
        while at < self.tabs.len() && self.descends_from(&self.tabs[at].id, parent) {
            at += 1;
        }
        self.tabs.insert(at, tab);
        self.save();
        self.mark(crate::changes::Areas::SIDEBAR);
    }
}
