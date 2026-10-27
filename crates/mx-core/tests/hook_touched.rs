use chrono::{Local, TimeZone};
use mx_core::hook::{apply_with_home, shell_markdown};
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

fn bash_touched(command: &str) -> Vec<String> {
    let mut s = HookState::new();
    tool(&mut s, "PostToolUse", "Bash", json!({"command": command}));
    s.touched
}

#[test]
fn permission_prompt_during_bash_wants_approval() {
    let mut s = HookState::new();
    tool(
        &mut s,
        "PreToolUse",
        "Bash",
        json!({"command": "rm -rf build"}),
    );
    fire(
        &mut s,
        "Notification",
        json!({"notification_type": "permission_prompt"}),
    );
    assert_eq!(s.status, ClaudeStatus::WaitingPermission);
    assert_eq!(s.status.call_to_action(), "quer aprovação");
    assert_eq!(s.subtitle(&now()), "Quer aprovação pra Bash");
}

#[test]
fn a_long_bash_target_is_clipped_with_ellipsis() {
    let mut s = HookState::new();
    tool(
        &mut s,
        "PreToolUse",
        "Bash",
        json!({"command": "flutter test --reporter compact -j 4 --coverage"}),
    );
    let target = s.last_tool_target.unwrap();
    assert!(target.chars().count() <= 29);
    assert!(target.ends_with('…'));
}

#[test]
fn post_tool_use_records_writes_but_not_reads() {
    let mut s = HookState::new();
    tool(
        &mut s,
        "PostToolUse",
        "Write",
        json!({"file_path": "/repo/a.dart"}),
    );
    tool(
        &mut s,
        "PostToolUse",
        "Edit",
        json!({"file_path": "/repo/b.dart"}),
    );
    tool(
        &mut s,
        "PostToolUse",
        "MultiEdit",
        json!({"file_path": "/repo/c.dart"}),
    );
    tool(
        &mut s,
        "PostToolUse",
        "NotebookEdit",
        json!({"notebook_path": "/repo/d.ipynb"}),
    );
    tool(
        &mut s,
        "PostToolUse",
        "Read",
        json!({"file_path": "/repo/e.dart"}),
    );
    tool(
        &mut s,
        "PostToolUse",
        "Grep",
        json!({"path": "/repo/f.dart"}),
    );
    assert_eq!(
        s.touched,
        [
            "/repo/a.dart",
            "/repo/b.dart",
            "/repo/c.dart",
            "/repo/d.ipynb"
        ]
    );
}

#[test]
fn bash_redirects_and_heredocs_into_markdown_are_touched() {
    assert_eq!(
        bash_touched("printf \"# oi\\n\" > notas.md"),
        ["/repo/notas.md"]
    );
    assert_eq!(
        bash_touched("echo linha >> docs/DIARIO.md"),
        ["/repo/docs/DIARIO.md"]
    );
    assert_eq!(
        bash_touched("cat > 'meu plano.md' <<'EOF'\n# plano\nEOF"),
        ["/repo/meu plano.md"]
    );
}

#[test]
fn bash_tee_sed_in_place_and_cp_target_markdown_are_touched() {
    assert_eq!(bash_touched("cat x | tee resumo.md"), ["/repo/resumo.md"]);
    assert_eq!(
        bash_touched("sed -i '' 's/a/b/' LEIAME.md"),
        ["/repo/LEIAME.md"]
    );
    assert_eq!(
        bash_touched("cp modelo.md /tmp/copia.md"),
        ["/tmp/copia.md"]
    );
}

#[test]
fn heredoc_bodies_are_not_read_as_commands() {
    assert_eq!(
        bash_touched("cat > nota.md <<'MD'\n> veja o inexistente.md\nMD"),
        ["/repo/nota.md"]
    );
}

#[test]
fn reads_greps_and_moves_into_directories_touch_nothing() {
    for cmd in [
        "cat PLANO.md",
        "grep -n TODO docs/*.md",
        "mv relatorio.md docs/",
    ] {
        assert!(bash_touched(cmd).is_empty(), "{cmd}");
    }
}

#[test]
fn variables_globs_and_stderr_merges_touch_nothing() {
    for cmd in [
        "echo oi > \"$OUT/nota.md\"",
        "sed -i \"\" s/a/b/ *.md",
        "dart run tool/gen.dart 2>&1",
    ] {
        assert!(bash_touched(cmd).is_empty(), "{cmd}");
    }
}

#[test]
fn redirects_to_non_markdown_touch_nothing() {
    for cmd in [
        "dart format lib/main.dart > /dev/null",
        "python3 gera.py > saida.json",
    ] {
        assert!(bash_touched(cmd).is_empty(), "{cmd}");
    }
}

#[test]
fn relative_writes_resolve_against_cwd() {
    let mut s = HookState::new();
    tool(
        &mut s,
        "PostToolUse",
        "Write",
        json!({"file_path": "./saida/lista.csv"}),
    );
    tool(
        &mut s,
        "PostToolUse",
        "Write",
        json!({"file_path": "notas.md"}),
    );
    assert_eq!(s.touched, ["/repo/saida/lista.csv", "/repo/notas.md"]);
}

#[test]
fn touched_keeps_first_touch_order_without_duplicates() {
    let mut s = HookState::new();
    tool(
        &mut s,
        "PostToolUse",
        "Write",
        json!({"file_path": "/repo/a"}),
    );
    tool(
        &mut s,
        "PostToolUse",
        "Write",
        json!({"file_path": "/repo/b"}),
    );
    tool(
        &mut s,
        "PostToolUse",
        "Edit",
        json!({"file_path": "/repo/a"}),
    );
    assert_eq!(s.touched, ["/repo/a", "/repo/b"]);
}

#[test]
fn touched_is_capped_at_200_dropping_the_oldest() {
    let mut s = HookState::new();
    for i in 0..205 {
        tool(
            &mut s,
            "PostToolUse",
            "Write",
            json!({"file_path": format!("f{i}")}),
        );
    }
    assert_eq!(s.touched.len(), 200);
    assert_eq!(s.touched.first().unwrap(), "/repo/f5");
    assert_eq!(s.touched.last().unwrap(), "/repo/f204");
}

#[test]
fn empty_paths_missing_input_and_empty_payloads_touch_nothing() {
    let mut s = HookState::new();
    tool(&mut s, "PostToolUse", "Write", json!({"file_path": ""}));
    fire(&mut s, "PostToolUse", json!({"tool_name": "Write"}));
    apply_with_home(&mut s, "PostToolUse", &json!({}), now(), None);
    assert!(s.touched.is_empty());
}

#[test]
fn subagent_writes_are_still_touched() {
    let mut s = HookState::new();
    fire(
        &mut s,
        "PostToolUse",
        json!({"tool_name": "Edit", "tool_input": {"file_path": "/repo/x.dart"}, "agent_id": "a03"}),
    );
    assert!(s.touched.contains(&"/repo/x.dart".to_owned()));
}

#[test]
fn subagent_reads_keep_the_parent_working() {
    let mut s = HookState::new();
    fire(&mut s, "UserPromptSubmit", json!({"prompt": "divide"}));
    fire(
        &mut s,
        "PostToolUse",
        json!({"tool_name": "Read", "tool_input": {"file_path": "/repo/x"}, "agent_id": "a01"}),
    );
    assert_eq!(s.status, ClaudeStatus::Working);
    assert_eq!(s.subtitle(&now()), "divide");
}

#[test]
fn shell_markdown_tilde_targets_expand_home() {
    let mut s = HookState::new();
    tool(
        &mut s,
        "PostToolUse",
        "Bash",
        json!({"command": "echo x > ~/notas.md"}),
    );
    assert_eq!(s.touched, ["/home/u/notas.md"]);
    assert_eq!(shell_markdown("A=1 B=2 tee a.md b.txt"), ["a.md"]);
}
