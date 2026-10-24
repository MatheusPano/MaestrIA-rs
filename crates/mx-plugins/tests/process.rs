mod common;

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{
    BOARD, DEAF, ECHO_ENV, MORRE, Recorder, SH_PLUGIN, SILENT, TAKER, obj, plugins_in, until, write_plugin,
};
use mx_plugins::process::{ConnHandlers, Connection};
use mx_plugins::{Permission, PluginState, Plugins, RpcError};
use serde_json::{Value, json};

fn sh_setup(tmp: &Path) -> (Plugins, Arc<Recorder>) {
    write_plugin(
        &tmp.join("plugins"),
        "sh",
        json!({"id": "sh", "version": "1", "main": ["./plugin.sh"],
            "activationEvents": ["onCommand:oi", "onSession"],
            "contributes": {"commands": [{"id": "oi", "title": "dizer oi"}]}}),
        &[("plugin.sh", SH_PLUGIN)],
    );
    let plugins = plugins_in(tmp);
    plugins.scan();
    let rec = Recorder::new(Value::Null);
    plugins.set_calls(rec.clone());
    (plugins, rec)
}

fn state(p: &Plugins, id: &str) -> PluginState {
    p.get(id).unwrap().state
}

#[tokio::test]
async fn a_plugin_without_on_startup_does_not_start_with_the_app() {
    let tmp = tempfile::tempdir().unwrap();
    let (plugins, _) = sh_setup(tmp.path());
    plugins.startup();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(state(&plugins, "sh"), PluginState::Idle);
}

#[tokio::test]
async fn invoking_a_command_starts_the_process_routes_its_reverse_request_and_logs_bad_stdout() {
    let tmp = tempfile::tempdir().unwrap();
    let (plugins, rec) = sh_setup(tmp.path());
    let cmd = plugins.commands()[0].clone();
    let out = plugins.invoke(&cmd, obj(json!({"cwd": "/repo"}))).await.unwrap();
    assert_eq!(out, json!("feito"));
    assert_eq!(rec.calls(), [r#"window.showBanner {"text":"oi"}"#]);
    assert_eq!(state(&plugins, "sh"), PluginState::Running);
    assert!(until(|| plugins.log_of("sh").iter().any(|l| l.contains("stdout: isto não é json"))).await);
    assert!(plugins.log_of("sh").iter().any(|l| l.contains("comando oi")));
    plugins.stop_all().await;
}

#[tokio::test]
async fn a_declared_event_starts_the_process_and_delivers_it() {
    let tmp = tempfile::tempdir().unwrap();
    let (plugins, _) = sh_setup(tmp.path());
    plugins.emit("session.opened", obj(json!({"tabId": "tab1"})), &["onSession"], None);
    assert!(until(|| plugins.log_of("sh").iter().any(|l| l.ends_with("evento"))).await);
    plugins.stop_all().await;
}

#[tokio::test]
async fn an_undeclared_event_or_one_needing_a_missing_permission_does_not_start_it() {
    let tmp = tempfile::tempdir().unwrap();
    let (plugins, _) = sh_setup(tmp.path());
    plugins.emit("hook", obj(json!({"name": "Stop"})), &["onHook:Stop"], None);
    plugins.emit("hook", obj(json!({"name": "Stop"})), &["onSession"], Some(Permission::Hooks));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(state(&plugins, "sh"), PluginState::Idle);
}

#[tokio::test]
async fn restart_brings_it_back_running_and_stop_all_returns_it_to_idle() {
    let tmp = tempfile::tempdir().unwrap();
    let (plugins, _) = sh_setup(tmp.path());
    let cmd = plugins.commands()[0].clone();
    plugins.invoke(&cmd, obj(json!({}))).await.unwrap();
    plugins.restart("sh").await;
    assert_eq!(state(&plugins, "sh"), PluginState::Running);
    plugins.stop_all().await;
    assert_eq!(state(&plugins, "sh"), PluginState::Idle);
    assert!(plugins.log_of("sh").iter().any(|l| l.ends_with("processo parado")));
}

#[tokio::test]
async fn a_missing_executable_crashes_with_the_reason() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        &tmp.path().join("plugins"),
        "x",
        json!({"id": "x", "version": "1", "main": ["programa-que-nao-existe-em-lugar-nenhum"]}),
        &[],
    );
    let plugins = plugins_in(tmp.path());
    plugins.scan();
    plugins.startup();
    assert!(until(|| state(&plugins, "x") == PluginState::Crashed).await);
    assert_eq!(
        plugins.get("x").unwrap().crash.unwrap(),
        "não achei \"programa-que-nao-existe-em-lugar-nenhum\" — nem na pasta do plugin, nem no PATH"
    );
}

#[tokio::test]
async fn a_process_that_dies_at_once_is_crashed_and_events_do_not_revive_it() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        &tmp.path().join("plugins"),
        "m",
        json!({"id": "m", "version": "1", "main": ["./morre.sh"], "activationEvents": ["onSession"]}),
        &[("morre.sh", MORRE)],
    );
    let plugins = plugins_in(tmp.path());
    plugins.scan();
    plugins.emit("session.opened", obj(json!({})), &["onSession"], None);
    assert!(until(|| state(&plugins, "m") == PluginState::Crashed).await);
    let before = plugins.log_of("m").len();
    plugins.emit("session.opened", obj(json!({})), &["onSession"], None);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(plugins.log_of("m").len(), before);
    assert_eq!(state(&plugins, "m"), PluginState::Crashed);
}

#[tokio::test]
async fn a_command_revives_a_crashed_plugin_and_reports_why_it_could_not_start() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        &tmp.path().join("plugins"),
        "m",
        json!({"id": "m", "version": "1", "main": ["./morre.sh"],
            "contributes": {"commands": [{"id": "go", "title": "Go"}]}}),
        &[("morre.sh", MORRE)],
    );
    let plugins = plugins_in(tmp.path());
    plugins.scan();
    let cmd = plugins.commands()[0].clone();
    let e = plugins.invoke(&cmd, obj(json!({}))).await.unwrap_err();
    assert_eq!(e.code, RpcError::UNAVAILABLE);
    assert!(e.message.starts_with("o initialize falhou: "), "{}", e.message);
    assert!(plugins.log_of("m").iter().any(|l| l.contains("comando go falhou: ")));
}

#[tokio::test]
async fn the_process_gets_the_plugin_env_and_initialize_params() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = write_plugin(
        &tmp.path().join("plugins"),
        "env",
        json!({"id": "env", "version": "1", "main": ["./echo.sh"], "permissions": ["hooks", "notifications"],
            "contributes": {"settings": [{"id": "n", "type": "number", "default": 3}]}}),
        &[("echo.sh", ECHO_ENV)],
    );
    let plugins = plugins_in(tmp.path());
    plugins.scan();
    plugins.startup();
    assert!(until(|| state(&plugins, "env") == PluginState::Running).await);
    let data = tmp.path().join("plugin-data/env");
    let env = std::fs::read_to_string(data.join("env.txt")).unwrap();
    assert_eq!(env.trim(), format!("1|env|{}|{}", dir.display(), data.display()));
    let line = std::fs::read_to_string(data.join("initialize.json")).unwrap();
    let expected = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"apiVersion":1,"blocks":2,"rfw":2,"floats":1,"input":1,"form":1,"modal":1,"editor":1,"pluginId":"env","pluginDir":"{}","dataDir":"{}","permissions":["hooks","notifications"],"settings":{{"n":3}}}}}}"#,
        dir.display(),
        data.display()
    );
    assert_eq!(line.trim(), expected);
    plugins.stop_all().await;
}

#[tokio::test]
async fn concurrent_starts_share_one_process() {
    let tmp = tempfile::tempdir().unwrap();
    let (plugins, _) = sh_setup(tmp.path());
    let (a, b) = tokio::join!(plugins.start("sh"), plugins.start("sh"));
    let (a, b) = (a.unwrap(), b.unwrap());
    assert!(Arc::ptr_eq(&a, &b));
    let spawned = plugins.log_of("sh").iter().filter(|l| l.contains(" subiu: ")).count();
    assert_eq!(spawned, 1);
    plugins.stop_all().await;
}

#[tokio::test]
async fn an_unexpected_exit_marks_the_crash_and_clears_busy_commands() {
    let tmp = tempfile::tempdir().unwrap();
    let (plugins, _) = sh_setup(tmp.path());
    let conn = plugins.start("sh").await.unwrap();
    plugins.set_busy("sh/oi", true);
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(conn.pid() as i32), nix::sys::signal::Signal::SIGKILL)
        .unwrap();
    assert!(until(|| state(&plugins, "sh") == PluginState::Crashed).await);
    assert_eq!(plugins.get("sh").unwrap().crash.unwrap(), "o processo saiu (código -9)");
    assert!(!plugins.plugin_busy("sh"));
}

#[tokio::test]
async fn a_suggestion_reaches_the_taker_and_its_answer_comes_back_as_a_call() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        &tmp.path().join("plugins"),
        "tracker",
        json!({"id": "tracker", "version": "1", "main": ["./plugin.sh"], "permissions": ["sessions.create"],
            "contributes": {"suggestions": {"label": "criar a task no Wiboor"}}}),
        &[("plugin.sh", TAKER)],
    );
    let plugins = plugins_in(tmp.path());
    plugins.scan();
    let rec = Recorder::new(json!({"tabId": "nova"}));
    plugins.set_calls(rec.clone());
    let takers: Vec<String> = plugins.suggestion_takers().into_iter().map(|p| p.id).collect();
    assert_eq!(takers, ["tracker"]);
    let params = json!({"suggestion": {"id": "task_00000001", "title": "t", "tldr": "", "prompt": "p"},
        "choice": "here", "origin": {"tabId": "a", "root": "/repo", "isRepo": true}});
    plugins.offer_suggestion("tracker", params).await.unwrap();
    assert!(until(|| rec.calls().len() == 1).await);
    let call = &rec.calls()[0];
    assert!(call.starts_with("suggestion.created"));
    assert!(call.contains(r#""suggestionId":"task_00000001""#));
    assert!(call.contains(r#""ref":"TASK#1""#));
    plugins.set_enabled("tracker", false).await;
    assert!(plugins.suggestion_takers().is_empty());
}

#[tokio::test]
async fn the_home_command_of_a_screen_plugin_opens_its_view_through_the_host() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        &tmp.path().join("plugins"),
        "wiboor",
        json!({"id": "wiboor", "version": "1", "name": "Wiboor", "icon": "terminal", "main": ["./plugin.sh"],
            "contributes": {"screen": {"home": "quadro"}, "commands": [{"id": "quadro", "title": "wiboor: o quadro"}]}}),
        &[("plugin.sh", BOARD)],
    );
    let plugins = plugins_in(tmp.path());
    plugins.scan();
    let rec = Recorder::new(json!({"tabId": "tab9"}));
    plugins.set_calls(rec.clone());
    let cmd = plugins.command_by_id("wiboor/quadro").unwrap();
    plugins.invoke(&cmd, obj(json!({}))).await.unwrap();
    assert_eq!(rec.calls(), [r#"view.open {"viewId":"quadro","title":"Quadro","blocks":[]}"#]);
    plugins.stop_all().await;
}

#[tokio::test]
async fn a_reverse_request_before_the_window_is_wired_does_not_break_the_command() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        &tmp.path().join("plugins"),
        "wiboor",
        json!({"id": "wiboor", "version": "1", "main": ["./plugin.sh"],
            "contributes": {"commands": [{"id": "quadro", "title": "q"}]}}),
        &[("plugin.sh", BOARD)],
    );
    let plugins = plugins_in(tmp.path());
    plugins.scan();
    let cmd = plugins.commands()[0].clone();
    assert_eq!(plugins.invoke(&cmd, obj(json!({}))).await.unwrap(), Value::Null);
    plugins.stop_all().await;
}

#[tokio::test]
async fn a_disabled_plugin_refuses_commands() {
    let tmp = tempfile::tempdir().unwrap();
    let (plugins, _) = sh_setup(tmp.path());
    let cmd = plugins.commands()[0].clone();
    plugins.set_enabled("sh", false).await;
    let e = plugins.invoke(&cmd, obj(json!({}))).await.unwrap_err();
    assert_eq!(e, RpcError::unavailable("o plugin não está ligado"));
}

fn quiet_handlers() -> ConnHandlers {
    ConnHandlers {
        on_request: Arc::new(|_, _| Box::pin(async { Ok(Value::Null) })),
        on_log: Arc::new(|_| {}),
        on_exit: Box::new(|_| {}),
    }
}

fn spawn_script(tmp: &Path, name: &str, text: &str) -> Arc<Connection> {
    let dir = write_plugin(tmp, "c", json!({}), &[(name, text)]);
    let env: Vec<(String, String)> = std::env::vars().collect();
    Connection::spawn(&dir.join(name), &[], &dir, &env, quiet_handlers()).unwrap()
}

#[tokio::test]
async fn a_request_without_answer_times_out_with_the_method_name() {
    let tmp = tempfile::tempdir().unwrap();
    let conn = spawn_script(tmp.path(), "s.sh", SILENT);
    let e = conn.request("initialize", None, Duration::from_secs(1)).await.unwrap_err();
    assert_eq!(e, RpcError::unavailable("\"initialize\" ficou sem resposta por 1s"));
    conn.close(Duration::from_millis(800)).await;
    assert!(!conn.alive());
}

#[tokio::test]
async fn close_escalates_to_sigkill_when_the_process_ignores_shutdown_and_sigterm() {
    let tmp = tempfile::tempdir().unwrap();
    let conn = spawn_script(tmp.path(), "d.sh", DEAF);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let t = Instant::now();
    conn.close(Duration::from_millis(300)).await;
    assert!(!conn.alive());
    assert!(t.elapsed() >= Duration::from_millis(600));
    assert_eq!(conn.exit_code(), Some(-9));
    let e = conn.request("x", None, Duration::from_secs(1)).await.unwrap_err();
    assert_eq!(e, RpcError::unavailable("o plugin não está rodando"));
}
