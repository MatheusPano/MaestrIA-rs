use mx_core::model::{AgentInfo, WorktreeInfo};
use mx_sys::history::{ChatEntry, ChatStanding};

use super::Rig;

const TITLED: &str = "aaaa1111-1111-4111-8111-000000000001";

async fn chat(rig: &Rig, id: &str) -> ChatEntry {
    rig.s.all_chats().await.into_iter().find(|c| c.session_id.starts_with(id)).unwrap()
}

#[tokio::test]
async fn y7_all_chats_and_folder_chats() {
    let mut rig = Rig::new();
    rig.folder("/tmp", "tmp");
    assert_eq!(rig.s.all_chats().await.len(), 4);
    assert_eq!(rig.s.chats_in("/tmp").await.len(), 3);
    let loose = rig.s.loose().root.clone();
    assert_eq!(rig.s.chats_in(&loose).await.len(), 4);
}

#[test]
fn y8_folder_at_matches_roots_and_worktrees() {
    let mut rig = Rig::new();
    rig.folder("/tmp", "tmp");
    let wt = |path: &str, main: bool| WorktreeInfo { path: path.into(), branch: "b".into(), is_main: main, prunable: false };
    rig.s.worktrees.insert("/tmp".into(), vec![wt("/tmp", true), wt("/var/wt/TASK-1", false)]);
    for hit in ["/tmp", "/tmp/lib/ui", "/var/wt/TASK-1"] {
        assert_eq!(rig.s.folder_at(hit).map(|f| f.root.as_str()), Some("/tmp"), "{hit}");
    }
    for miss in ["/var/wt/TASK-1000", "/tmpfs/outro", ""] {
        assert!(rig.s.folder_at(miss).is_none(), "{miss}");
    }
}

#[tokio::test]
async fn y12_a_fresh_chat_resumes_into_the_folder() {
    let mut rig = Rig::new();
    rig.folder("/tmp", "tmp");
    let c = chat(&rig, "aaaa").await;
    assert_eq!(rig.s.standing_of(&c), ChatStanding::Fresh);
    let id = rig.s.resume_chat(&c, Some("/tmp"), None).unwrap();
    let t = rig.t(&id);
    assert_eq!((t.folder_root.as_str(), t.cwd.as_str()), ("/tmp", "/tmp"));
    assert_eq!(t.custom_label.as_deref(), Some("Cabeçalho da lateral com a busca"));
    assert_eq!(t.resume_id(), Some(TITLED));
    assert_eq!(rig.s.title(t), "Cabeçalho da lateral com a busca");
    assert!(rig.terms.log(&id).commands[0].contains(&format!("--resume '{TITLED}'")));
}

#[tokio::test]
async fn y13_without_folder_the_chat_cwd_finds_it() {
    let mut rig = Rig::new();
    rig.folder("/tmp", "tmp");
    let c = chat(&rig, "aaaa").await;
    let id = rig.s.resume_chat(&c, None, None).unwrap();
    assert_eq!(rig.t(&id).folder_root, "/tmp");
}

#[tokio::test]
async fn y14_without_folders_it_lands_in_the_tray() {
    let mut rig = Rig::new();
    let c = chat(&rig, "aaaa").await;
    let id = rig.s.resume_chat(&c, None, None).unwrap();
    assert_eq!(rig.t(&id).folder_root, rig.s.loose().root);
    assert_eq!(rig.t(&id).cwd, "/tmp");
}

#[tokio::test]
async fn y15_a_chat_on_screen_is_selected_not_reopened() {
    let mut rig = Rig::new();
    rig.folder("/tmp", "tmp");
    let c = chat(&rig, "aaaa").await;
    let id = rig.s.resume_chat(&c, None, None).unwrap();
    rig.s.set_panes(None, None);
    assert_eq!(rig.s.standing_of(&c), ChatStanding::OnScreen);
    assert_eq!(rig.s.resume_chat(&c, None, None), Some(id));
    assert_eq!(rig.s.tabs().len(), 1);
    assert!(rig.s.panes().is_some());
}

#[tokio::test]
async fn y16_a_chat_running_elsewhere_is_refused() {
    let mut rig = Rig::new();
    let c = chat(&rig, "aaaa").await;
    let row = AgentInfo::from_json(&serde_json::json!({"pid": 4242, "sessionId": TITLED})).unwrap();
    rig.s.apply_agents(vec![row]);
    assert_eq!(rig.s.standing_of(&c), ChatStanding::Live);
    assert!(rig.s.resume_chat(&c, None, None).is_none());
    assert!(rig.s.tabs().is_empty());
    assert!(rig.s.banner().unwrap().contains("ainda está rodando fora do MaestrIA"));
}

#[tokio::test]
async fn y17_an_orphan_chat_names_the_missing_folder() {
    let mut rig = Rig::new();
    let c = chat(&rig, "dddd").await;
    assert_eq!(rig.s.standing_of(&c), ChatStanding::Gone);
    assert!(rig.s.resume_chat(&c, None, None).is_none());
    assert!(rig.s.banner().unwrap().contains("/repo/sumiu"));
}

fn entry(id: &str, cwd: &str) -> ChatEntry {
    ChatEntry {
        session_id: id.into(),
        cwd: cwd.into(),
        title: format!("conversa {id}"),
        at: std::time::SystemTime::now(),
        bytes: 10,
        missing: false,
    }
}

fn agent(v: serde_json::Value) -> AgentInfo {
    AgentInfo::from_json(&v).unwrap()
}

#[tokio::test]
async fn a_background_chat_attaches_in_a_shell_and_is_on_screen_until_it_exits() {
    let mut rig = Rig::new();
    rig.folder("/tmp", "tmp");
    let c = chat(&rig, "aaaa").await;
    rig.s.apply_agents(vec![agent(serde_json::json!({
        "pid": null, "id": "40e816ba", "cwd": "/tmp", "kind": "background",
        "sessionId": TITLED, "name": "revisão do schema"
    }))]);
    assert_eq!(rig.s.standing_of(&c), ChatStanding::Background);
    assert!(ChatStanding::Background.note().unwrap().contains("segundo plano"));
    let id = rig.s.resume_chat(&c, None, None).unwrap();
    assert_eq!(rig.terms.log(&id).commands, vec!["claude attach '40e816ba'".to_owned()]);
    let t = rig.t(&id);
    assert_eq!((t.folder_root.as_str(), t.cwd.as_str(), t.kind), ("/tmp", "/tmp", mx_core::model::TabKind::Shell));
    assert_eq!(rig.s.title(t), c.label());
    assert_eq!(t.agent_name.as_deref(), Some("revisão do schema"));
    assert_eq!(rig.s.standing_of(&c), ChatStanding::OnScreen);
    assert_eq!(rig.s.resume_chat(&c, None, None), Some(id.clone()));
    assert_eq!(rig.s.tabs().len(), 1);
    rig.tm(&id).exited = true;
    assert_eq!(rig.s.standing_of(&c), ChatStanding::Background);
}

#[tokio::test]
async fn a_background_row_without_a_job_id_is_live() {
    let mut rig = Rig::new();
    let c = chat(&rig, "aaaa").await;
    rig.s.apply_agents(vec![agent(serde_json::json!({"cwd": "/tmp", "kind": "background", "sessionId": TITLED}))]);
    assert_eq!(rig.s.standing_of(&c), ChatStanding::Live);
}

#[test]
fn a_tab_that_moved_to_a_new_session_holds_the_new_one_and_frees_the_old() {
    let mut rig = Rig::new();
    rig.folder("/tmp", "tmp");
    rig.add_tab("c", "/tmp", mx_core::model::TabKind::Claude, "/tmp", None);
    rig.tm("c").session_id = Some("old".into());
    rig.terms.edit("c", |l| l.pid = Some(4242));
    rig.hook("c", "SessionStart", serde_json::json!({"session_id": "now"}));
    rig.s.agents = vec![agent(serde_json::json!({"pid": 4242, "kind": "interactive", "sessionId": "old"}))];
    let (now, old) = (entry("now", "/tmp"), entry("old", "/tmp"));
    assert_eq!(rig.s.standing_of(&now), ChatStanding::OnScreen);
    assert_eq!(rig.s.resume_chat(&now, None, None).as_deref(), Some("c"));
    rig.tm("c").session_id = Some("now".into());
    assert_eq!(rig.s.standing_of(&old), ChatStanding::Fresh);
}
