use std::sync::Arc;

use mx_core::model::{ClaudeStatus, Folder, Tab, TabKind};
use mx_store::testing::{FakeBus, FakeClock, FakeHooks, FakeNotifier, FakeTerms};
use mx_store::{Ports, Store, StoreOptions};
use mx_sys::FakeRunner;

use super::*;

fn tab(id: &str, kind: TabKind) -> Tab {
    Tab::new(id, &Folder::new("/repo", "repo"), kind, "/repo", chrono::Local::now())
}

fn claude(id: &str, status: ClaudeStatus) -> Tab {
    let mut t = tab(id, TabKind::Claude);
    t.hooks.status = status;
    t
}

#[test]
fn sections_come_waiting_then_unseen_then_working_then_resting() {
    let mut asks = claude("pede permissão", ClaudeStatus::WaitingPermission);
    asks.hooks.active_tool = Some("Bash".into());
    let mut finished = claude("terminou", ClaudeStatus::Idle);
    finished.rested_at = Some(chrono::Local::now());
    finished.unseen = true;
    let shell = tab("um shell", TabKind::Shell);
    let busy = claude("pensando", ClaudeStatus::Working);
    let tabs = vec![&shell, &busy, &finished, &asks];
    let got: Vec<(&str, Vec<&str>)> =
        sections(&tabs).into_iter().map(|(b, list)| (b.label(), list.iter().map(|t| t.id.as_str()).collect())).collect();
    assert_eq!(
        got,
        vec![
            ("Esperando você", vec!["pede permissão"]),
            ("Terminaram enquanto você não via", vec!["terminou"]),
            ("Trabalhando", vec!["pensando"]),
            ("Em repouso", vec!["um shell"]),
        ]
    );
}

#[test]
fn a_done_session_waiting_for_you_rests_instead() {
    let mut t = claude("x", ClaudeStatus::WaitingInput);
    t.done = true;
    assert_eq!(classify(&t), Bucket::Rest);
}

#[test]
fn empty_sections_are_left_out() {
    let shell = tab("s", TabKind::Shell);
    let got: Vec<Bucket> = sections(&[&shell]).into_iter().map(|(b, _)| b).collect();
    assert_eq!(got, vec![Bucket::Rest]);
}

#[test]
fn columns_fit_as_many_280_px_cards_as_the_width_allows() {
    assert_eq!(columns(280., GAP, CARD_MIN), (1, 280.));
    assert_eq!(columns(100., GAP, CARD_MIN), (1, 100.));
    let (cols, w) = columns(1000., GAP, CARD_MIN);
    assert_eq!(cols, 3);
    assert!((w - (1000. - 24.) / 3.).abs() < 1e-3);
}

#[test]
fn where_drops_the_folder_the_title_already_says_and_a_detached_head() {
    assert_eq!(where_line("TASK#1", "api", false, "feature/TASK#1"), "api · feature/TASK#1");
    assert_eq!(where_line("api", "api", false, "main"), "main");
    assert_eq!(where_line("x", "~", true, "(detached)"), "");
}

#[test]
fn a_plain_shell_card_shows_its_folder_with_home_as_tilde() {
    let mut t = tab("s", TabKind::Shell);
    t.cwd = "/home/u/proj".into();
    assert_eq!(body(&t, "Shell", Some("/home/u")), Some(("~/proj".into(), true)));
}

#[test]
fn a_claude_that_only_says_its_state_shows_the_last_prompt() {
    let mut t = claude("c", ClaudeStatus::Ready);
    assert_eq!(body(&t, "Pronto", None), None);
    t.hooks.last_prompt = Some("arruma o teste".into());
    assert_eq!(body(&t, "Pronto", None), Some(("arruma o teste".into(), false)));
    let asks = claude("p", ClaudeStatus::WaitingPermission);
    assert_eq!(body(&asks, "Quer aprovação pra Bash", None), Some(("Quer aprovação pra Bash".into(), false)));
}

#[test]
fn the_footer_names_what_the_panel_is_when_it_is_not_claude() {
    assert_eq!(state(&tab("s", TabKind::Shell)).0, "Terminal");
    let mut program = tab("p", TabKind::Shell);
    program.launcher_id = Some("btop".into());
    assert_eq!(state(&program).0, "Programa");
    assert_eq!(state(&tab("r", TabKind::Reader)).0, "Documento");
    assert_eq!(state(&tab("c", TabKind::Setup)).0, "Configuração");
    assert_eq!(state(&claude("c", ClaudeStatus::Working)), ("Pensando", StateTone::Status(ClaudeStatus::Working)));
}

#[test]
fn the_apps_screen_invites_to_install_when_the_rail_is_empty() {
    assert_eq!(apps_subtitle(0), "Instale um plugin, ou abra uma sessão pela lateral");
    assert_eq!(apps_subtitle(2), "Escolha um plugin, ou abra uma sessão pela lateral");
}

#[test]
fn tapping_the_tile_of_the_plugin_already_shown_never_hides_the_sidebar() {
    assert!(tile_opens(false, None, "git"));
    assert!(!tile_opens(false, Some("git"), "git"));
    assert!(tile_opens(true, Some("git"), "git"));
}

struct Rig {
    s: Store,
    _rt: tokio::runtime::Runtime,
    dir: std::path::PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn rig(name: &str) -> Rig {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let dir = std::env::temp_dir().join(format!("mx-cards-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ports = Ports {
        clock: Arc::new(FakeClock::new(chrono::Local::now())),
        terms: Arc::new(FakeTerms::default()),
        runner: Arc::new(FakeRunner::new()),
        notifier: Arc::new(FakeNotifier::default()),
        plugins: Arc::new(FakeBus::default()),
        hooks: Arc::new(FakeHooks { port: 4242 }),
        rt: rt.handle().clone(),
    };
    let opts = StoreOptions::at(dir.join("state"), dir.join("chats"), dir.to_str());
    Rig { s: Store::new(opts, ports), _rt: rt, dir }
}

#[test]
fn given_no_tabs_there_are_no_cards() {
    let r = rig("none");
    assert!(card_tabs(&r.s).is_empty());
}

#[test]
fn tapping_a_shell_card_puts_it_back_on_screen() {
    let mut r = rig("tap");
    let id = r.s.open_shell(&Folder::new("/repo", "repo"), None, None, None, None);
    r.s.dismiss(&id);
    assert!(!r.s.is_open(&id));
    assert_eq!(card_tabs(&r.s).len(), 1);
    open_card(&mut r.s, &id);
    assert!(r.s.is_open(&id));
}
