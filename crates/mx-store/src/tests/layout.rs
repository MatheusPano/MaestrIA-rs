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
        json!({"folderRoot":"/repo","kind":"claude","cwd":"/repo/.claude/worktrees/TASK-47730","label":"TASK#47730","sessionId":"abc-123","tabId":"w"})
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
    assert!(!f.root.starts_with('~'));
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

fn repo(rig: &Rig, name: &str) -> String {
    let p = rig.mkdir(name);
    std::fs::create_dir_all(std::path::Path::new(&p).join(".git")).unwrap();
    p
}

#[tokio::test]
async fn a_folder_holding_repos_becomes_a_workspace_with_it_and_them() {
    let mut rig = Rig::new();
    let back = repo(&rig, "uplii/uplii_backend");
    let front = repo(&rig, "uplii/uplii_frontend");
    rig.mkdir("uplii/docs");
    repo(&rig, "uplii/.cache");
    let uplii = rig.mkdir("uplii");
    rig.s.add_folder(&format!("{uplii}/"));
    rig.s.settle().await;
    let roots: Vec<&str> = rig.s.folders().iter().map(|f| f.root.as_str()).collect();
    assert_eq!(roots, [uplii.as_str(), back.as_str(), front.as_str()]);
    assert_eq!(rig.s.workspaces().len(), 1);
    let w = &rig.s.workspaces()[0];
    assert_eq!((w.name.as_str(), &w.folder_roots), ("uplii", &vec![back, front]));
    assert_eq!((w.root.as_deref(), w.code_workspace_path.as_deref()), (Some(uplii.as_str()), None));
    let ws = w.id.clone();
    assert_eq!(rig.s.root_folder_of(&ws).map(|f| f.root.as_str()), Some(uplii.as_str()));
    assert_eq!(rig.s.sidebar_rows(), [mx_core::model::sidebar::RowKey::Workspace(ws)]);
    assert_eq!(rig.s.banner(), Some("Workspace \"uplii\": 2 pastas adicionadas"));
}

#[tokio::test]
async fn adding_the_same_folder_of_repos_again_reuses_its_workspace() {
    let mut rig = Rig::new();
    repo(&rig, "uplii/api");
    let uplii = rig.mkdir("uplii");
    rig.s.add_folder(&uplii);
    rig.s.settle().await;
    repo(&rig, "uplii/web");
    rig.s.add_folder(&uplii);
    rig.s.settle().await;
    assert_eq!(rig.s.workspaces().len(), 1);
    assert_eq!(rig.s.workspaces()[0].folder_roots.len(), 2);
    assert_eq!(rig.s.banner(), Some("Workspace \"uplii\": 1 pasta adicionada, 1 já estava aqui"));
}

#[tokio::test]
async fn a_plain_folder_without_repos_inside_is_added_as_is() {
    let mut rig = Rig::new();
    let docs = rig.mkdir("uplii_backend/docs");
    let made = rig.mkdir("uplii_backend");
    rig.s.add_folder(&made);
    rig.s.settle().await;
    assert!(rig.s.find_folder(&made).is_some());
    assert!(rig.s.find_folder(&docs).is_none());
    assert!(rig.s.workspaces().is_empty());
}

#[tokio::test]
async fn a_folder_that_is_a_repo_is_not_scanned() {
    let mut rig = Rig::new();
    let made = repo(&rig, "uplii_backend");
    repo(&rig, "uplii_backend/vendor/lib");
    rig.runner.respond("git worktree list", mx_sys::output(0, format!("worktree {made}\n"), ""));
    rig.s.add_folder(&made);
    rig.s.settle().await;
    let roots: Vec<&str> = rig.s.folders().iter().map(|f| f.root.as_str()).collect();
    assert_eq!(roots, [made.as_str()]);
    assert!(rig.s.workspaces().is_empty());
}

async fn uplii(rig: &mut Rig) -> (String, String, String) {
    let api = repo(rig, "uplii/api");
    let parent = rig.mkdir("uplii");
    rig.s.add_folder(&parent);
    rig.s.settle().await;
    let ws = rig.s.workspaces()[0].id.clone();
    (parent, api, ws)
}

#[tokio::test]
async fn closing_a_folder_workspace_takes_its_parent_folder_too() {
    let mut rig = Rig::new();
    let (parent, _, ws) = uplii(&mut rig).await;
    assert_eq!(rig.s.closing_with(&ws).len(), 2);
    rig.s.close_workspace(&ws);
    assert!(rig.s.find_folder(&parent).is_none());
    assert!(rig.s.folders().is_empty());
}

#[tokio::test]
async fn dissolving_a_folder_workspace_frees_the_parent_first() {
    let mut rig = Rig::new();
    let (parent, api, ws) = uplii(&mut rig).await;
    rig.s.dissolve_workspace(&ws);
    use mx_core::model::sidebar::RowKey;
    assert_eq!(rig.s.sidebar_rows(), [RowKey::Folder(parent), RowKey::Folder(api)]);
}

#[tokio::test]
async fn removing_the_parent_folder_keeps_a_plain_workspace() {
    let mut rig = Rig::new();
    let (parent, api, ws) = uplii(&mut rig).await;
    rig.s.remove_folder(&parent);
    let w = rig.s.workspace_by_id(&ws).unwrap();
    assert_eq!((w.root.as_deref(), &w.folder_roots), (None, &vec![api]));
}

#[tokio::test]
async fn hiding_the_folders_of_a_folder_workspace_is_saved_and_a_reveal_undoes_it() {
    let mut rig = Rig::new();
    let (_, api, ws) = uplii(&mut rig).await;
    rig.s.toggle_workspace_folders(&ws);
    assert!(rig.s.workspace_by_id(&ws).unwrap().hide_folders);
    rig.add_tab("t", &api, mx_core::model::TabKind::Shell, &api, None);
    rig.s.reveal_tab("t");
    assert!(!rig.s.workspace_by_id(&ws).unwrap().hide_folders);
}

#[tokio::test]
async fn a_plain_workspace_has_no_folders_to_hide() {
    let mut rig = Rig::new();
    let ws = rig.s.create_workspace("cefis", &[], None).unwrap();
    rig.s.toggle_workspace_folders(&ws);
    assert!(!rig.s.workspace_by_id(&ws).unwrap().hide_folders);
}
