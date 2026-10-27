use chrono::TimeDelta;
use mx_core::model::{ClaudeStatus, FeatureOrHotfixKind};
use mx_sys::FakeRunner;
use serde_json::json;

use super::Rig;
use crate::{Areas, ClaudeSpec, StoreMsg};

fn open(rig: &mut Rig) -> String {
    let f = rig.s1();
    rig.s.open_claude(ClaudeSpec::new(&f, "/repo"))
}

#[tokio::test]
async fn launch_settles_after_three_seconds() {
    let mut rig = Rig::new();
    let id = open(&mut rig);
    rig.s.settle().await;
    assert_eq!(rig.t(&id).status(), ClaudeStatus::Starting);
    rig.advance(TimeDelta::milliseconds(2900));
    assert_eq!(rig.t(&id).status(), ClaudeStatus::Starting);
    rig.advance(TimeDelta::milliseconds(100));
    assert_eq!(rig.t(&id).status(), ClaudeStatus::Ready);
}

#[tokio::test]
async fn launch_command_keeps_mcp_config_first() {
    let mut rig = Rig::new();
    let f = rig.s1();
    let feat = rig.s.add_feature("/repo", "login", "contexto", FeatureOrHotfixKind::Feature);
    let mut spec = ClaudeSpec::new(&f, "/repo");
    spec.label = Some("job".into());
    spec.feature = Some(feat);
    spec.resume_id = Some("abc".into());
    spec.prompt = Some(" faz ".into());
    let id = rig.s.open_claude(spec);
    let log = rig.terms.log(&id);
    let cmd = &log.commands[0];
    assert!(cmd.starts_with("claude --mcp-config '{\"mcpServers\""));
    assert!(cmd.contains(" --name 'job' --settings '"));
    assert!(cmd.ends_with("--append-system-prompt 'contexto' --resume 'abc' -- 'faz'"));
    assert_eq!(
        log.displays[0].as_deref(),
        Some("claude --mcp-config <tarefas :4242> --name 'job' --settings <hooks :4242> --append-system-prompt <briefing de login> --resume … <prompt inicial>")
    );
    rig.s.settle().await;
}

#[tokio::test]
async fn an_early_exit_is_explained() {
    let mut rig = Rig::new();
    let id = open(&mut rig);
    rig.terms.exit(&id, 127);
    rig.s.apply(StoreMsg::TermExited { tab_id: id.clone(), code: Some(127) });
    assert!(rig.t(&id).exited);
    assert!(rig.s.banner().unwrap().contains("não achei o `claude` no PATH"));
    assert!(rig.bus.names().contains(&"session.exited".to_owned()));
    rig.s.settle().await;
}

#[tokio::test]
async fn a_claude_that_dies_mid_work_stops_spinning() {
    let mut rig = Rig::new();
    let id = open(&mut rig);
    rig.tm(&id).hooks.status = ClaudeStatus::Working;
    rig.terms.exit(&id, 1);
    rig.s.apply(StoreMsg::TermExited { tab_id: id.clone(), code: Some(1) });
    assert_eq!(rig.t(&id).status(), ClaudeStatus::Ended);
    rig.s.settle().await;
}

#[tokio::test]
async fn a_claude_that_exits_while_waiting_keeps_its_status() {
    let mut rig = Rig::new();
    let id = open(&mut rig);
    rig.tm(&id).hooks.status = ClaudeStatus::WaitingPermission;
    rig.terms.exit(&id, 0);
    rig.s.apply(StoreMsg::TermExited { tab_id: id.clone(), code: Some(0) });
    assert_eq!(rig.t(&id).status(), ClaudeStatus::WaitingPermission);
    rig.s.settle().await;
}

#[tokio::test]
async fn a_stale_exit_after_restart_is_ignored() {
    let mut rig = Rig::new();
    let id = open(&mut rig);
    rig.s.apply(StoreMsg::TermExited { tab_id: id.clone(), code: Some(0) });
    assert!(!rig.t(&id).exited);
    rig.s.settle().await;
}

#[test]
fn banners_fade_after_six_seconds_unless_sticky() {
    let mut rig = Rig::new();
    rig.s.show_banner("oi");
    rig.advance(TimeDelta::seconds(6));
    assert!(rig.s.banner().is_none());
    rig.s.show_banner_sticky("fica");
    rig.advance(TimeDelta::minutes(5));
    assert_eq!(rig.s.banner(), Some("fica"));
}

#[test]
fn reveal_lights_up_for_two_seconds() {
    let mut rig = Rig::new();
    let f = rig.s1();
    rig.panel("um");
    rig.s.toggle_collapsed(&f.root);
    rig.s.reveal_tab("um");
    assert!(!rig.s.find_folder("/repo").unwrap().collapsed);
    assert!(rig.s.take_reveal_scroll("um", None));
    assert!(!rig.s.take_reveal_scroll("um", None));
    rig.advance(TimeDelta::seconds(2));
    assert!(rig.s.revealed().is_none());
}

#[test]
fn status_line_updates_only_on_change() {
    let mut rig = Rig::new();
    rig.s1();
    rig.panel("t1");
    let body = json!({"context_window": {"used_percentage": 58.4, "context_window_size": 200000,
        "current_usage": {"input_tokens": 10, "cache_creation_input_tokens": 1000, "cache_read_input_tokens": 115000}}});
    rig.s.take_changes();
    rig.s.apply_status("t1", &body);
    let c = rig.t("t1").context.unwrap();
    assert_eq!((c.percent, c.tokens, c.window), (58, 116010, 200000));
    assert!(!rig.s.take_changes().is_empty());
    rig.s.apply_status("t1", &body);
    assert!(rig.s.take_changes().is_empty());
    rig.s.apply_status("t1", &json!({"context_window": {"used_percentage": null}}));
    assert!(rig.t("t1").context.is_none());
    rig.s.apply_status("ghost", &body);
}

#[tokio::test]
async fn idle_ticks_mark_nothing() {
    let mut rig = Rig::new();
    rig.s1();
    for id in ["a", "b"] {
        rig.panel(id);
        rig.tm(id).session_id = Some(format!("s-{id}"));
        rig.turn(id);
    }
    rig.s.start();
    for _ in 0..21 {
        rig.advance(TimeDelta::seconds(1));
        rig.s.settle().await;
    }
    rig.s.take_changes();
    for _ in 0..19 {
        rig.advance(TimeDelta::seconds(1));
        rig.s.settle().await;
        assert!(rig.s.take_changes().is_empty());
    }
}

#[tokio::test]
async fn a_running_tool_ticks_its_tab_every_second() {
    let mut rig = Rig::new();
    rig.s1();
    rig.panel("t");
    rig.hook("t", "PreToolUse", json!({"tool_name": "Bash"}));
    rig.s.start();
    rig.s.settle().await;
    rig.s.take_changes();
    rig.advance(TimeDelta::seconds(1));
    rig.s.settle().await;
    assert_eq!(rig.s.take_changes().tabs, ["t"]);
}

#[tokio::test]
async fn agents_poll_teaches_session_ids_by_pid() {
    let runner = FakeRunner::new();
    runner.respond("claude agents --json", mx_sys::output(0, "x [{\"pid\": 1000, \"sessionId\": \"sid\", \"name\": \"nome\"}]", ""));
    let mut rig = Rig::with_runner(runner);
    let id = open(&mut rig);
    rig.terms.give_pid(&id);
    rig.s.start();
    rig.s.settle().await;
    assert_eq!(rig.t(&id).session_id.as_deref(), Some("sid"));
    assert_eq!(rig.t(&id).agent_name.as_deref(), Some("nome"));
    assert!(rig.s.save_pending());
    let polls = || rig.runner.commands().iter().filter(|c| c.starts_with("claude agents")).count();
    assert_eq!(polls(), 1);
}

#[tokio::test]
async fn eager_polling_is_every_two_seconds_idle_every_thirty() {
    let mut rig = Rig::new();
    let id = open(&mut rig);
    rig.s.start();
    rig.s.settle().await;
    let polls = |rig: &Rig| rig.runner.commands().iter().filter(|c| c.starts_with("claude agents")).count();
    rig.advance(TimeDelta::seconds(2));
    rig.s.settle().await;
    assert_eq!(polls(&rig), 2);
    rig.tm(&id).session_id = Some("x".into());
    rig.s.take_changes();
    rig.advance(TimeDelta::seconds(2));
    rig.s.settle().await;
    assert_eq!(polls(&rig), 2);
    rig.advance(TimeDelta::seconds(30));
    rig.s.settle().await;
    assert_eq!(polls(&rig), 3);
}

#[tokio::test]
async fn git_refresh_learns_worktrees_and_is_quiet_when_unchanged() {
    let runner = FakeRunner::new();
    runner.respond(
        "git worktree list",
        mx_sys::output(0, "worktree /repo\nbranch refs/heads/master\n\nworktree /repo/.claude/worktrees/T-1\nbranch refs/heads/feature/T-1\n", ""),
    );
    let mut rig = Rig::with_runner(runner);
    rig.s1();
    rig.shell("sh");
    rig.s.refresh_git();
    rig.s.settle().await;
    let f = rig.s.find_folder("/repo").unwrap();
    assert!(f.is_repo);
    assert_eq!(f.branch, "master");
    assert_eq!(rig.s.worktrees_of("/repo").len(), 2);
    rig.s.take_changes();
    rig.s.refresh_git();
    rig.s.settle().await;
    assert!(!rig.s.take_changes().areas.intersects(Areas::SIDEBAR));
}

#[tokio::test]
async fn shutdown_writes_the_pending_config_before_killing() {
    let mut rig = Rig::new();
    let id = open(&mut rig);
    rig.s.settle().await;
    assert!(rig.s.save_pending());
    rig.s.shutdown().await;
    assert!(rig.s.options().config_path().exists());
    assert_eq!(rig.terms.log(&id).kills, 1);
}

#[tokio::test]
async fn new_task_creates_a_worktree_and_opens_claude_there() {
    let runner = FakeRunner::new();
    runner.respond("git symbolic-ref", mx_sys::output(0, "refs/remotes/origin/main", ""));
    runner.respond("git rev-parse --verify", mx_sys::output(0, "", ""));
    runner.respond("git worktree add", mx_sys::output(0, "", ""));
    let mut rig = Rig::with_runner(runner);
    rig.s1();
    rig.s.new_task(crate::NewTask {
        folder_root: "/repo".into(),
        task_id: "TASK#1".into(),
        branch_pattern: "feature/{id}".into(),
        dir_pattern: "{id}".into(),
        setup_command: Some("npm i".into()),
        ..Default::default()
    });
    rig.s.settle().await;
    let add = rig.runner.commands().into_iter().find(|c| c.starts_with("git worktree add")).unwrap();
    assert_eq!(add, "git worktree add '/repo/.claude/worktrees/TASK-1' -b 'feature/TASK#1' 'origin/main'");
    let labels: Vec<String> = rig.s.tabs().iter().filter_map(|t| t.custom_label.clone()).collect();
    assert_eq!(labels, ["TASK#1 setup", "TASK#1"]);
    assert_eq!(rig.s.banner(), Some("Worktree criada em /repo/.claude/worktrees/TASK-1 (branch feature/TASK#1)"));
}

#[test]
fn removing_a_worktree_with_an_open_panel_is_refused() {
    let mut rig = Rig::new();
    rig.s1();
    rig.add_tab("w", "/repo", mx_core::model::TabKind::Shell, "/repo/wt", None);
    let w = mx_core::model::WorktreeInfo { path: "/repo/wt".into(), branch: "b".into(), is_main: false, prunable: false };
    rig.s.remove_worktree("/repo", &w, false, None, false);
    assert_eq!(rig.s.banner(), Some("Feche o painel que está nessa worktree antes de excluí-la"));
}

#[test]
fn editor_for_a_missing_path_names_it() {
    let mut rig = Rig::new();
    rig.s.open_in_editor("/nao/existe.code-workspace");
    assert_eq!(rig.s.banner(), Some("Esse workspace não está mais lá: /nao/existe.code-workspace"));
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_stops_the_running_plugins() {
    use std::os::unix::fs::PermissionsExt;
    let mut rig = Rig::new();
    let root = rig.mkdir("plugins");
    let dir = std::path::Path::new(&root).join("p");
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = serde_json::json!({"id": "p", "version": "1", "main": ["./p.sh"]});
    std::fs::write(dir.join(mx_plugins::MANIFEST_NAME), manifest.to_string()).unwrap();
    let script = "#!/bin/sh\nwhile IFS= read -r l; do case \"$l\" in *initialize*) id=$(echo \"$l\" | sed 's/.*\"id\":\\([0-9]*\\).*/\\1/'); echo \"{\\\"jsonrpc\\\":\\\"2.0\\\",\\\"id\\\":$id,\\\"result\\\":{}}\";; esac; done\n";
    std::fs::write(dir.join("p.sh"), script).unwrap();
    std::fs::set_permissions(dir.join("p.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    rig.s.attach_plugins(mx_plugins::Plugins::with_dirs(root.into(), rig.mkdir("plugin-data").into()));
    let plugins = rig.s.plugins().unwrap().clone();
    let conn = plugins.start("p").await.expect("plugin started");
    assert!(conn.alive());
    rig.s.shutdown().await;
    assert!(!conn.alive());
}
