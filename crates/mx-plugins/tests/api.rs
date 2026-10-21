mod common;

use std::path::PathBuf;
use std::sync::Arc;

use common::host::FakeHost;
use common::obj;
use mx_core::model::{ClaudeStatus, FeatureOrHotfixKind, TabKind};
use mx_plugins::host_json::{FeatureJson, SessionActivity, SessionJson, WorkspaceJson};
use mx_plugins::{HostDispatcher, PluginInfo, PluginManifest, PluginState, RpcError};
use serde_json::{Value, json};

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

fn plugin() -> PluginInfo {
    info(json!({"id": "p", "name": "Plugin P", "version": "1", "main": ["x"]}))
}

async fn call(host: FakeHost, p: &PluginInfo, method: &str, params: Value) -> (Result<Value, RpcError>, Arc<FakeHost>) {
    let host = Arc::new(host);
    let d = HostDispatcher::new(host.clone());
    (d.dispatch(p, method, &obj(params)).await, host)
}

async fn run(p: &PluginInfo, method: &str, params: Value) -> Result<Value, RpcError> {
    call(FakeHost::default(), p, method, params).await.0
}

#[tokio::test]
async fn window_pick_without_a_window_errors_and_drops_items_without_value() {
    let p = plugin();
    let d = HostDispatcher::new(Arc::new(FakeHost::default())).with_window_probe(Arc::new(|| false));
    let e = d.dispatch(&p, "window.pick", &obj(json!({"items": [{"value": "a"}]}))).await.unwrap_err();
    assert_eq!(e, RpcError::unavailable("a janela não tem onde perguntar"));
    let host = FakeHost { pick_last: true, ..FakeHost::default() };
    let params = json!({"title": "qual projeto?", "items": [
        {"value": "/a", "label": "a", "detail": "~/a"}, {"value": "/b"}, {"label": "sem valor"}]});
    let (out, host) = call(host, &p, "window.pick", params).await;
    assert_eq!(out.unwrap(), json!("/b"));
    assert_eq!(host.calls(), ["pick qual projeto? a|~/a,/b|null"]);
    assert_eq!(run(&p, "window.pick", json!({"items": []})).await.unwrap(), Value::Null);
}

#[tokio::test]
async fn window_input_defaults_the_title_to_the_plugin_name() {
    let p = info(json!({"id": "ssh", "name": "SSH", "version": "1"}));
    let host = FakeHost { input_answer: Some("AWS".into()), ..FakeHost::default() };
    let (out, host) = call(host, &p, "window.input", json!({"placeholder": "nome", "value": "a"})).await;
    assert_eq!(out.unwrap(), json!("AWS"));
    assert_eq!(
        host.calls(),
        [r#"input InputRequest { title: "SSH", placeholder: Some("nome"), value: Some("a"), prompt: None }"#]
    );
}

#[tokio::test]
async fn window_form_rejects_malformed_fields_and_answers_with_string_values() {
    let p = plugin();
    let e = run(&p, "window.form", json!({"fields": []})).await.unwrap_err();
    assert_eq!(e, RpcError::invalid("\"fields\" precisa de ao menos um campo"));
    let e = run(&p, "window.form", json!({"fields": [{"id": "a", "type": "data"}]})).await.unwrap_err();
    assert_eq!(e, RpcError::invalid("o campo \"a\" tem \"type\": \"data\" — vale \"text\" ou \"select\""));
    let out = run(&p, "window.form", json!({"fields": [{"id": "a"}]})).await.unwrap();
    assert_eq!(out.to_string(), r#"{"action":"ok","values":{"a":""}}"#);
}

#[tokio::test]
async fn banner_is_prefixed_with_the_plugin_name_and_open_url_is_web_only() {
    let p = plugin();
    let (_, host) = call(FakeHost::default(), &p, "window.showBanner", json!({"text": "oi", "sticky": true})).await;
    assert_eq!(host.calls(), ["banner Plugin P: oi true"]);
    assert_eq!(run(&p, "window.showBanner", json!({})).await.unwrap_err(), RpcError::invalid("\"text\" é obrigatório"));
    let e = run(&p, "window.openUrl", json!({"url": "file:///etc"})).await.unwrap_err();
    assert_eq!(e, RpcError::invalid("só dá pra abrir http(s)"));
    assert!(run(&p, "window.openUrl", json!({"url": "https://x"})).await.is_ok());
}

#[tokio::test]
async fn gated_methods_need_their_permission() {
    let p = plugin();
    for (method, perm) in [
        ("window.notify", "notifications"),
        ("chats.list", "hooks"),
        ("terminal.sendText", "terminal.write"),
        ("session.openClaude", "sessions.create"),
        ("session.openShell", "sessions.create"),
    ] {
        let e = run(&p, method, json!({"body": "b", "day": "2026-01-01", "text": "t"})).await.unwrap_err();
        assert_eq!(e, RpcError::forbidden(format!("o manifesto não declara a permissão \"{perm}\"")));
    }
    assert_eq!(e_code(run(&p, "nada.disso", json!({})).await), (-32601, "método desconhecido: nada.disso".into()));
}

fn e_code(r: Result<Value, RpcError>) -> (i64, String) {
    let e = r.unwrap_err();
    (e.code, e.message)
}

#[tokio::test]
async fn chats_list_needs_a_parseable_day() {
    let p = info(json!({"id": "p", "version": "1", "permissions": ["hooks"]}));
    let e = run(&p, "chats.list", json!({"day": "ontem"})).await.unwrap_err();
    assert_eq!(e, RpcError::invalid("\"day\" é AAAA-MM-DD"));
    let (out, host) = call(FakeHost::default(), &p, "chats.list", json!({"day": "2026-09-23"})).await;
    assert_eq!(out.unwrap(), json!([]));
    assert_eq!(host.calls(), ["chats 2026-09-23"]);
}

#[tokio::test]
async fn sidebar_update_needs_the_contribution_stringifies_the_badge_and_keeps_blocks_when_absent() {
    let q = info(json!({"id": "q", "version": "1", "main": ["x"]}));
    let e = run(&q, "sidebar.update", json!({})).await.unwrap_err();
    assert_eq!(e, RpcError::invalid("o manifesto não declara \"contributes.sidebar\""));
    let git = info(json!({"id": "git", "version": "1", "main": ["x"], "contributes": {"sidebar": true}}));
    let (out, host) = call(FakeHost::default(), &git, "sidebar.update", json!({"badge": 3, "blocks": [{}, 1]})).await;
    assert_eq!(out.unwrap().to_string(), r#"{"shown":false}"#);
    let (_, h2) = call(FakeHost::default(), &git, "sidebar.update", json!({"badge": null})).await;
    let (_, h3) = call(FakeHost::default(), &git, "sidebar.update", json!({})).await;
    assert_eq!(host.calls(), ["sidebar Set(\"3\") 1"]);
    assert_eq!(h2.calls(), ["sidebar Clear 0"]);
    assert_eq!(h3.calls(), ["sidebar Unchanged 0"]);
}

#[tokio::test]
async fn view_open_refuses_the_sidebar_id_and_rfw_must_be_an_object() {
    let p = plugin();
    let e = run(&p, "view.open", json!({"viewId": "sidebar"})).await.unwrap_err();
    assert_eq!(e, RpcError::invalid("\"sidebar\" é o id da aba da lateral — use sidebar.update"));
    let (out, host) = call(FakeHost::default(), &p, "view.open", json!({"viewId": "v", "modal": true})).await;
    assert_eq!(out.unwrap().to_string(), r#"{"tabId":"tab3"}"#);
    assert_eq!(host.calls(), ["view.open v Plugin P 0 true"]);
    let e = run(&p, "view.update", json!({"viewId": "x", "rfw": "texto solto"})).await.unwrap_err();
    assert_eq!(e, RpcError::invalid("\"rfw\" é { library, root? }"));
    let e = run(&p, "view.update", json!({"viewId": "x", "data": []})).await.unwrap_err();
    assert_eq!(e, RpcError::invalid("\"data\" é um objeto"));
    let e = run(&p, "view.update", json!({"viewId": "x", "rfw": {"library": 3}})).await.unwrap_err();
    assert_eq!(e.code, RpcError::INTERNAL);
    let out = run(&p, "view.update", json!({"viewId": "y"})).await.unwrap();
    assert_eq!(out.to_string(), r#"{"open":false}"#);
}

#[tokio::test]
async fn floats_validate_the_corner_and_the_id_and_show_always_answers_shown() {
    let p = plugin();
    let e = run(&p, "float.show", json!({"id": "x", "corner": "meio"})).await.unwrap_err();
    assert_eq!(e, RpcError::invalid("\"corner\" é topLeft, topRight, bottomLeft ou bottomRight"));
    assert_eq!(run(&p, "float.show", json!({})).await.unwrap_err(), RpcError::invalid("\"id\" é obrigatório"));
    let out = run(&p, "float.show", json!({"id": "t", "corner": "topLeft"})).await.unwrap();
    assert_eq!(out.to_string(), r#"{"shown":true}"#);
    assert_eq!(run(&p, "float.update", json!({"id": "t"})).await.unwrap().to_string(), r#"{"shown":false}"#);
    assert_eq!(run(&p, "float.hide", json!({"id": "t"})).await.unwrap(), Value::Null);
}

#[tokio::test]
async fn session_close_only_closes_terminals_the_plugin_owns() {
    let ssh = info(json!({"id": "ssh", "version": "1", "main": ["x"]}));
    let host = || FakeHost {
        owners: vec![("t1".into(), Some("ssh".into())), ("t2".into(), None)],
        ..FakeHost::default()
    };
    let (e, _) = call(host(), &ssh, "session.close", json!({"tabId": "t2"})).await;
    assert_eq!(e.unwrap_err(), RpcError::forbidden("só dá pra fechar um terminal que o plugin abriu com \"owned\""));
    let q = info(json!({"id": "q", "version": "1", "main": ["x"]}));
    let (e, _) = call(host(), &q, "session.close", json!({"tabId": "t1"})).await;
    assert_eq!(e.unwrap_err().code, RpcError::FORBIDDEN);
    let (ok, h) = call(host(), &ssh, "session.close", json!({"tabId": "t1"})).await;
    assert_eq!(ok.unwrap(), Value::Null);
    assert_eq!(h.calls(), ["close t1"]);
    let (e, _) = call(host(), &ssh, "session.focus", json!({"tabId": "zz"})).await;
    assert_eq!(e.unwrap_err(), RpcError::invalid("não há painel \"zz\""));
}

#[tokio::test]
async fn an_embedded_shell_is_owned_and_carries_its_tag() {
    let docker = info(json!({"id": "docker", "version": "1", "main": ["x"], "permissions": ["sessions.create"]}));
    let params = json!({"command": "docker exec -it db sh", "embedded": true, "tag": "e:db", "label": ""});
    let (out, host) = call(FakeHost::default(), &docker, "session.openShell", params).await;
    assert_eq!(out.unwrap().to_string(), r#"{"tabId":"tab8"}"#);
    assert_eq!(
        host.calls(),
        [r#"shell OpenShell { cwd: None, command: Some("docker exec -it db sh"), label: None, owned: true, tag: Some("e:db"), embedded: true }"#]
    );
}

fn session(id: &str, owner: Option<&str>, tag: Option<&str>) -> SessionJson {
    SessionJson {
        id: id.into(),
        kind: TabKind::Shell,
        title: "t".into(),
        cwd: "/repos/infra".into(),
        folder: "/repos/infra".into(),
        branch: "main".into(),
        status: ClaudeStatus::Unknown,
        done: false,
        session_id: None,
        hibernated: false,
        exited: false,
        on_screen: true,
        focused: false,
        started_at: "2026-09-23T10:00:00.000".into(),
        feature_or_hotfix: Some(("login quebrado".into(), FeatureOrHotfixKind::Hotfix)),
        workspaces: vec!["ATRIUM".into()],
        launcher: None,
        owner: owner.map(str::to_owned),
        owner_tag: tag.map(str::to_owned),
        embedded: false,
        activity: SessionActivity { prompts: 1, tools: 2, touched: vec![], last_prompt: None, last_message: None },
    }
}

#[tokio::test]
async fn the_owner_tag_is_private_and_activity_needs_hooks() {
    let host = || FakeHost { sessions: vec![session("t1", Some("ssh"), Some("s:h1"))], ..FakeHost::default() };
    let ssh = info(json!({"id": "ssh", "version": "1", "main": ["x"]}));
    let (out, _) = call(host(), &ssh, "sessions.list", json!({})).await;
    let first = out.unwrap()[0].clone();
    assert_eq!(first["owner"], json!("ssh"));
    assert_eq!(first["tag"], json!("s:h1"));
    assert!(first.get("activity").is_none());
    assert_eq!(first["project"], json!("login quebrado"));
    assert_eq!(first["featureOrHotfixKind"], json!("hotfix"));
    assert_eq!(first["workspaces"], json!(["ATRIUM"]));
    let q = info(json!({"id": "q", "version": "1", "main": ["x"], "permissions": ["hooks"]}));
    let (out, _) = call(host(), &q, "sessions.list", json!({})).await;
    let first = out.unwrap()[0].clone();
    assert!(first.get("tag").is_none());
    assert_eq!(first["activity"]["tools"], json!(2));
}

#[tokio::test]
async fn workspaces_omit_an_unset_code_workspace_and_projects_list_is_an_alias() {
    let host = || FakeHost {
        workspaces: vec![WorkspaceJson {
            id: "w1".into(),
            name: "ATRIUM".into(),
            folders: vec!["/repos/infra".into()],
            code_workspace_path: None,
        }],
        features: vec![FeatureJson {
            id: "f1".into(),
            name: "login quebrado".into(),
            kind: FeatureOrHotfixKind::Hotfix,
            folder: "/repos/infra".into(),
            brief: String::new(),
        }],
        ..FakeHost::default()
    };
    let p = plugin();
    let (w, _) = call(host(), &p, "workspaces.list", json!({})).await;
    assert_eq!(w.unwrap().to_string(), r#"[{"id":"w1","name":"ATRIUM","folders":["/repos/infra"]}]"#);
    let (f, _) = call(host(), &p, "featuresOrHotfixes.list", json!({})).await;
    let (pr, _) = call(host(), &p, "projects.list", json!({})).await;
    let f = f.unwrap();
    assert_eq!(f[0]["kind"], json!("hotfix"));
    assert_eq!(f, pr.unwrap());
}

#[tokio::test]
async fn suggestion_created_needs_the_contribution_and_the_ids() {
    let p = plugin();
    let e = run(&p, "suggestion.created", json!({})).await.unwrap_err();
    assert_eq!(e, RpcError::forbidden("só quem declara \"contributes.suggestions\" recebe tarefas sugeridas"));
    let t = info(json!({"id": "tracker", "version": "1", "main": ["x"], "contributes": {"suggestions": {"label": "l"}}}));
    let e = run(&t, "suggestion.created", json!({"suggestionId": "s", "ref": " "})).await.unwrap_err();
    assert_eq!(e, RpcError::invalid("faltam \"suggestionId\" e \"ref\""));
    let out = run(&t, "suggestion.created", json!({"suggestionId": "s", "ref": "TASK#1"})).await.unwrap();
    assert_eq!(out.to_string(), r#"{"tabId":"a","cwd":"/repo"}"#);
}

#[tokio::test]
async fn markdown_and_editor_and_send_text_pass_their_params() {
    let p = info(json!({"id": "p", "name": "Plugin P", "version": "1", "permissions": ["terminal.write"]}));
    let (out, h) = call(FakeHost::default(), &p, "pane.openMarkdown", json!({"markdown": "# oi"})).await;
    assert_eq!(out.unwrap().to_string(), r#"{"tabId":"tab9"}"#);
    assert_eq!(h.calls(), ["markdown Plugin P|# oi"]);
    let (_, h) = call(FakeHost::default(), &p, "editor.open", json!({"path": "/a", "line": 12.0})).await;
    assert_eq!(h.calls(), ["editor /a Some(12)"]);
    let (_, h) = call(FakeHost::default(), &p, "terminal.sendText", json!({"text": "ls", "submit": true})).await;
    assert_eq!(h.calls(), [r#"send SendText { text: "ls", submit: true, tab_id: None }"#]);
}
