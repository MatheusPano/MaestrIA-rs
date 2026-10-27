use std::path::Path;
use std::sync::Arc;

use chrono::TimeDelta;
use mx_core::model::{AgentInfo, ClaudeStatus, ContextUsage, TabKind};
use mx_core::ports::TermKey;
use mx_plugins::{HostDispatcher, PluginInfo, PluginManifest, PluginState, RpcError};
use serde_json::{Value, json};

use super::Rig;
use crate::Effect;

fn plugin(id: &str, perms: &[&str], dir: &Path, data: &Path) -> PluginInfo {
    let m = json!({"id": id, "version": "1", "main": ["x"], "permissions": perms});
    let manifest = PluginManifest::parse(m.as_object().unwrap()).unwrap();
    PluginInfo {
        id: id.into(),
        dir: dir.into(),
        data_dir: data.into(),
        manifest: Some(Arc::new(manifest)),
        problem: None,
        linked: false,
        enabled: true,
        crash: None,
        state: PluginState::Idle,
        palettes: Vec::new(),
    }
}

fn with(perms: &[&str]) -> PluginInfo {
    plugin("q", perms, Path::new("/nao/existe"), Path::new("/nao/existe-dados"))
}

async fn rpc(rig: &mut Rig, p: &PluginInfo, method: &str, params: Value) -> Result<Value, RpcError> {
    let d = HostDispatcher::new(rig.s.plugin_host());
    let (p, method) = (p.clone(), method.to_owned());
    let params = params.as_object().unwrap().clone();
    let call = tokio::spawn(async move { d.dispatch(&p, &method, &params).await });
    while !call.is_finished() {
        rig.s.pump();
        tokio::task::yield_now().await;
    }
    call.await.unwrap()
}

fn claude(rig: &mut Rig, id: &str) -> String {
    rig.add_tab(id, "/repo", TabKind::Claude, "/repo", Some(id));
    rig.tm(id).session_id = Some(format!("sess-{id}"));
    id.to_owned()
}

fn events(rig: &Rig, name: &str) -> Vec<Value> {
    rig.bus.events.lock().unwrap().iter().filter(|(n, _)| n == name).map(|(_, d)| d.clone()).collect()
}

fn sleeping(rig: &mut Rig) -> String {
    rig.s1();
    let id = claude(rig, "c1");
    rig.hook(&id, "Stop", json!({}));
    rig.s.hibernate(&id);
    id
}

#[tokio::test]
async fn wake_needs_sessions_create_and_the_tab_id_and_wakes_once() {
    let mut rig = Rig::new();
    let id = sleeping(&mut rig);
    assert!(rpc(&mut rig, &with(&[]), "session.wake", json!({"tabId": id})).await.is_err());
    let create = with(&["sessions.create"]);
    assert!(rpc(&mut rig, &create, "session.wake", json!({})).await.is_err());
    assert!(rig.t(&id).hibernated);
    assert_eq!(rpc(&mut rig, &create, "session.wake", json!({"tabId": id})).await.unwrap(), json!({"woke": true}));
    assert!(!rig.t(&id).hibernated);
    assert_eq!(rig.t(&id).status(), ClaudeStatus::Starting);
    assert_eq!(rpc(&mut rig, &create, "session.wake", json!({"tabId": id})).await.unwrap(), json!({"woke": false}));
}

#[tokio::test]
async fn hibernate_needs_sessions_close_and_a_session_at_rest() {
    let mut rig = Rig::new();
    rig.s1();
    let id = claude(&mut rig, "c1");
    let create = with(&["sessions.create"]);
    assert!(rpc(&mut rig, &create, "session.hibernate", json!({"tabId": id})).await.is_err());
    let close = with(&["sessions.close"]);
    assert!(rpc(&mut rig, &close, "session.hibernate", json!({})).await.is_err());
    rig.hook(&id, "UserPromptSubmit", json!({}));
    let busy = rpc(&mut rig, &close, "session.hibernate", json!({"tabId": id})).await.unwrap_err();
    assert_eq!(busy.message, "\"c1\" está trabalhando: hiberne quando ele parar");
    assert!(!rig.t(&id).hibernated);
    rig.hook(&id, "Stop", json!({}));
    let out = rpc(&mut rig, &close, "session.hibernate", json!({"tabId": id})).await.unwrap();
    assert_eq!(out, json!({"hibernated": true}));
    assert!(rig.t(&id).hibernated);
    let again = rpc(&mut rig, &close, "session.hibernate", json!({"tabId": id})).await.unwrap();
    assert_eq!(again, json!({"hibernated": false}));
}

#[tokio::test]
async fn hibernate_explains_why_a_tab_cannot_sleep() {
    let mut rig = Rig::new();
    rig.s1();
    rig.shell("um shell");
    claude(&mut rig, "novo");
    rig.tm("novo").session_id = None;
    let close = with(&["sessions.close"]);
    let shell = rpc(&mut rig, &close, "session.hibernate", json!({"tabId": "um shell"})).await.unwrap_err();
    assert_eq!(shell.message, "\"um shell\" não é uma sessão do claude rodando");
    let fresh = rpc(&mut rig, &close, "session.hibernate", json!({"tabId": "novo"})).await.unwrap_err();
    assert_eq!(fresh.message, "\"novo\" ainda não tem conversa pra retomar");
}

#[tokio::test]
async fn nothing_is_typed_into_a_claude_asking_to_trust_the_folder() {
    let mut rig = Rig::new();
    rig.s1();
    let id = claude(&mut rig, "c1");
    rig.terms.edit(&id, |l| l.trust = true);
    let w = with(&["terminal.write"]);
    let text = rpc(&mut rig, &w, "terminal.sendText", json!({"tabId": id, "text": "oi", "submit": true})).await;
    assert!(text.unwrap_err().message.contains("confia na pasta"));
    let keys = rpc(&mut rig, &w, "terminal.sendKeys", json!({"tabId": id, "keys": ["enter"]})).await;
    assert_eq!(keys.unwrap_err().message, "\"c1\" está perguntando se você confia na pasta: responda no terminal dele");
    let paste = rpc(&mut rig, &w, "terminal.pasteClipboard", json!({"tabId": id})).await;
    assert!(paste.unwrap_err().message.contains("confia na pasta"));
    assert!(rig.terms.log(&id).submitted.is_empty());
    rig.terms.edit(&id, |l| l.trust = false);
    rpc(&mut rig, &w, "terminal.sendKeys", json!({"tabId": id, "keys": ["down", {"text": "sim"}, "enter"]}))
        .await
        .unwrap();
    assert_eq!(rig.terms.log(&id).keys, [TermKey::Down, TermKey::Text("sim".into()), TermKey::Enter]);
}

#[tokio::test]
async fn send_keys_refuses_dead_tabs_before_checking_the_list() {
    let mut rig = Rig::new();
    rig.s1();
    let id = claude(&mut rig, "c1");
    let w = with(&["terminal.write"]);
    let bad = rpc(&mut rig, &w, "terminal.sendKeys", json!({"tabId": id, "keys": ["pgup"]})).await.unwrap_err();
    assert!(bad.message.starts_with("`keys`: uma lista de up, down"));
    rig.tm(&id).hibernated = true;
    let dead = rpc(&mut rig, &w, "terminal.sendKeys", json!({"tabId": id, "keys": ["pgup"]})).await.unwrap_err();
    assert_eq!(dead.message, "\"c1\" não tem processo pra receber teclas");
    let paste = rpc(&mut rig, &w, "terminal.pasteClipboard", json!({"tabId": id})).await.unwrap_err();
    assert_eq!(paste.message, "\"c1\" não tem processo pra receber a colagem");
}

#[tokio::test]
async fn paste_clipboard_and_an_empty_submit_reach_a_claude_terminal() {
    let mut rig = Rig::new();
    rig.s1();
    let id = claude(&mut rig, "c1");
    rig.s.look(&id);
    let w = with(&["terminal.write"]);
    rpc(&mut rig, &w, "terminal.pasteClipboard", json!({})).await.unwrap();
    assert_eq!(rig.s.take_effects(), [Effect::PasteClipboard { tab_id: id.clone(), images_via_ctrl_v: true }]);
    rpc(&mut rig, &w, "terminal.sendText", json!({"text": "", "submit": true})).await.unwrap();
    assert_eq!(rig.terms.log(&id).enters, 1);
    assert!(rig.terms.log(&id).submitted.is_empty());
    let none = rpc(&mut rig, &with(&[]), "terminal.pasteClipboard", json!({})).await.unwrap_err();
    assert_eq!(none.code, RpcError::FORBIDDEN);
    rig.shell("sh");
    rpc(&mut rig, &w, "terminal.pasteClipboard", json!({"tabId": "sh"})).await.unwrap();
    assert_eq!(rig.s.take_effects(), [Effect::PasteClipboard { tab_id: "sh".into(), images_via_ctrl_v: false }]);
}

#[tokio::test]
async fn a_portrait_must_be_a_png_of_the_plugin_or_its_data() {
    let mut rig = Rig::new();
    rig.s1();
    let id = claude(&mut rig, "c1");
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("dados");
    std::fs::write(dir.path().join("rosto.png"), b"png").unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(elsewhere.path().join("outro.png"), b"png").unwrap();
    let p = plugin("escritorio", &[], dir.path(), &data);
    let rosto = dir.path().join("rosto.png");
    rpc(&mut rig, &p, "session.portrait", json!({"tabId": id, "image": rosto})).await.unwrap();
    let real = std::fs::canonicalize(&rosto).unwrap();
    assert_eq!(rig.s.portrait_of(&id), Some(real.to_str().unwrap()));
    let outro = elsewhere.path().join("outro.png");
    let foreign = rpc(&mut rig, &p, "session.portrait", json!({"tabId": id, "image": outro})).await.unwrap_err();
    assert_eq!(foreign, RpcError::forbidden("\"image\" tem que estar na pasta do plugin ou na de dados dele"));
    let nada = dir.path().join("nada.png");
    let missing = rpc(&mut rig, &p, "session.portrait", json!({"tabId": id, "image": nada})).await.unwrap_err();
    assert_eq!(missing, RpcError::invalid("\"image\": o caminho de um .png que existe"));
    rpc(&mut rig, &p, "session.portrait", json!({"tabId": id, "image": null})).await.unwrap();
    assert_eq!(rig.s.portrait_of(&id), None);
    rig.shell("sh");
    let shell = rpc(&mut rig, &p, "session.portrait", json!({"tabId": "sh", "image": null})).await.unwrap_err();
    assert_eq!(shell.message, "só uma sessão do claude tem retrato");
}

#[tokio::test]
async fn rename_trims_clips_and_empty_goes_back_to_the_default_name() {
    let mut rig = Rig::new();
    rig.s1();
    let id = claude(&mut rig, "c1");
    assert!(rpc(&mut rig, &with(&[]), "session.rename", json!({"tabId": id, "title": "x"})).await.is_err());
    let create = with(&["sessions.create"]);
    assert!(rpc(&mut rig, &create, "session.rename", json!({"title": "x"})).await.is_err());
    let out = rpc(&mut rig, &create, "session.rename", json!({"tabId": id, "title": "  login do google  "})).await;
    assert_eq!(out.unwrap(), json!({"title": "login do google"}));
    assert_eq!(rig.s.title(rig.t(&id)), "login do google");
    assert_eq!(events(&rig, "session.renamed"), [json!({"tabId": id, "title": "login do google"})]);
    let back = rpc(&mut rig, &create, "session.rename", json!({"tabId": id, "title": ""})).await.unwrap();
    assert_ne!(back["title"], json!("login do google"));
    let long = "x".repeat(100);
    let clipped = rpc(&mut rig, &create, "session.rename", json!({"tabId": id, "title": long})).await.unwrap();
    assert_eq!(clipped["title"].as_str().unwrap().len(), 80);
}

#[tokio::test]
async fn open_claude_in_background_registers_without_placing() {
    let mut rig = Rig::new();
    rig.s1();
    let create = with(&["sessions.create"]);
    let out = rpc(&mut rig, &create, "session.openClaude", json!({"cwd": "/repo", "background": true})).await.unwrap();
    let id = out["tabId"].as_str().unwrap().to_owned();
    assert!(rig.s.has_tab(&id));
    assert!(!rig.s.is_open(&id));
    assert_eq!(events(&rig, "session.opened").len(), 1);
    let out = rpc(&mut rig, &create, "session.openClaude", json!({"cwd": "/repo"})).await.unwrap();
    assert!(rig.s.is_open(out["tabId"].as_str().unwrap()));
}

#[tokio::test]
async fn session_close_takes_a_user_claude_only_with_sessions_close() {
    let mut rig = Rig::new();
    rig.s1();
    let c1 = claude(&mut rig, "c1");
    rig.shell("s1");
    let q = with(&[]);
    let escritorio = plugin("escritorio", &["sessions.close"], Path::new("/p"), Path::new("/d"));
    assert!(rpc(&mut rig, &q, "session.close", json!({"tabId": c1})).await.is_err());
    assert!(rpc(&mut rig, &escritorio, "session.close", json!({"tabId": "s1"})).await.is_err());
    assert!(rpc(&mut rig, &escritorio, "session.close", json!({})).await.is_err());
    assert!(rig.s.has_tab(&c1));
    rpc(&mut rig, &escritorio, "session.close", json!({"tabId": c1})).await.unwrap();
    assert!(!rig.s.has_tab(&c1));
    assert!(rig.s.has_tab("s1"));
}

#[test]
fn session_json_carries_the_context_usage() {
    let mut rig = Rig::new();
    rig.s1();
    claude(&mut rig, "c1");
    let j = rig.s.session_list()[0].to_json("q", false);
    assert_eq!(j["context"], Value::Null);
    rig.tm("c1").context = Some(ContextUsage { percent: 71, tokens: 142000, window: 200000 });
    let j = rig.s.session_list()[0].to_json("q", false);
    assert_eq!(j["context"], json!({"percent": 71, "tokens": 142000, "window": 200000}));
    let keys: Vec<&String> = j.as_object().unwrap().keys().collect();
    let at = |k: &str| keys.iter().position(|x| *x == k).unwrap();
    assert_eq!(at("context") + 1, at("owner"));
}

#[test]
fn a_changed_context_is_announced_once() {
    let mut rig = Rig::new();
    rig.s1();
    claude(&mut rig, "c1");
    let body = json!({"context_window": {"used_percentage": 58.4, "context_window_size": 200000,
        "current_usage": {"input_tokens": 10, "cache_creation_input_tokens": 1000, "cache_read_input_tokens": 115000}}});
    rig.s.apply_status("c1", &body);
    rig.s.apply_status("c1", &body);
    let seen = events(&rig, "session.context");
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0], json!({"tabId": "c1", "context": {"percent": 58, "tokens": 116010, "window": 200000}}));
    rig.s.apply_status("c1", &json!({"context_window": {"used_percentage": null}}));
    assert_eq!(events(&rig, "session.context")[1], json!({"tabId": "c1", "context": null}));
}

#[test]
fn compaction_hooks_leave_the_status_alone() {
    let mut rig = Rig::new();
    rig.s1();
    claude(&mut rig, "c1");
    rig.turn("c1");
    assert_eq!(rig.t("c1").status(), ClaudeStatus::Idle);
    let notices = rig.s.notices().len();
    rig.hook("c1", "PreCompact", json!({"trigger": "manual"}));
    assert_eq!(rig.t("c1").status(), ClaudeStatus::Idle);
    rig.hook("c1", "PostCompact", json!({"trigger": "manual"}));
    assert_eq!(rig.t("c1").status(), ClaudeStatus::Idle);
    assert_eq!(rig.s.notices().len(), notices);
    let hooks: Vec<String> = events(&rig, "hook").iter().map(|e| e["name"].as_str().unwrap().to_owned()).collect();
    assert!(hooks.ends_with(&["PreCompact".to_owned(), "PostCompact".to_owned()]));
}

#[test]
fn sleeping_and_waking_tell_the_plugins() {
    let mut rig = Rig::new();
    let id = sleeping(&mut rig);
    assert_eq!(
        events(&rig, "session.status").last().unwrap(),
        &json!({"tabId": id, "status": "ended", "previous": "idle", "hibernated": true})
    );
    rig.s.wake(&id);
    assert_eq!(
        events(&rig, "session.status").last().unwrap(),
        &json!({"tabId": id, "status": "starting", "previous": "ended", "hibernated": false})
    );
}

#[test]
fn the_launch_settles_to_ready_only_after_the_trust_prompt_is_answered() {
    let mut rig = Rig::new();
    let folder = rig.s1();
    let id = rig.s.open_claude(crate::ClaudeSpec::new(&folder, "/repo"));
    rig.terms.edit(&id, |l| l.trust = true);
    rig.advance(TimeDelta::seconds(3));
    assert_eq!(rig.t(&id).status(), ClaudeStatus::Starting);
    rig.advance(TimeDelta::seconds(2));
    rig.advance(TimeDelta::seconds(2));
    assert_eq!(rig.t(&id).status(), ClaudeStatus::Starting);
    rig.terms.edit(&id, |l| l.trust = false);
    rig.advance(TimeDelta::seconds(2));
    assert_eq!(rig.t(&id).status(), ClaudeStatus::Ready);
    assert_eq!(
        events(&rig, "session.status").last().unwrap(),
        &json!({"tabId": id, "status": "ready", "previous": "starting"})
    );
}

#[test]
fn the_trust_wait_gives_up_after_three_hundred_retries() {
    let mut rig = Rig::new();
    let folder = rig.s1();
    let id = rig.s.open_claude(crate::ClaudeSpec::new(&folder, "/repo"));
    rig.terms.edit(&id, |l| l.trust = true);
    rig.advance(TimeDelta::seconds(3));
    for _ in 0..300 {
        rig.advance(TimeDelta::seconds(2));
    }
    rig.terms.edit(&id, |l| l.trust = false);
    rig.advance(TimeDelta::seconds(2));
    assert_eq!(rig.t(&id).status(), ClaudeStatus::Starting);
}

#[test]
fn a_waiting_session_with_a_portrait_notifies_with_its_name_and_question() {
    let mut rig = Rig::new();
    rig.s1();
    let id = claude(&mut rig, "login");
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("rosto.png");
    std::fs::write(&png, b"png").unwrap();
    let real = std::fs::canonicalize(&png).unwrap().to_string_lossy().into_owned();
    let p = plugin("escritorio", &[], dir.path(), dir.path());
    rig.s.plugin_portrait(&p, id.clone(), Some(json!(png))).unwrap();
    rig.hook(&id, "UserPromptSubmit", json!({}));
    rig.hook(
        &id,
        "PreToolUse",
        json!({"tool_name": "AskUserQuestion", "tool_input": {"questions": [{"question": "Qual banco?"}]}}),
    );
    let sent = rig.notifier.portraits.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "login");
    assert!(sent[0].1.starts_with("pergunta: "), "{}", sent[0].1);
    assert_eq!(sent[0].2, real);
    assert!(rig.notifier.alerts.lock().unwrap().is_empty());
}

#[test]
fn a_hook_reported_session_id_survives_the_agents_poll() {
    let mut rig = Rig::new();
    rig.s1();
    claude(&mut rig, "c1");
    rig.tm("c1").session_id = None;
    let pid = rig.terms.give_pid("c1");
    let agent = |sid: &str| AgentInfo {
        id: None,
        pid: Some(i64::from(pid)),
        cwd: "/repo".into(),
        kind: "interactive".into(),
        session_id: Some(sid.into()),
        name: None,
        status: None,
        waiting_for: None,
        state: None,
        started_at: None,
    };
    rig.s.apply_agents(vec![agent("velho")]);
    assert_eq!(rig.t("c1").resume_id(), Some("velho"));
    rig.hook("c1", "SessionStart", json!({"session_id": "novo"}));
    rig.s.apply_agents(vec![agent("velho")]);
    assert_eq!(rig.t("c1").resume_id(), Some("novo"));
}
