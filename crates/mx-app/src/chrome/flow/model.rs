use mx_core::model::{FlowHold, FollowUp, FollowUpKind};

use crate::chrome::menu::Tone;

pub const REVIEW_PROMPT: &str = "Revise o que acabou de ser feito nesta worktree: leia `git status` e `git diff`, procure bugs, regressões e coisas fora do padrão do repo. Não altere nada — só relate o que achou.";

pub const RULE_NEW: &str = "A sessão abre com o prompt acima; o passo 1 sai quando ela terminar de verdade — turno encerrado, nenhum agente dela ainda rodando e alguns segundos de silêncio. A fila não sobrevive a fechar o app.";

pub const RULE_EXISTING: &str = "Um passo por vez: sai quando a sessão terminar de verdade — turno encerrado, nenhum agente dela ainda rodando e alguns segundos de silêncio. Passo que não devolve o turno pra ela (comando, sessão nova, passar a bola) já emenda no seguinte. A fila não sobrevive a fechar o app.";

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub title: String,
    pub status: String,
    pub tone: Tone,
    pub forks: usize,
    pub hold: Option<FlowHold>,
    pub at_rest: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Trigger {
    New { place: String },
    Existing(Session),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub kind: FollowUpKind,
    pub text: String,
    pub target: Option<String>,
}

impl Draft {
    pub fn new(kind: FollowUpKind) -> Self {
        let text = if kind == FollowUpKind::NewSession { REVIEW_PROMPT.to_owned() } else { String::new() };
        Self { kind, text, target: None }
    }

    pub fn filled(&self) -> bool {
        match self.kind {
            FollowUpKind::Handoff => self.target.is_some(),
            _ => !self.text.trim().is_empty(),
        }
    }

    pub fn step(&self) -> FollowUp {
        FollowUp { kind: self.kind, text: self.text.trim().to_owned(), target_tab_id: self.target.clone() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlowResult {
    pub label: String,
    pub prompt: String,
    pub steps: Vec<FollowUp>,
}

pub fn kinds(has_targets: bool) -> Vec<FollowUpKind> {
    FollowUpKind::ALL.into_iter().filter(|k| *k != FollowUpKind::Handoff || has_targets).collect()
}

pub fn ready(steps: &[Draft]) -> Vec<FollowUp> {
    steps.iter().filter(|s| s.filled()).map(Draft::step).collect()
}

pub fn subtitle(trigger: &Trigger, steps: usize) -> String {
    match trigger {
        Trigger::New { place } => format!("Em {place}"),
        Trigger::Existing(s) => format!("{steps} {} depois de \"{}\"", if steps == 1 { "passo" } else { "passos" }, s.title),
    }
}

pub fn standing(s: &Session) -> String {
    let mut out = format!("{} · {}", s.title, s.status);
    if s.forks > 0 {
        out.push_str(&format!(" · {} {} rodando", s.forks, if s.forks == 1 { "agente" } else { "agentes" }));
    }
    if let Some(hold) = s.hold.filter(|h| *h != FlowHold::Go) {
        out.push_str(&format!(" · {}", hold.label()));
    }
    out
}

pub fn verb(trigger: &Trigger, steps: &[Draft]) -> &'static str {
    let any = steps.iter().any(Draft::filled);
    match trigger {
        Trigger::New { .. } if any => "Abrir e armar",
        Trigger::New { .. } => "Abrir a sessão",
        Trigger::Existing(_) if !any => "Limpar a fila",
        Trigger::Existing(s) if s.at_rest => "Disparar",
        Trigger::Existing(_) => "Armar",
    }
}

pub fn can_save(trigger: &Trigger, prompt: &str) -> bool {
    match trigger {
        Trigger::New { .. } => !prompt.trim().is_empty(),
        Trigger::Existing(_) => true,
    }
}

pub fn rule(trigger: &Trigger) -> &'static str {
    match trigger {
        Trigger::New { .. } => RULE_NEW,
        Trigger::Existing(_) => RULE_EXISTING,
    }
}

pub fn adder_title(steps: usize) -> &'static str {
    if steps == 0 { "Por onde o fluxo começa?" } else { "E depois?" }
}
