use mx_core::model::TabKind;
use serde_json::json;

use super::Rig;

fn worktree_tab(rig: &mut Rig, label: Option<&str>) -> String {
    rig.s1();
    rig.add_tab("w", "/repo", TabKind::Claude, "/repo/.claude/worktrees/TASK-47730", label);
    rig.tm("w").branch = "feature/TASK#47730".into();
    "w".into()
}

#[test]
fn l1_a_label_wins_the_title() {
    let mut rig = Rig::new();
    let id = worktree_tab(&mut rig, Some("refatorar auth"));
    assert_eq!(rig.s.title(rig.t(&id)), "refatorar auth");
}

#[test]
fn l2_without_label_the_branch_task_names_the_tab() {
    let mut rig = Rig::new();
    let id = worktree_tab(&mut rig, None);
    assert_eq!(rig.s.title(rig.t(&id)), "TASK#47730");
}

#[test]
fn l3_a_subfolder_shell_is_named_after_the_folder() {
    let mut rig = Rig::new();
    rig.s1();
    rig.add_tab("s", "/repo", TabKind::Shell, "/repo/api", None);
    assert_eq!(rig.s.title(rig.t("s")), "api");
}

#[test]
fn l4_at_the_root_the_tab_takes_the_folder_name() {
    let mut rig = Rig::new();
    rig.s1();
    rig.add_tab("s", "/repo", TabKind::Shell, "/repo", None);
    rig.tm("s").branch = "master".into();
    assert_eq!(rig.s.title(rig.t("s")), "meu-repo");
}

#[test]
fn l5_layout_entry_of_a_resumable_claude_tab() {
    let mut rig = Rig::new();
    let id = worktree_tab(&mut rig, Some("TASK#47730"));
    rig.tm(&id).session_id = Some("abc-123".into());
    assert_eq!(
        rig.t(&id).to_json(),
        json!({"folderRoot":"/repo","kind":"claude","cwd":"/repo/.claude/worktrees/TASK-47730","label":"TASK#47730","sessionId":"abc-123"})
    );
}

#[test]
fn l6_a_shell_never_writes_a_session_id() {
    let mut rig = Rig::new();
    rig.s1();
    rig.shell("s");
    rig.tm("s").session_id = Some("x".into());
    assert!(rig.t("s").to_json().get("sessionId").is_none());
}

#[test]
fn l7_l8_sidebar_width_is_clamped() {
    let mut rig = Rig::new();
    rig.s.set_sidebar_width(2000.0);
    assert_eq!(rig.s.prefs().sidebar_width, 680.0);
    rig.s.set_sidebar_width(-40.0);
    assert_eq!(rig.s.prefs().sidebar_width, 248.0);
    rig.s.set_sidebar_width(420.0);
    assert_eq!(rig.s.prefs().sidebar_width, 420.0);
}

#[tokio::test]
async fn p5_add_folder_expands_the_home_tilde() {
    let mut rig = Rig::new();
    let made = rig.mkdir("projeto");
    rig.s.add_folder("~/projeto");
    rig.s.settle().await;
    let f = rig.s.find_folder(&made).expect("folder added");
    assert!(!f.root.contains('~'));
    assert!(f.root.starts_with(rig.dir.path().to_str().unwrap()));
    assert!(std::path::Path::new(&f.root).is_dir());
}

#[tokio::test]
async fn add_folder_that_does_not_exist_is_a_sticky_banner() {
    let mut rig = Rig::new();
    rig.s.add_folder("/nao/existe");
    rig.s.settle().await;
    assert!(rig.s.folders().is_empty());
    assert_eq!(rig.s.banner(), Some("Pasta não encontrada: /nao/existe"));
}
