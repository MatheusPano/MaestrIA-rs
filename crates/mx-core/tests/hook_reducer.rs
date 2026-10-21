use chrono::{Local, TimeZone};
use mx_core::hook::apply_with_home;
use mx_core::model::{ClaudeStatus, HookState};
use mx_core::text::Time;
use serde_json::{Value, json};

fn now() -> Time {
    Local.with_ymd_and_hms(2026, 9, 18, 12, 0, 0).unwrap()
}

fn fire(s: &mut HookState, event: &str, extra: Value) {
    let mut p = json!({"hook_event_name": event, "session_id": "sess-1", "cwd": "/repo"});
    if let (Some(base), Some(more)) = (p.as_object_mut(), extra.as_object()) {
        for (k, v) in more {
            base.insert(k.clone(), v.clone());
        }
    }
    apply_with_home(s, event, &p, now(), Some("/home/u"));
}

fn tool(s: &mut HookState, event: &str, name: &str, input: Value) {
    fire(s, event, json!({"tool_name": name, "tool_input": input}));
}

#[test]
fn session_start_makes_a_fresh_state_ready() {
    let mut s = HookState::new();
    fire(&mut s, "SessionStart", json!({}));
    assert_eq!(s.status, ClaudeStatus::Ready);
    assert_eq!(s.session_id.as_deref(), Some("sess-1"));
}

#[test]
fn a_full_turn_walks_working_tool_working_idle() {
    let mut s = HookState::new();
    fire(
        &mut s,
        "UserPromptSubmit",
        json!({"prompt": "troca o middleware  de sessão"}),
    );
    assert_eq!(s.status, ClaudeStatus::Working);
    assert_eq!(s.prompts, 1);
    assert_eq!(
        s.last_prompt.as_deref(),
        Some("troca o middleware de sessão")
    );

    tool(
        &mut s,
        "PreToolUse",
        "Edit",
        json!({"file_path": "/repo/lib/auth/jwt.dart"}),
    );
    assert_eq!(s.status, ClaudeStatus::Tool);
    assert_eq!(s.active_tool.as_deref(), Some("Edit"));
    assert_eq!(s.last_tool_target.as_deref(), Some("jwt.dart"));
    assert!(s.subtitle(&now()).starts_with("Edit(jwt.dart)"));

    tool(
        &mut s,
        "PostToolUse",
        "Edit",
        json!({"file_path": "/repo/lib/auth/jwt.dart"}),
    );
    assert_eq!(s.status, ClaudeStatus::Working);
    assert_eq!(s.active_tool, None);

    fire(
        &mut s,
        "Stop",
        json!({"last_assistant_message": "Troquei os 7 call sites."}),
    );
    assert_eq!(s.status, ClaudeStatus::Idle);
    assert_eq!(s.subtitle(&now()), "Troquei os 7 call sites.");
}

#[test]
fn permission_prompt_on_a_fresh_state_waits_for_permission() {
    let mut s = HookState::new();
    fire(
        &mut s,
        "Notification",
        json!({"notification_type": "permission_prompt"}),
    );
    assert_eq!(s.status, ClaudeStatus::WaitingPermission);
    assert!(s.status.needs_human());
}

#[test]
fn ask_user_question_waits_for_an_answer_and_permission_prompt_keeps_it() {
    let mut s = HookState::new();
    let questions = json!({"questions": [
        {"header": "Receitas", "question": "Quais receitas?"},
        {"header": "Briefing", "question": "Qual briefing?"}
    ]});
    tool(&mut s, "PreToolUse", "AskUserQuestion", questions);
    assert_eq!(s.status, ClaudeStatus::WaitingAnswer);
    assert_eq!(s.subtitle(&now()), "Pergunta: Receitas (+1)");
    assert_eq!(s.status.call_to_action(), "te faz uma pergunta");

    fire(
        &mut s,
        "Notification",
        json!({"notification_type": "permission_prompt"}),
    );
    assert_eq!(s.status, ClaudeStatus::WaitingAnswer);

    tool(&mut s, "PostToolUse", "AskUserQuestion", json!({}));
    assert_eq!(s.status, ClaudeStatus::Working);
    assert_eq!(s.question, None);
}

#[test]
fn a_question_without_header_shows_the_question_text() {
    let mut s = HookState::new();
    tool(
        &mut s,
        "PreToolUse",
        "AskUserQuestion",
        json!({"questions": [{"question": "Sobrescrevo o arquivo?"}]}),
    );
    assert_eq!(s.subtitle(&now()), "Pergunta: Sobrescrevo o arquivo?");
}

#[test]
fn agent_needs_input_waits_for_you_until_stop() {
    let mut s = HookState::new();
    fire(
        &mut s,
        "Notification",
        json!({"notification_type": "agent_needs_input", "message": "Posso rodar os testes?"}),
    );
    assert_eq!(s.status, ClaudeStatus::WaitingInput);
    assert_eq!(s.subtitle(&now()), "Posso rodar os testes?");
    fire(&mut s, "Stop", json!({}));
    assert!(!s.status.needs_human());
}

#[test]
fn idle_prompt_after_stop_rests_and_keeps_the_message() {
    let mut s = HookState::new();
    fire(
        &mut s,
        "UserPromptSubmit",
        json!({"user_input": "roda os testes"}),
    );
    assert_eq!(s.last_prompt.as_deref(), Some("roda os testes"));
    fire(
        &mut s,
        "Stop",
        json!({"last_assistant_message": "Pronto, os testes passam."}),
    );
    fire(
        &mut s,
        "Notification",
        json!({"notification_type": "idle_prompt"}),
    );
    assert!(!s.status.needs_human());
    assert!(s.status.at_rest());
    assert_eq!(s.subtitle(&now()), "Pronto, os testes passam.");
}

#[test]
fn idle_prompt_does_not_override_a_waiting_state() {
    for start in [ClaudeStatus::WaitingAnswer, ClaudeStatus::WaitingPermission] {
        let mut s = HookState {
            status: start,
            ..HookState::new()
        };
        fire(
            &mut s,
            "Notification",
            json!({"notification_type": "idle_prompt"}),
        );
        assert_eq!(s.status, start);
    }
}

#[test]
fn session_end_logout_ends_the_session() {
    let mut s = HookState::new();
    fire(&mut s, "SessionEnd", json!({"reason": "logout"}));
    assert_eq!(s.status, ClaudeStatus::Ended);
}

#[test]
fn post_tool_use_failure_keeps_working_unless_interrupted() {
    let mut s = HookState::new();
    tool(&mut s, "PreToolUse", "Bash", json!({"command": "ls"}));
    fire(&mut s, "PostToolUseFailure", json!({"is_interrupt": false}));
    assert_eq!(s.status, ClaudeStatus::Working);
    assert_eq!(s.active_tool, None);
    tool(&mut s, "PreToolUse", "Bash", json!({"command": "ls"}));
    fire(&mut s, "PostToolUseFailure", json!({"is_interrupt": true}));
    assert_eq!(s.status, ClaudeStatus::Idle);
}

#[test]
fn stop_failure_rests_with_the_error_as_message() {
    let mut s = HookState {
        status: ClaudeStatus::Working,
        ..HookState::new()
    };
    fire(&mut s, "StopFailure", json!({"error": "rate_limit"}));
    assert_eq!(s.status, ClaudeStatus::Idle);
    assert_eq!(s.last_message.as_deref(), Some("rate_limit"));
}

#[test]
fn session_end_with_resume_or_clear_is_ready() {
    for reason in ["resume", "clear"] {
        let mut s = HookState::new();
        fire(&mut s, "SessionStart", json!({}));
        fire(&mut s, "SessionEnd", json!({"reason": reason}));
        assert_eq!(s.status, ClaudeStatus::Ready);
    }
    let mut s = HookState::new();
    fire(&mut s, "SessionEnd", json!({"end_reason": "clear"}));
    assert_eq!(s.status, ClaudeStatus::Ready);
}

#[test]
fn settle_moves_starting_to_ready_once() {
    let mut s = HookState::new();
    assert!(s.settle());
    assert_eq!(s.status, ClaudeStatus::Ready);
    assert_eq!(s.subtitle(&now()), "Pronto");

    let mut s = HookState::new();
    fire(&mut s, "UserPromptSubmit", json!({"prompt": "oi"}));
    assert!(!s.settle());
    assert_eq!(s.status, ClaudeStatus::Working);
}

#[test]
fn subagent_tool_events_do_not_change_the_parent_tool() {
    let mut s = HookState::new();
    tool(
        &mut s,
        "PreToolUse",
        "Agent",
        json!({"prompt": "investiga"}),
    );
    fire(
        &mut s,
        "PreToolUse",
        json!({"tool_name": "Grep", "tool_input": {"pattern": "x"}, "agent_id": "a03"}),
    );
    assert_eq!(s.active_tool.as_deref(), Some("Agent"));
    assert_eq!(s.tools, 2);
}

#[test]
fn forks_are_counted_by_spawn_and_by_distinct_agent_ids() {
    let mut s = HookState::new();
    fire(&mut s, "UserPromptSubmit", json!({"prompt": "revisa"}));
    tool(&mut s, "PreToolUse", "Agent", json!({}));
    tool(&mut s, "PreToolUse", "Agent", json!({}));
    fire(&mut s, "Stop", json!({}));
    assert_eq!(s.forks_out, 2);
    fire(
        &mut s,
        "PostToolUse",
        json!({"tool_name": "Read", "tool_input": {}, "agent_id": "a1"}),
    );
    assert_eq!(s.forks_out, 2);
    fire(&mut s, "SubagentStop", json!({"agent_id": "a1"}));
    assert_eq!(s.forks_out, 1);
    assert!(s.busy_forks());
    fire(&mut s, "SubagentStop", json!({"agent_id": "a2"}));
    assert_eq!(s.forks_out, 0);
    fire(&mut s, "SubagentStop", json!({}));
    assert_eq!(s.forks_out, 0);
    assert_eq!(s.status, ClaudeStatus::Idle);
}

#[test]
fn exit_plan_mode_captures_plans_deduplicated_and_capped() {
    let mut s = HookState::new();
    for i in 0..10 {
        tool(
            &mut s,
            "PreToolUse",
            "ExitPlanMode",
            json!({"plan": format!("# Plano {i}\nfaz")}),
        );
        tool(
            &mut s,
            "PreToolUse",
            "ExitPlanMode",
            json!({"plan": format!("# Plano {i}\nfaz")}),
        );
    }
    tool(&mut s, "PreToolUse", "ExitPlanMode", json!({"plan": "   "}));
    assert_eq!(s.plans.len(), 8);
    assert_eq!(s.plan().unwrap().headline(), "Plano 9");
}

#[test]
fn non_string_session_id_or_permission_mode_is_ignored() {
    let mut s = HookState::new();
    fire(&mut s, "SessionStart", json!({"permission_mode": "plan"}));
    apply_with_home(
        &mut s,
        "UserPromptSubmit",
        &json!({"session_id": 42, "permission_mode": ["x"], "prompt": "oi"}),
        now(),
        None,
    );
    assert_eq!(s.session_id.as_deref(), Some("sess-1"));
    assert_eq!(s.permission_mode.as_deref(), Some("plan"));
    assert_eq!(s.status, ClaudeStatus::Working);
}

#[test]
fn user_prompt_without_text_clears_the_last_prompt() {
    let mut s = HookState::new();
    fire(&mut s, "UserPromptSubmit", json!({"prompt": "oi"}));
    fire(&mut s, "UserPromptSubmit", json!({}));
    assert_eq!(s.last_prompt, None);
    assert_eq!(s.subtitle(&now()), "Trabalhando");
}

#[test]
fn tool_subtitle_counts_elapsed_seconds() {
    let mut s = HookState::new();
    tool(&mut s, "PreToolUse", "Bash", json!({"command": "ls"}));
    let later = now() + chrono::Duration::seconds(7);
    assert_eq!(s.subtitle(&later), "Bash(ls) 7s");
}

#[test]
fn every_event_stamps_last_event_at() {
    let mut s = HookState::new();
    fire(&mut s, "Whatever", json!({}));
    assert_eq!(s.last_event_at, Some(now()));
    assert_eq!(s.status, ClaudeStatus::Starting);
}
