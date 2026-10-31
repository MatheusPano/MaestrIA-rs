mod common;

use std::path::PathBuf;
use std::sync::Arc;

use common::host::FakeHost;
use common::{obj, plugins_in, until, write_plugin};
use mx_core::model::ContextUsage;
use mx_plugins::host_json::context_json;
use mx_plugins::{HostDispatcher, PluginInfo, PluginManifest, PluginState, RpcError};
use serde_json::{Value, json};

const RECORDER: &str = r#"#!/bin/sh
while IFS= read -r line; do
  echo "$line" >> "$MAESTRIA_PLUGIN_DATA/in.log"
  case "$line" in
    *'"method":"initialize"'*)
      id=$(echo "$line" | sed 's/.*"id":\([0-9]*\).*/\1/')
      echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{}}" ;;
    *'"method":"shutdown"'*)
      exit 0 ;;
  esac
done
"#;

fn info(manifest: Value) -> PluginInfo {
    let m = PluginManifest::parse_for(&obj(manifest), true).unwrap();
    PluginInfo {
        id: m.id.clone(),
        dir: PathBuf::from("/p"),
        data_dir: PathBuf::from("/d"),
        manifest: Some(Arc::new(m)),
        problem: None,
        linked: false,
        enabled: true,
        crash: None,
        state: PluginState::Idle,
        palettes: Vec::new(),
    }
}

fn with(perms: &[&str]) -> PluginInfo {
    info(json!({"id": "escritorio", "version": "1", "main": ["x"], "permissions": perms}))
}

async fn call(host: FakeHost, p: &PluginInfo, method: &str, params: Value) -> (Result<Value, RpcError>, Arc<FakeHost>) {
    let host = Arc::new(host);
    let d = HostDispatcher::new(host.clone());
    (d.dispatch(p, method, &obj(params)).await, host)
}

async fn run(p: &PluginInfo, method: &str, params: Value) -> Result<Value, RpcError> {
    call(FakeHost::default(), p, method, params).await.0
}

fn forbidden(perm: &str) -> RpcError {
    RpcError::forbidden(format!("o manifesto não declara a permissão \"{perm}\""))
}

#[tokio::test]
async fn each_new_session_method_asks_for_its_permission() {
    let none = with(&[]);
    for (method, perm) in [
        ("terminal.sendKeys", "terminal.write"),
        ("terminal.pasteClipboard", "terminal.write"),
        ("session.rename", "sessions.create"),
        ("session.wake", "sessions.create"),
        ("session.hibernate", "sessions.close"),
    ] {
        let e = run(&none, method, json!({"tabId": "t1", "title": "x", "keys": ["enter"]})).await.unwrap_err();
        assert_eq!(e, forbidden(perm), "{method}");
    }
    let create = with(&["sessions.create"]);
    let e = run(&create, "session.hibernate", json!({"tabId": "t1"})).await.unwrap_err();
    assert_eq!(e, forbidden("sessions.close"));
    assert!(run(&none, "session.portrait", json!({"tabId": "t1", "image": null})).await.is_ok());
}

#[tokio::test]
async fn rename_wake_hibernate_portrait_and_close_require_the_tab_id() {
    let all = with(&["sessions.create", "sessions.close"]);
    for method in ["session.rename", "session.wake", "session.hibernate", "session.portrait", "session.close"] {
        let e = run(&all, method, json!({"title": "x", "tabId": 7})).await.unwrap_err();
        assert_eq!(e, RpcError::invalid("\"tabId\" é obrigatório"), "{method}");
    }
}

#[tokio::test]
async fn results_carry_the_host_answers() {
    let all = with(&["sessions.create", "sessions.close"]);
    let out = run(&all, "session.rename", json!({"tabId": "t1", "title": "  login do google  "})).await;
    assert_eq!(out.unwrap(), json!({"title": "login do google"}));
    assert_eq!(run(&all, "session.wake", json!({"tabId": "t1"})).await.unwrap(), json!({"woke": true}));
    assert_eq!(run(&all, "session.hibernate", json!({"tabId": "t1"})).await.unwrap(), json!({"hibernated": true}));
    let e = run(&all, "session.rename", json!({"tabId": "t1"})).await.unwrap_err();
    assert_eq!(e, RpcError::invalid("\"title\" é obrigatório"));
}

#[tokio::test]
async fn send_keys_hands_an_invalid_list_to_the_host_as_none() {
    let w = with(&["terminal.write"]);
    let (_, host) = call(FakeHost::default(), &w, "terminal.sendKeys", json!({"keys": ["up", {"text": "oi"}]})).await;
    assert_eq!(host.calls(), [r#"keys SendKeys { tab_id: None, keys: Some([Up, Text("oi")]) }"#]);
    let (_, host) = call(FakeHost::default(), &w, "terminal.sendKeys", json!({"tabId": "t1", "keys": ["pgup"]})).await;
    assert_eq!(host.calls(), [r#"keys SendKeys { tab_id: Some("t1"), keys: None }"#]);
    let (_, host) = call(FakeHost::default(), &w, "terminal.pasteClipboard", json!({})).await;
    assert_eq!(host.calls(), ["paste None"]);
}

#[tokio::test]
async fn open_claude_in_background_is_passed_through() {
    let c = with(&["sessions.create"]);
    let (out, host) = call(FakeHost::default(), &c, "session.openClaude", json!({"background": true, "label": "a"})).await;
    assert_eq!(out.unwrap(), json!({"tabId": "tab7"}));
    assert_eq!(
        host.calls(),
        [r#"claude OpenClaude { cwd: None, prompt: None, label: Some("a"), background: true }"#]
    );
    let (_, host) = call(FakeHost::default(), &c, "session.openClaude", json!({})).await;
    assert!(host.calls()[0].ends_with("background: false }"));
}

#[tokio::test]
async fn a_portrait_of_null_or_absent_clears_it() {
    let p = with(&[]);
    let (_, host) = call(FakeHost::default(), &p, "session.portrait", json!({"tabId": "t1"})).await;
    assert_eq!(host.calls(), ["portrait t1 None"]);
    let (_, host) = call(FakeHost::default(), &p, "session.portrait", json!({"tabId": "t1", "image": "/p/a.png"})).await;
    assert_eq!(host.calls(), [r#"portrait t1 Some(String("/p/a.png"))"#]);
}

#[tokio::test]
async fn session_close_lets_sessions_close_end_a_user_claude_session_but_not_a_user_terminal() {
    let host = || FakeHost {
        owners: vec![("c1".into(), None), ("s1".into(), None), ("mine".into(), Some("q".into()))],
        claude: vec!["c1".into()],
        ..FakeHost::default()
    };
    let q = info(json!({"id": "q", "version": "1", "main": ["x"]}));
    let escritorio = with(&["sessions.close"]);
    let (e, h) = call(host(), &q, "session.close", json!({"tabId": "c1"})).await;
    assert_eq!(e.unwrap_err(), RpcError::forbidden("fechar uma sessão do claude pede a permissão \"sessions.close\""));
    assert!(h.calls().is_empty());
    let (e, _) = call(host(), &escritorio, "session.close", json!({"tabId": "s1"})).await;
    assert_eq!(e.unwrap_err(), RpcError::forbidden("só dá pra fechar um terminal que o plugin abriu com \"owned\""));
    let (e, _) = call(host(), &escritorio, "session.close", json!({})).await;
    assert_eq!(e.unwrap_err(), RpcError::invalid("\"tabId\" é obrigatório"));
    let (ok, h) = call(host(), &escritorio, "session.close", json!({"tabId": "c1"})).await;
    assert_eq!(ok.unwrap(), Value::Null);
    assert_eq!(h.calls(), ["close c1"]);
    let (ok, h) = call(host(), &q, "session.close", json!({"tabId": "mine"})).await;
    assert_eq!(ok.unwrap(), Value::Null);
    assert_eq!(h.calls(), ["close mine"]);
}

#[tokio::test]
async fn view_open_reads_the_anchor_and_the_requested_size() {
    let p = with(&[]);
    let params = json!({"viewId": "v", "modal": true, "from": "agente:1", "width": 500, "height": 400.5});
    let (_, host) = call(FakeHost::default(), &p, "view.open", params).await;
    assert_eq!(host.calls()[1], r#"view.open from Some("agente:1") Some(500.0)xSome(400.5)"#);
}

#[test]
fn session_json_context_is_null_until_reported() {
    assert_eq!(context_json(None), Value::Null);
    let c = ContextUsage { percent: 71, tokens: 142000, window: 200000 };
    assert_eq!(context_json(Some(&c)).to_string(), r#"{"percent":71,"tokens":142000,"window":200000}"#);
}

#[test]
fn the_capability_list_is_the_2_7_one_plus_picture_in_wire_order() {
    let keys: Vec<&str> = mx_plugins::CAPABILITIES.iter().map(|(k, _, _)| *k).collect();
    assert_eq!(
        keys,
        [
            "rfw", "floats", "input", "form", "modal", "editor", "sprites", "zoom", "textInput", "openInBackground",
            "markdown", "pasteImage", "keys", "arrows", "wake", "hibernate", "portrait", "closeSessions", "modalFrom",
            "selectable", "rename", "picture"
        ]
    );
    assert!(mx_plugins::CAPABILITIES.iter().all(|c| c.2));
    assert_eq!(mx_plugins::MAESTRIA_COMPAT, "2.7.0");
}

#[tokio::test]
async fn view_closed_reaches_only_a_running_owner_and_never_starts_it() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        &tmp.path().join("plugins"),
        "rec",
        json!({"id": "rec", "version": "1", "main": ["./rec.sh"], "activationEvents": ["onCommand:x"]}),
        &[("rec.sh", RECORDER)],
    );
    let plugins = plugins_in(tmp.path());
    plugins.scan();
    let log = tmp.path().join("plugin-data/rec/in.log");
    plugins.view_closed("rec", "v0");
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert!(!log.exists());
    assert!(plugins.start("rec").await.is_some());
    plugins.view_closed("rec", "v1");
    let read = || std::fs::read_to_string(&log).unwrap_or_default();
    assert!(until(|| read().contains("view.closed")).await);
    let line = read().lines().find(|l| l.contains("view.closed")).unwrap().to_owned();
    assert_eq!(line, r#"{"jsonrpc":"2.0","method":"event","params":{"type":"view.closed","viewId":"v1"}}"#);
    plugins.stop_all().await;
}
