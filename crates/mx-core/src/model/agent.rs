use serde_json::Value;

use crate::json::{get_num, str_of};

#[derive(Debug, Clone, PartialEq)]
pub struct AgentInfo {
    pub id: Option<String>,
    pub pid: Option<i64>,
    pub cwd: String,
    pub kind: String,
    pub session_id: Option<String>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub waiting_for: Option<String>,
    pub state: Option<String>,
    pub started_at: Option<i64>,
}

impl AgentInfo {
    pub fn from_json(v: &Value) -> Option<AgentInfo> {
        let m = v.as_object()?;
        let owned = |k: &str| str_of(m, k).map(str::to_owned);
        Some(AgentInfo {
            id: owned("id"),
            pid: m.get("pid").and_then(Value::as_i64),
            cwd: owned("cwd").unwrap_or_default(),
            kind: owned("kind").unwrap_or_else(|| "interactive".into()),
            session_id: owned("sessionId"),
            name: owned("name"),
            status: owned("status"),
            waiting_for: owned("waitingFor"),
            state: owned("state"),
            started_at: get_num(m, "startedAt").lenient().map(|n| n as i64),
        })
    }

    pub fn list_from_stdout(stdout: &str) -> Option<Vec<AgentInfo>> {
        let at = stdout.find('[')?;
        let parsed: Value = serde_json::from_str(&stdout[at..]).ok()?;
        let items = parsed.as_array()?;
        Some(items.iter().filter_map(AgentInfo::from_json).collect())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextUsage {
    pub percent: i64,
    pub tokens: i64,
    pub window: i64,
}

impl ContextUsage {
    pub fn from_statusline(v: &Value) -> Option<ContextUsage> {
        let cw = v.get("context_window")?.as_object()?;
        let percent = cw.get("used_percentage")?.as_f64()?;
        let window = cw.get("context_window_size")?.as_f64()?;
        if window <= 0.0 {
            return None;
        }
        let usage = cw.get("current_usage")?.as_object()?;
        let tokens: f64 = [
            "input_tokens",
            "cache_creation_input_tokens",
            "cache_read_input_tokens",
        ]
        .iter()
        .map(|k| usage.get(*k).and_then(Value::as_f64).unwrap_or(0.0))
        .sum();
        Some(ContextUsage {
            percent: percent.round() as i64,
            tokens: tokens.round() as i64,
            window: window.round() as i64,
        })
    }
}
