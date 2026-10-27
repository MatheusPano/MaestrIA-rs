use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Value, json};

use crate::json::{Obj, get_str};
use crate::model::ClaudeStatus;
use crate::text::{Time, clip_or_ellipsis, iso8601, parse_iso8601_or};

pub const MAX_TOUCHED: usize = 200;
pub const MAX_PLANS: usize = 8;
pub const MAX_SUGGESTIONS: usize = 20;

static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#+\s*").expect("valid regex"));

#[derive(Debug, Clone, PartialEq)]
pub struct PlanNote {
    pub text: String,
    pub at: Time,
}

impl PlanNote {
    pub fn headline(&self) -> String {
        for line in self.text.split('\n') {
            let one = HEADING.replace(line.trim(), "");
            if !one.is_empty() {
                return clip_or_ellipsis(&one, 70);
            }
        }
        "Plano sem título".into()
    }

    pub fn to_json(&self) -> Value {
        json!({"text": self.text, "at": iso8601(&self.at)})
    }

    pub fn from_json(v: &Value, now: Time) -> Option<PlanNote> {
        let m = v.as_object()?;
        let text = get_str(m, "text").lenient()?;
        if text.trim().is_empty() {
            return None;
        }
        Some(PlanNote {
            text: text.to_owned(),
            at: parse_iso8601_or(get_str(m, "at").lenient(), now),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SuggestedTask {
    pub id: String,
    pub title: String,
    pub tldr: String,
    pub prompt: String,
    pub at: Time,
}

impl SuggestedTask {
    pub fn to_json(&self) -> Value {
        let mut m = Obj::new();
        m.insert("id".into(), json!(self.id));
        m.insert("title".into(), json!(self.title));
        m.insert("tldr".into(), json!(self.tldr));
        m.insert("prompt".into(), json!(self.prompt));
        m.insert("at".into(), json!(iso8601(&self.at)));
        Value::Object(m)
    }

    pub fn from_json(v: &Value, now: Time) -> Option<SuggestedTask> {
        let m = v.as_object()?;
        let id = get_str(m, "id").lenient()?;
        let prompt = get_str(m, "prompt").lenient()?;
        if prompt.trim().is_empty() {
            return None;
        }
        Some(SuggestedTask {
            id: id.to_owned(),
            title: get_str(m, "title").lenient().unwrap_or("").to_owned(),
            tldr: get_str(m, "tldr").lenient().unwrap_or("").to_owned(),
            prompt: prompt.to_owned(),
            at: parse_iso8601_or(get_str(m, "at").lenient(), now),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HookState {
    pub status: ClaudeStatus,
    pub session_id: Option<String>,
    pub permission_mode: Option<String>,
    pub active_tool: Option<String>,
    pub tool_started_at: Option<Time>,
    pub last_tool_target: Option<String>,
    pub last_message: Option<String>,
    pub last_message_full: Option<String>,
    pub question: Option<String>,
    pub questions: usize,
    pub last_prompt: Option<String>,
    pub prompts: u64,
    pub tools: u64,
    pub last_event_at: Option<Time>,
    pub forks_out: usize,
    pub fork_ids: Vec<String>,
    pub touched: Vec<String>,
    pub plans: Vec<PlanNote>,
    pub suggestions: Vec<SuggestedTask>,
}

impl HookState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn plan(&self) -> Option<&PlanNote> {
        self.plans.last()
    }

    pub fn busy_forks(&self) -> bool {
        self.forks_out > 0
    }

    pub fn settle(&mut self) -> bool {
        if self.status != ClaudeStatus::Starting {
            return false;
        }
        self.status = ClaudeStatus::Ready;
        true
    }

    pub fn push_suggestion(&mut self, task: SuggestedTask) {
        self.suggestions.push(task);
        while self.suggestions.len() > MAX_SUGGESTIONS {
            self.suggestions.remove(0);
        }
    }

    pub fn subtitle(&self, now: &Time) -> String {
        match self.status {
            ClaudeStatus::WaitingPermission => format!(
                "Quer aprovação pra {}",
                self.active_tool.as_deref().unwrap_or("null")
            ),
            ClaudeStatus::WaitingAnswer => match &self.question {
                None => "Te faz uma pergunta".into(),
                Some(q) => {
                    let rest = if self.questions > 1 {
                        format!(" (+{})", self.questions - 1)
                    } else {
                        String::new()
                    };
                    format!("Pergunta: {q}{rest}")
                }
            },
            ClaudeStatus::WaitingInput => self
                .last_message
                .clone()
                .unwrap_or_else(|| "Te espera".into()),
            ClaudeStatus::Tool if self.active_tool.is_some() => {
                let tool = self.active_tool.as_deref().unwrap_or_default();
                let target = self
                    .last_tool_target
                    .as_ref()
                    .map(|t| format!("({t})"))
                    .unwrap_or_default();
                let secs = self
                    .tool_started_at
                    .map(|at| format!(" {}s", now.signed_duration_since(at).num_seconds()))
                    .unwrap_or_default();
                format!("{tool}{target}{secs}")
            }
            ClaudeStatus::Working => self
                .last_prompt
                .clone()
                .unwrap_or_else(|| "Trabalhando".into()),
            ClaudeStatus::Idle => self.last_message.clone().unwrap_or_else(|| "Pronto".into()),
            other => other.label().into(),
        }
    }
}
