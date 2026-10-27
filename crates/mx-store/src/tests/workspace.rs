use mx_core::model::sidebar::RowKey;
use mx_core::model::{TabKind, Tint};

use super::{Rig, fixtures, names};

fn rows(rig: &Rig) -> Vec<String> {
    rig.s
        .sidebar_rows()
        .into_iter()
        .map(|r| match r {
            RowKey::Workspace(id) => format!("ws:{}", rig.s.workspace_by_id(&id).unwrap().name),
            RowKey::Folder(root) => rig.s.find_folder(&root).unwrap().name.clone(),
        })
        .collect()
}

fn write_ws(rig: &Rig, file: &str, body: &str) -> String {
    let p = rig.dir.path().join(file);
    std::fs::write(&p, body).unwrap();
    p.to_str().unwrap().to_owned()
}

async fn import(rig: &mut Rig, path: &str) {
    rig.s.import_workspace(path);
    rig.s.settle().await;
}

fn cefis(rig: &mut Rig) -> (String, String) {
    let api = rig.mkdir("api");
    let web = rig.mkdir("web");
    (api, web)
}

#[tokio::test]
async fn w14_import_adds_every_folder_into_one_workspace() {
    let mut rig = Rig::new();
    let (api, web) = cefis(&mut rig);
    let file = write_ws(&rig, "cefis.code-workspace", r#"{"folders":[{"path":"api"},{"path":"web"}]}"#);
    import(&mut rig, &file).await;
    let r = rig.s.last_import().unwrap();
    assert_eq!(r.added.len(), 2);
    let roots: Vec<&str> = rig.s.folders().iter().map(|f| f.root.as_str()).collect();
    assert_eq!(roots, [api.as_str(), web.as_str()]);
    assert_eq!(rig.s.workspaces().len(), 1);
    let w = &rig.s.workspaces()[0];
    assert_eq!(w.folder_roots, [api, web]);
    assert_eq!(w.code_workspace_path.as_deref(), Some(file.as_str()));
    assert_eq!(w.name, "cefis");
    assert_eq!(rig.s.banner(), Some("Workspace \"cefis\": 2 pastas adicionadas"));
}

#[tokio::test]
async fn w15_import_takes_the_entry_name() {
    let mut rig = Rig::new();
    cefis(&mut rig);
    let file = write_ws(&rig, "cefis.code-workspace", r#"{"folders":[{"path":"api","name":"API do cefis"}]}"#);
    import(&mut rig, &file).await;
    assert_eq!(rig.s.folders()[0].name, "API do cefis");
}

#[tokio::test]
async fn w16_import_of_a_folder_listed_twice_counts_it_once() {
    let mut rig = Rig::new();
    cefis(&mut rig);
    let file = write_ws(&rig, "cefis.code-workspace", r#"{"folders":[{"path":"api"},{"path":"api"}]}"#);
    import(&mut rig, &file).await;
    let r = rig.s.last_import().unwrap();
    assert_eq!((rig.s.folders().len(), r.added.len(), r.already.len()), (1, 1, 1));
    assert_eq!(rig.s.banner(), Some("Workspace \"cefis\": 1 pasta adicionada, 1 já estava aqui"));
}

#[tokio::test]
async fn w17_import_reports_missing_folders_and_sticks() {
    let mut rig = Rig::new();
    cefis(&mut rig);
    let file = write_ws(&rig, "cefis.code-workspace", r#"{"folders":[{"path":"api"},{"path":"sumiu"}]}"#);
    import(&mut rig, &file).await;
    let r = rig.s.last_import().unwrap();
    assert_eq!(r.added.len(), 1);
    assert!(r.missing[0].ends_with("/sumiu"));
    assert!(r.sticky());
    assert!(!rig.s.timers.is_set(&crate::TimerKey::Banner));
    assert_eq!(rig.s.banner(), Some("Workspace \"cefis\": 1 pasta adicionada, 1 não existe no disco"));
}

#[tokio::test]
async fn w18_importing_twice_adds_nothing_the_second_time() {
    let mut rig = Rig::new();
    cefis(&mut rig);
    let file = write_ws(&rig, "cefis.code-workspace", r#"{"folders":[{"path":"api"},{"path":"web"}]}"#);
    import(&mut rig, &file).await;
    import(&mut rig, &file).await;
    let r = rig.s.last_import().unwrap();
    assert_eq!(rig.s.folders().len(), 2);
    assert!(r.added.is_empty());
    assert_eq!(r.already.len(), 2);
    assert_eq!(rig.s.workspaces().len(), 1);
    assert_eq!(rig.s.banner(), Some("Workspace \"cefis\": 2 já estavam aqui"));
}

#[tokio::test]
async fn w19_two_workspace_files_share_one_folder() {
    let mut rig = Rig::new();
    cefis(&mut rig);
    let a = write_ws(&rig, "cefis.code-workspace", r#"{"folders":[{"path":"api"}]}"#);
    let b = write_ws(&rig, "outro.code-workspace", r#"{"folders":[{"path":"api"}]}"#);
    import(&mut rig, &a).await;
    import(&mut rig, &b).await;
    assert_eq!(rig.s.folders().len(), 1);
    assert_eq!(rig.s.workspaces().len(), 2);
    let root = rig.s.folders()[0].root.clone();
    assert_eq!(rig.s.workspaces_of(&root).len(), 2);
}

#[tokio::test]
async fn w20_unreadable_workspace_file_is_a_sticky_banner() {
    let mut rig = Rig::new();
    import(&mut rig, "/nao/existe.code-workspace").await;
    assert!(rig.s.last_import().is_none());
    assert!(rig.s.banner().unwrap().starts_with("Não consegui ler esse workspace"));
    assert!(rig.s.folders().is_empty());
}

#[tokio::test]
async fn w21_workspace_without_folders_adds_nothing() {
    let mut rig = Rig::new();
    let file = write_ws(&rig, "vazio.code-workspace", r#"{"folders": []}"#);
    import(&mut rig, &file).await;
    assert!(rig.s.last_import().is_none());
    assert_eq!(rig.s.banner(), Some("O workspace \"vazio\" não lista nenhuma pasta"));
    assert!(rig.s.folders().is_empty());
}

#[tokio::test]
async fn w24_only_missing_folders_create_no_workspace() {
    let mut rig = Rig::new();
    let file = write_ws(&rig, "cefis.code-workspace", r#"{"folders":[{"path":"sumiu"}]}"#);
    import(&mut rig, &file).await;
    assert!(rig.s.workspaces().is_empty());
}

fn w27() -> (Rig, String) {
    let mut rig = Rig::new();
    rig.folder("/repos/api", "api");
    rig.folder("/repos/solta", "solta");
    rig.folder("/repos/web", "web");
    let id = rig.s.create_workspace("cefis", &["/repos/api", "/repos/web"], None).unwrap();
    rig.s.root_order.clear();
    (rig, id)
}

#[test]
fn w27_sidebar_rows_put_the_workspace_where_its_first_folder_was() {
    let (rig, id) = w27();
    assert_eq!(rows(&rig), ["ws:cefis", "solta"]);
    assert_eq!(names(&rig.s.folders_of(&id)), ["api", "web"]);
}

#[test]
fn w29_removing_every_folder_keeps_the_empty_workspace() {
    let (mut rig, id) = w27();
    rig.s.remove_folder("/repos/api");
    rig.s.remove_folder("/repos/web");
    assert!(rig.s.workspace_by_id(&id).unwrap().folder_roots.is_empty());
    assert!(rows(&rig).contains(&"ws:cefis".to_owned()));
}

#[test]
fn w30_closing_a_workspace_removes_its_own_folders() {
    let (mut rig, id) = w27();
    rig.s.close_workspace(&id);
    assert!(rig.s.workspaces().is_empty());
    assert_eq!(rig.s.folders().iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["solta"]);
    assert_eq!(rig.s.banner(), Some("Workspace \"cefis\" fechado — 2 pastas saíram da lateral"));
}

#[test]
fn w31_closing_a_workspace_closes_its_panels() {
    let (mut rig, id) = w27();
    rig.add_tab("tab1", "/repos/api", TabKind::Shell, "/repos/api", None);
    rig.add_tab("tab2", "/repos/solta", TabKind::Shell, "/repos/solta", None);
    rig.s.close_workspace(&id);
    assert_eq!(rig.ids(), ["tab2"]);
}

fn legacy() -> Rig {
    let mut rig = Rig::new();
    let state = rig.s.opts.state_dir.clone();
    std::fs::create_dir_all(&state).unwrap();
    std::fs::copy(fixtures().join("config-2.4.0.json"), state.join("config.json")).unwrap();
    rig.s.load();
    rig
}

#[tokio::test]
async fn w33_to_w36_the_240_fixture_migrates_into_workspaces() {
    let rig = legacy();
    let atrium = rig.s.workspaces.iter().find(|w| w.name == "atrium").unwrap();
    assert_eq!(atrium.code_workspace_path.as_deref(), Some("/repos/atrium.code-workspace"));
    assert_eq!(atrium.folder_roots, ["/repos/atrium-backend", "/repos/atrium-frontend"]);
    assert!(atrium.collapsed);
    let uplii = rig.s.workspaces.iter().find(|w| w.name == "uplii").unwrap();
    assert_eq!(uplii.code_workspace_path.as_deref(), Some("/repos/uplii.code-workspace"));
    assert_eq!(uplii.folder_roots, ["/repos/uplii-ai"]);
    assert_eq!(rows(&rig), ["ws:atrium", "solta", "ws:uplii"]);
    assert_eq!(rig.s.features().len(), 1);
    assert_eq!(rig.s.features()[0].folder_root, "/repos/atrium-backend");
    assert!(rig.s.is_folder_collapsed("/repos/atrium-frontend", Some(&atrium.id)));
}

fn w37() -> Rig {
    let mut rig = Rig::new();
    rig.folder("/repos/atrium-api", "atrium-api");
    rig.folder("/repos/atrium-web", "atrium-web");
    rig.folder("/repos/infra", "infra");
    rig
}

#[test]
fn w37_create_workspace_trims_and_groups_folders() {
    let mut rig = w37();
    let id = rig.s.create_workspace(" ATRIUM ", &["/repos/atrium-api", "/repos/atrium-web"], None).unwrap();
    let w = rig.s.workspace_by_id(&id).unwrap();
    assert_eq!(w.name, "ATRIUM");
    assert!(w.code_workspace_path.is_none());
    assert_eq!(names(&rig.s.folders_of(&id)), ["atrium-api", "atrium-web"]);
    assert!(rig.s.stands_alone("/repos/infra"));
    assert!(!rig.s.stands_alone("/repos/atrium-api"));
}

#[test]
fn w38_a_folder_can_mirror_into_a_second_workspace() {
    let mut rig = w37();
    let a = rig.s.create_workspace("ATRIUM", &["/repos/infra"], None).unwrap();
    let u = rig.s.create_workspace("UPLII", &[], None).unwrap();
    rig.s.add_to_workspace("/repos/infra", &u, None);
    let of: Vec<&str> = rig.s.workspaces_of("/repos/infra").iter().map(|w| w.id.as_str()).collect();
    assert_eq!(of, [a.as_str(), u.as_str()]);
    assert!(rig.s.workspace_by_id(&u).unwrap().has("/repos/infra"));
}

#[test]
fn w39_adding_twice_is_a_no_op() {
    let mut rig = w37();
    let a = rig.s.create_workspace("ATRIUM", &[], None).unwrap();
    rig.s.add_to_workspace("/repos/infra", &a, None);
    rig.s.add_to_workspace("/repos/infra", &a, None);
    assert_eq!(rig.s.workspace_by_id(&a).unwrap().folder_roots, ["/repos/infra"]);
}

#[test]
fn w40_add_before_a_member_inserts_there() {
    let mut rig = w37();
    let a = rig.s.create_workspace("ATRIUM", &["/repos/atrium-api", "/repos/atrium-web"], None).unwrap();
    rig.s.add_to_workspace("/repos/infra", &a, Some("/repos/atrium-web"));
    assert_eq!(names(&rig.s.folders_of(&a)), ["atrium-api", "infra", "atrium-web"]);
}

#[test]
fn w41_removing_from_a_workspace_lands_at_the_given_row() {
    let mut rig = w37();
    let a = rig.s.create_workspace("ATRIUM", &["/repos/atrium-api"], None).unwrap();
    rig.s.remove_from_workspace("/repos/atrium-api", &a, Some(&RowKey::Folder("/repos/infra".into())));
    assert!(rig.s.folders_of(&a).is_empty());
    assert_eq!(rows(&rig), ["atrium-web", "atrium-api", "infra", "ws:ATRIUM"]);
}

#[test]
fn w42_a_mirrored_folder_does_not_stand_alone_after_one_removal() {
    let mut rig = w37();
    let a = rig.s.create_workspace("ATRIUM", &["/repos/infra"], None).unwrap();
    rig.s.create_workspace("UPLII", &["/repos/infra"], None).unwrap();
    rig.s.remove_from_workspace("/repos/infra", &a, None);
    assert!(!rig.s.stands_alone("/repos/infra"));
    let alone: Vec<String> = rows(&rig).into_iter().filter(|r| !r.starts_with("ws:")).collect();
    assert_eq!(alone, ["atrium-api", "atrium-web"]);
}

#[test]
fn w43_dissolving_frees_the_folders_in_place() {
    let mut rig = w37();
    let a = rig.s.create_workspace("ATRIUM", &["/repos/atrium-api", "/repos/atrium-web"], None).unwrap();
    rig.add_tab("t", "/repos/atrium-api", TabKind::Shell, "/repos/atrium-api", None);
    rig.s.dissolve_workspace(&a);
    assert!(rig.s.workspaces().is_empty());
    assert_eq!(rig.s.tabs().len(), 1);
    assert_eq!(rows(&rig), ["infra", "atrium-api", "atrium-web"]);
}

#[test]
fn w44_closing_keeps_folders_that_live_elsewhere() {
    let mut rig = w37();
    let a = rig.s.create_workspace("ATRIUM", &["/repos/atrium-api", "/repos/infra"], None).unwrap();
    let u = rig.s.create_workspace("UPLII", &["/repos/infra"], None).unwrap();
    assert_eq!(names(&rig.s.closing_with(&a)), ["atrium-api"]);
    rig.s.close_workspace(&a);
    let left: Vec<&str> = rig.s.folders().iter().map(|f| f.name.as_str()).collect();
    assert_eq!(left, ["atrium-web", "infra"]);
    assert_eq!(rig.s.workspaces().len(), 1);
    assert!(rig.s.workspace_by_id(&u).unwrap().has("/repos/infra"));
}

#[test]
fn w45_removing_a_folder_leaves_every_workspace() {
    let mut rig = w37();
    let a = rig.s.create_workspace("ATRIUM", &["/repos/infra"], None).unwrap();
    let u = rig.s.create_workspace("UPLII", &["/repos/infra"], None).unwrap();
    rig.s.remove_folder("/repos/infra");
    assert!(rig.s.workspace_by_id(&a).unwrap().folder_roots.is_empty());
    assert!(rig.s.workspace_by_id(&u).unwrap().folder_roots.is_empty());
}

#[test]
fn w46_a_re_added_folder_goes_to_the_end() {
    let mut rig = w37();
    rig.s.create_workspace("W", &[], None).unwrap();
    rig.s.remove_folder("/repos/atrium-api");
    rig.folder("/repos/atrium-api", "atrium-api");
    assert_eq!(rows(&rig), ["atrium-web", "infra", "ws:W", "atrium-api"]);
}

#[test]
fn w47_fold_state_is_per_appearance() {
    let mut rig = w37();
    let a = rig.s.create_workspace("ATRIUM", &["/repos/infra"], None).unwrap();
    let u = rig.s.create_workspace("UPLII", &["/repos/infra"], None).unwrap();
    rig.s.toggle_folder_collapsed("/repos/infra", Some(&a));
    assert!(rig.s.is_folder_collapsed("/repos/infra", Some(&a)));
    assert!(!rig.s.is_folder_collapsed("/repos/infra", Some(&u)));
    assert!(!rig.s.find_folder("/repos/infra").unwrap().collapsed);
}

#[test]
fn w48_blank_workspace_name_is_refused() {
    let mut rig = w37();
    assert!(rig.s.create_workspace("   ", &[], None).is_err());
    assert!(rig.s.workspaces().is_empty());
}

#[test]
fn w49_rename_tint_and_link() {
    let mut rig = w37();
    let a = rig.s.create_workspace("atrium", &[], None).unwrap();
    rig.s.rename_workspace(&a, "  ATRIUM ");
    rig.s.set_workspace_tint(&a, Some(Tint::Red));
    rig.s.link_code_workspace(&a, Some("/repos/atrium.code-workspace"));
    let w = rig.s.workspace_by_id(&a).unwrap();
    assert_eq!((w.name.as_str(), w.tint), ("ATRIUM", Some(Tint::Red)));
    assert_eq!(w.code_workspace_path.as_deref(), Some("/repos/atrium.code-workspace"));
    rig.s.link_code_workspace(&a, None);
    assert!(rig.s.workspace_by_id(&a).unwrap().code_workspace_path.is_none());
}

#[test]
fn w50_panel_colour_falls_back_to_the_first_workspace() {
    let mut rig = w37();
    let a = rig.s.create_workspace("ATRIUM", &["/repos/infra"], None).unwrap();
    let u = rig.s.create_workspace("UPLII", &["/repos/infra"], None).unwrap();
    rig.s.set_workspace_tint(&a, Some(Tint::Red));
    rig.s.set_workspace_tint(&u, Some(Tint::Cyan));
    rig.add_tab("t", "/repos/infra", TabKind::Shell, "/repos/infra", None);
    assert_eq!(rig.s.chosen_tint_of("t"), Some(Tint::Red));
    rig.s.set_folder_tint("/repos/infra", Some(Tint::Green));
    assert_eq!(rig.s.chosen_tint_of("t"), Some(Tint::Green));
}

#[test]
fn w51_create_workspace_is_saved_with_the_derived_root_order() {
    let mut rig = w37();
    let a = rig.s.create_workspace("ATRIUM", &["/repos/atrium-api"], None).unwrap();
    let j = rig.written();
    let wss = j["workspaces"].as_array().unwrap();
    assert_eq!(wss.len(), 1);
    assert_eq!(wss[0]["name"], "ATRIUM");
    assert_eq!(wss[0]["folderRoots"], serde_json::json!(["/repos/atrium-api"]));
    assert_eq!(
        j["rootOrder"],
        serde_json::json!(["folder:/repos/atrium-web", "folder:/repos/infra", format!("workspace:{a}")])
    );
    assert!(j["folders"].as_array().unwrap().iter().all(|f| f.get("workspace").is_none()));
}
