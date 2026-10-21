use mx_core::model::{ClaudeStatus, DocSource, PlanNote, TabKind};

use super::Rig;

fn session() -> Rig {
    let mut rig = Rig::new();
    rig.s1();
    rig.add_tab("s", "/repo", TabKind::Claude, "/repo", Some("TASK#47730"));
    rig.s.look("s");
    rig
}

fn plan(rig: &mut Rig, text: &str) {
    let at = rig.now();
    rig.tm("s").hooks.plans.push(PlanNote { text: text.into(), at });
}

fn readers(rig: &Rig) -> Vec<String> {
    rig.s.tabs().iter().filter(|t| t.is_reader()).map(|t| t.id.clone()).collect()
}

#[test]
fn show_plan_opens_a_reader_beside_the_session() {
    let mut rig = session();
    plan(&mut rig, "## Trocar o middleware");
    let r = rig.s.show_plan("s", None).unwrap();
    assert_eq!(rig.s.order(), ["s".to_owned(), r.clone()]);
    assert_eq!(rig.s.focused_pane_id(), Some(r.as_str()));
    assert_eq!(rig.t(&r).doc.as_ref().unwrap().title, "Plano de TASK#47730");
}

#[test]
fn show_message_reuses_the_reader() {
    let mut rig = session();
    plan(&mut rig, "# p");
    rig.tm("s").hooks.last_message_full = Some("feito".into());
    let a = rig.s.show_plan("s", None).unwrap();
    let b = rig.s.show_message("s").unwrap();
    assert_eq!(a, b);
    assert_eq!(rig.s.pane_count(), 2);
    let doc = rig.t(&b).doc.clone().unwrap();
    assert_eq!((doc.source, doc.title.as_str()), (DocSource::Message, "Recado de TASK#47730"));
}

#[test]
fn an_off_screen_reader_comes_back() {
    let mut rig = session();
    plan(&mut rig, "# p");
    let r = rig.s.show_plan("s", None).unwrap();
    rig.s.dismiss(&r);
    rig.s.show_plan("s", None);
    assert_eq!(readers(&rig), std::slice::from_ref(&r));
    assert!(rig.s.is_open(&r));
}

#[test]
fn no_plan_is_a_banner() {
    let mut rig = session();
    assert!(rig.s.show_plan("s", None).is_none());
    assert_eq!(rig.s.pane_count(), 1);
    assert!(rig.s.banner().unwrap().contains("não apresentou nenhum plano"));
}

#[test]
fn older_plans_are_numbered() {
    let mut rig = session();
    plan(&mut rig, "# primeiro");
    plan(&mut rig, "# segundo");
    let r = rig.s.show_plan("s", Some(0)).unwrap();
    assert_eq!(rig.t(&r).doc.as_ref().unwrap().title, "Plano 1/2 de TASK#47730");
    assert_eq!(rig.t(&r).status(), ClaudeStatus::Unknown);
    assert!(!rig.t(&r).status().needs_human());
}

#[test]
fn show_setup_is_one_tab_per_root() {
    let mut rig = session();
    let a = rig.s.show_setup("/repo", None, None).unwrap();
    let b = rig.s.show_setup("/repo", None, None).unwrap();
    assert_eq!(a, b);
    assert_eq!(rig.s.title(rig.t(&a)), "Claude · repo");
    assert_eq!(rig.s.pane_count(), 2);
    assert_eq!(rig.s.focused_pane_id(), Some(a.as_str()));
    let c = rig.s.show_setup("/repo", Some("/repo/api"), None).unwrap();
    assert_ne!(a, c);
}

#[test]
fn links_open_externally_read_markdown_or_quick_look() {
    let mut rig = session();
    let dir = rig.mkdir("docs");
    std::fs::write(format!("{dir}/a.md"), "# a").unwrap();
    std::fs::write(format!("{dir}/b.png"), "x").unwrap();
    rig.s.follow_link("https://exemplo.com", Some("s"), None);
    assert_eq!(*rig.notifier.links.lock().unwrap(), ["https://exemplo.com"]);
    rig.s.follow_link("nada.md", Some("s"), Some(&dir));
    assert!(rig.s.banner().unwrap().starts_with("Esse link aponta pra um arquivo que não existe"));
    rig.s.follow_link("a.md#topo", Some("s"), Some(&dir));
    assert_eq!(readers(&rig).len(), 1);
    rig.s.follow_link("b.png", Some("s"), Some(&dir));
    assert_eq!(*rig.notifier.looks.lock().unwrap(), [format!("{dir}/b.png")]);
}

#[tokio::test]
async fn open_markdown_shows_the_chosen_file() {
    let mut rig = session();
    let dir = rig.mkdir("docs");
    let file = format!("{dir}/n.md");
    std::fs::write(&file, "# n").unwrap();
    *rig.notifier.chosen.lock().unwrap() = Some(file.clone());
    rig.s.open_markdown(crate::DocPlace::default());
    rig.s.settle().await;
    let r = readers(&rig);
    assert_eq!(rig.t(&r[0]).doc.as_ref().unwrap().path.as_deref(), Some(file.as_str()));
}
