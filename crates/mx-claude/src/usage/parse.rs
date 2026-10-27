use chrono::{DateTime, Datelike, Local, NaiveDateTime, TimeZone};
use serde_json::{Map, Value};

use crate::context::dart_round;

pub const NOT_JSON: &str = "A api respondeu uma coisa que não é json";
pub const NO_LIMITS: &str = "A resposta não trouxe limite nenhum";

#[derive(Debug, Clone, PartialEq)]
pub struct UsageWindow {
    pub label: String,
    pub percent: i64,
    pub resets_at: Option<DateTime<Local>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageAccount {
    pub email: Option<String>,
    pub organization: Option<String>,
    pub plan: Option<String>,
}

impl UsageAccount {
    pub fn is_empty(&self) -> bool {
        self.email.is_none() && self.organization.is_none() && self.plan.is_none()
    }

    pub fn headline(&self) -> Option<String> {
        let parts: Vec<&str> = [self.organization.as_deref(), self.plan.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        (!parts.is_empty()).then(|| parts.join(" · "))
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsageReading {
    pub windows: Vec<UsageWindow>,
    pub account: UsageAccount,
    pub credits: Option<String>,
    pub problem: Option<String>,
}

impl UsageReading {
    pub fn failed(problem: impl Into<String>) -> Self {
        Self {
            problem: Some(problem.into()),
            ..Self::default()
        }
    }

    pub fn ok(&self) -> bool {
        self.problem.is_none()
    }
}

pub fn parse(body: &str, account: UsageAccount) -> UsageReading {
    let Ok(Value::Object(json)) = serde_json::from_str::<Value>(body) else {
        return UsageReading::failed(NOT_JSON);
    };
    let Some(Value::Array(limits)) = json.get("limits") else {
        return UsageReading::failed(NO_LIMITS);
    };
    let windows: Vec<UsageWindow> = limits
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|limit| {
            let percent = limit.get("percent")?.as_f64()?;
            Some(UsageWindow {
                label: label_for(limit),
                percent: dart_round(percent),
                resets_at: limit
                    .get("resets_at")
                    .and_then(Value::as_str)
                    .and_then(parse_instant),
            })
        })
        .collect();
    if windows.is_empty() {
        return UsageReading::failed(NO_LIMITS);
    }
    UsageReading {
        windows,
        account,
        credits: credits_of(&json),
        problem: None,
    }
}

fn parse_instant(text: &str) -> Option<DateTime<Local>> {
    if let Ok(at) = DateTime::parse_from_rfc3339(text) {
        return Some(at.with_timezone(&Local));
    }
    ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"]
        .iter()
        .find_map(|fmt| NaiveDateTime::parse_from_str(text, fmt).ok())
        .and_then(|naive| Local.from_local_datetime(&naive).earliest())
}

fn label_for(limit: &Map<String, Value>) -> String {
    let kind = limit.get("kind").and_then(Value::as_str).unwrap_or("");
    let model = limit
        .get("scope")
        .and_then(|s| s.get("model"))
        .and_then(|m| m.get("display_name"))
        .and_then(Value::as_str);
    match (kind, model) {
        ("session", _) => "Sessão (5h)".into(),
        ("weekly_all", _) => "Semanal (7 dias)".into(),
        ("weekly_scoped", Some(model)) => format!("Semanal {model}"),
        ("weekly_scoped", None) => "Semanal por modelo".into(),
        ("", _) => "Limite".into(),
        (other, _) => other.replace('_', " "),
    }
}

fn credits_of(json: &Map<String, Value>) -> Option<String> {
    let spend = json.get("spend")?.as_object()?;
    if spend.get("enabled") != Some(&Value::Bool(true)) {
        return None;
    }
    let used = money(spend.get("used")?)?;
    Some(match spend.get("limit").and_then(money) {
        Some(limit) => format!("Créditos extras: {used} de {limit}"),
        None => format!("Créditos extras: {used}"),
    })
}

pub fn money(value: &Value) -> Option<String> {
    let value = value.as_object()?;
    let minor = value.get("amount_minor")?.as_f64()?;
    let exponent = value
        .get("exponent")
        .and_then(Value::as_f64)
        .map_or(2, |e| e.clamp(0.0, 20.0) as usize);
    let amount = minor / 10f64.powi(exponent as i32);
    let currency = value.get("currency").and_then(Value::as_str).unwrap_or("");
    let formatted = format!("{amount:.exponent$}").replace('.', ",");
    Some(format!("{currency} {formatted}").trim().to_owned())
}

pub fn plan_name(kind: Option<&str>) -> Option<String> {
    match kind? {
        "" => None,
        "claude_team" => Some("Claude team".into()),
        "claude_enterprise" => Some("Claude enterprise".into()),
        "claude_max" => Some("Claude Max".into()),
        "claude_pro" => Some("Claude Pro".into()),
        other => Some(other.replace('_', " ")),
    }
}

pub fn short_until<Tz: TimeZone>(at: &DateTime<Tz>, now: &DateTime<Tz>) -> String {
    let left = at.clone().signed_duration_since(now.clone());
    if left < chrono::TimeDelta::zero() {
        return "agora".into();
    }
    let minutes = left.num_minutes();
    if minutes < 1 {
        return "<1min".into();
    }
    if minutes < 60 {
        return format!("{minutes}min");
    }
    let hours = left.num_hours();
    if hours < 24 {
        return format!("{hours}h");
    }
    let days = left.num_days();
    if days < 7 {
        return format!("{days}d");
    }
    format!("{}/{}", at.day(), at.month())
}
