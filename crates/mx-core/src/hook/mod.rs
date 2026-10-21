mod touched;

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::json::Obj;
use crate::model::hook_state::MAX_PLANS;
use crate::model::{ClaudeStatus, HookState, PlanNote};
use crate::text::{Time, clip_or_ellipsis};

pub use touched::{absolute, shell_markdown};

pub const EVENTS: [&str; 10] = [
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Notification",
    "Stop",
    "StopFailure",
    "SubagentStop",
];

const FORK_END: &str = "SubagentStop";
const ASK_TOOL: &str = "AskUserQuestion";
const PLAN_TOOL: &str = "ExitPlanMode";

static WHITESPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").expect("valid regex"));

pub fn clip(text: Option<&str>, max: usize) -> Option<String> {
    let one = WHITESPACE.replace_all(text?, " ");
    let one = one.trim();
    if one.is_empty() {
        return None;
    }
    Some(clip_or_ellipsis(one, max))
}

fn s<'a>(p: &'a Obj, key: &str) -> Option<&'a str> {
    p.get(key).and_then(Value::as_str)
}

pub fn apply(state: &mut HookState, event: &str, payload: &Value, now: Time) {
    apply_with_home(state, event, payload, now, crate::paths::home().as_deref());
}

pub fn apply_with_home(
    st: &mut HookState,
    event: &str,
    payload: &Value,
    now: Time,
    home: Option<&str>,
) {
    let empty = Obj::new();
    let p = payload.as_object().unwrap_or(&empty);
    st.last_event_at = Some(now);
    if let Some(id) = s(p, "session_id") {
        st.session_id = Some(id.to_owned());
    }
    if let Some(mode) = s(p, "permission_mode") {
        st.permission_mode = Some(mode.to_owned());
    }

    if let Some(fork) = s(p, "agent_id").filter(|f| !f.is_empty()) {
        if event == FORK_END {
            fork_done(st, Some(fork));
        } else if !st.fork_ids.iter().any(|f| f == fork) {
            st.fork_ids.push(fork.to_owned());
            if st.fork_ids.len() > st.forks_out {
                st.forks_out = st.fork_ids.len();
            }
        }
        if event == "PreToolUse" {
            st.tools += 1;
        }
        if event == "PostToolUse" {
            touched::touch(st, p, home);
        }
        return;
    }
    if event == FORK_END {
        fork_done(st, None);
        return;
    }

    match event {
        "SessionStart" => {
            st.status = ClaudeStatus::Ready;
            forks_clear(st);
        }
        "UserPromptSubmit" => {
            st.question = None;
            st.questions = 0;
            st.prompts += 1;
            st.status = ClaudeStatus::Working;
            let said = match p.get("prompt").filter(|v| !v.is_null()) {
                Some(v) => v.as_str(),
                None => s(p, "user_input"),
            };
            st.last_prompt = clip(said, 70);
        }
        "PreToolUse" => {
            let tool = s(p, "tool_name");
            let input = p.get("tool_input");
            if tool == Some(ASK_TOOL) {
                asked(st, input);
            }
            if matches!(tool, Some("Task" | "Agent")) {
                st.forks_out += 1;
            }
            if tool == Some(PLAN_TOOL) {
                planned(st, input, now);
            }
            st.tools += 1;
            st.status = if tool == Some(ASK_TOOL) {
                ClaudeStatus::WaitingAnswer
            } else {
                ClaudeStatus::Tool
            };
            st.active_tool = tool.map(str::to_owned);
            st.tool_started_at = Some(now);
            st.last_tool_target = target(input);
        }
        "PostToolUse" => {
            st.status = ClaudeStatus::Working;
            clear_tool(st);
            st.question = None;
            st.questions = 0;
            touched::touch(st, p, home);
        }
        "PostToolUseFailure" => {
            clear_tool(st);
            st.question = None;
            st.questions = 0;
            st.status = if p.get("is_interrupt") == Some(&Value::Bool(true)) {
                ClaudeStatus::Idle
            } else {
                ClaudeStatus::Working
            };
        }
        "StopFailure" => {
            st.status = ClaudeStatus::Idle;
            clear_tool(st);
            if let Some(m) = clip(s(p, "error"), 90) {
                st.last_message = Some(m);
            }
        }
        "Notification" => match s(p, "notification_type").unwrap_or("") {
            "permission_prompt" => {
                st.status = if st.active_tool.as_deref() == Some(ASK_TOOL) {
                    ClaudeStatus::WaitingAnswer
                } else {
                    ClaudeStatus::WaitingPermission
                };
            }
            "agent_needs_input" => {
                st.status = ClaudeStatus::WaitingInput;
                if let Some(m) = clip(s(p, "message"), 70) {
                    st.last_message = Some(m);
                }
            }
            "idle_prompt" => {
                if !st.status.needs_human() {
                    st.status = ClaudeStatus::Idle;
                }
            }
            _ => {}
        },
        "Stop" => {
            st.status = ClaudeStatus::Idle;
            clear_tool(st);
            let closing = s(p, "last_assistant_message");
            if let Some(m) = clip(closing, 90) {
                st.last_message = Some(m);
            }
            if let Some(full) = closing.map(str::trim).filter(|c| !c.is_empty()) {
                st.last_message_full = Some(full.to_owned());
            }
        }
        "SessionEnd" => {
            let reason = match p.get("reason").filter(|v| !v.is_null()) {
                Some(v) => v.as_str(),
                None => s(p, "end_reason"),
            }
            .unwrap_or("other");
            st.status = if matches!(reason, "resume" | "clear") {
                ClaudeStatus::Ready
            } else {
                ClaudeStatus::Ended
            };
            forks_clear(st);
        }
        _ => {}
    }
}

fn clear_tool(st: &mut HookState) {
    st.active_tool = None;
    st.tool_started_at = None;
}

fn fork_done(st: &mut HookState, id: Option<&str>) {
    if let Some(id) = id {
        st.fork_ids.retain(|f| f != id);
    }
    st.forks_out = st.forks_out.saturating_sub(1);
}

fn forks_clear(st: &mut HookState) {
    st.fork_ids.clear();
    st.forks_out = 0;
}

fn planned(st: &mut HookState, input: Option<&Value>, now: Time) {
    let Some(text) = input.and_then(|i| i.get("plan")).and_then(Value::as_str) else {
        return;
    };
    if text.trim().is_empty() || st.plans.last().is_some_and(|p| p.text == text) {
        return;
    }
    st.plans.push(PlanNote {
        text: text.to_owned(),
        at: now,
    });
    if st.plans.len() > MAX_PLANS {
        st.plans.remove(0);
    }
}

fn asked(st: &mut HookState, input: Option<&Value>) {
    let Some(list) = input
        .and_then(Value::as_object)
        .and_then(|i| i.get("questions"))
        .and_then(Value::as_array)
        .filter(|l| !l.is_empty())
    else {
        return;
    };
    st.questions = list.len();
    let Some(first) = list[0].as_object() else {
        return;
    };
    st.question = clip(s(first, "header"), 24).or_else(|| clip(s(first, "question"), 60));
}

fn target(input: Option<&Value>) -> Option<String> {
    let input = input?.as_object()?;
    for key in ["file_path", "path", "notebook_path", "pattern"] {
        if let Some(v) = s(input, key).filter(|v| !v.is_empty()) {
            return Some(crate::text::last_segment(v).to_owned());
        }
    }
    clip(s(input, "command").filter(|c| !c.is_empty()), 28)
}
