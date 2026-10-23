use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, size, px};
use mx_claude::setup::{self, MxSetup, SetupSection};

use super::overlay::Overlay;
use super::tests_support::Scratch;
use super::{SetupActions, SetupView};

type Log = Rc<RefCell<Vec<(String, bool)>>>;

struct Pane {
    view: Entity<SetupView>,
    banners: Log,
    persisted: Rc<RefCell<Vec<MxSetup>>>,
}

fn open<'a>(cx: &'a mut TestAppContext, root: &str, selected: Option<&str>) -> (Pane, &'a mut VisualTestContext) {
    let banners: Log = Rc::default();
    let persisted: Rc<RefCell<Vec<MxSetup>>> = Rc::default();
    let (b, p) = (banners.clone(), persisted.clone());
    let setup = MxSetup { root: root.to_owned(), selected: selected.map(str::to_owned) };
    let (view, vcx) = cx.add_window_view(move |window, cx| {
        let actions = SetupActions {
            banner: Some(Rc::new(move |text, sticky, _, _| b.borrow_mut().push((text, sticky)))),
            changed: Some(Rc::new(move |s, _, _| p.borrow_mut().push(s))),
            ..Default::default()
        };
        SetupView::new(setup, mx_theme::default_theme().clone(), actions, window, cx)
    });
    vcx.simulate_resize(size(px(1100.), px(720.)));
    vcx.run_until_parked();
    (Pane { view, banners, persisted }, vcx)
}

fn click(cx: &mut VisualTestContext, selector: String) {
    let selector: &'static str = Box::leak(selector.into_boxed_str());
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let b = cx.debug_bounds(selector).unwrap_or_else(|| panic!("no element {selector}"));
    cx.simulate_click(b.center(), Modifiers::none());
    cx.run_until_parked();
}

fn text(p: &Pane, cx: &mut VisualTestContext) -> String {
    p.view.read_with(cx, |v, cx| v.text(cx))
}

fn focus_editor(p: &Pane, cx: &mut VisualTestContext) {
    let editor = p.view.read_with(cx, |v, _| v.editor().clone());
    cx.update(|window, cx| editor.read(cx).focus(window));
    cx.run_until_parked();
}

fn overlay_is<F: Fn(&Overlay) -> bool>(p: &Pane, cx: &mut VisualTestContext, f: F) -> bool {
    p.view.read_with(cx, |v, _| v.overlay.as_ref().is_some_and(&f))
}

#[gpui::test]
fn selecting_a_missing_slot_shows_its_template_and_saving_creates_it(cx: &mut TestAppContext) {
    let dir = Scratch::new();
    let (p, cx) = open(cx, &dir.root(), None);
    click(cx, "slot:.mcp.json".into());
    assert_eq!(text(&p, cx), "{\n  \"mcpServers\": {}\n}\n");
    assert_eq!(p.persisted.borrow().last().unwrap().selected.as_deref(), Some(".mcp.json"));
    click(cx, "setup:Salvar  ctrl+S".into());
    assert_eq!(std::fs::read_to_string(dir.0.join(".mcp.json")).unwrap(), "{\n  \"mcpServers\": {}\n}\n");
    assert_eq!(p.banners.borrow().last().unwrap(), &(".mcp.json salvo".to_owned(), false));
    assert!(p.view.read_with(cx, |v, _| v.slots.iter().any(|s| s.rel == ".mcp.json" && s.exists)));
}

#[gpui::test]
fn typing_then_ctrl_s_saves_what_was_typed(cx: &mut TestAppContext) {
    let dir = Scratch::new();
    setup::write(&dir.root(), "CLAUDE.md", "x").unwrap();
    let (p, cx) = open(cx, &dir.root(), Some("CLAUDE.md"));
    focus_editor(&p, cx);
    cx.simulate_keystrokes("ctrl-end");
    cx.simulate_input("yz");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("ação");
    assert_eq!(text(&p, cx), "xyz\nação");
    cx.simulate_keystrokes("ctrl-s");
    assert_eq!(std::fs::read_to_string(dir.0.join("CLAUDE.md")).unwrap(), "xyz\nação");
    assert_eq!(p.banners.borrow().last().unwrap().0, "CLAUDE.md salvo");
}

#[gpui::test]
fn leaving_an_edited_file_asks_and_voltar_keeps_the_text(cx: &mut TestAppContext) {
    let dir = Scratch::new();
    setup::write(&dir.root(), "CLAUDE.md", "a").unwrap();
    let (p, cx) = open(cx, &dir.root(), Some("CLAUDE.md"));
    focus_editor(&p, cx);
    cx.simulate_input("b");
    click(cx, "slot:.mcp.json".into());
    assert!(overlay_is(&p, cx, |o| matches!(o, Overlay::Discard(label, _) if label == "CLAUDE.md")));
    click(cx, "button:Voltar".into());
    assert_eq!(text(&p, cx), "ba");
    assert_eq!(p.view.read_with(cx, |v, _| v.setup().selected.clone()).as_deref(), Some("CLAUDE.md"));
    click(cx, "slot:.mcp.json".into());
    click(cx, "button:Descartar".into());
    assert_eq!(p.view.read_with(cx, |v, _| v.setup().selected.clone()).as_deref(), Some(".mcp.json"));
    assert_eq!(std::fs::read_to_string(dir.0.join("CLAUDE.md")).unwrap(), "a");
}

#[gpui::test]
fn opening_a_template_without_typing_does_not_ask_to_discard(cx: &mut TestAppContext) {
    let dir = Scratch::new();
    let (p, cx) = open(cx, &dir.root(), Some("CLAUDE.md"));
    click(cx, "slot:.mcp.json".into());
    assert!(p.view.read_with(cx, |v, _| v.overlay.is_none()));
}

#[gpui::test]
fn deleting_a_skill_takes_its_folder_and_clears_the_selection(cx: &mut TestAppContext) {
    let dir = Scratch::new();
    let root = dir.root();
    setup::write(&root, ".claude/skills/release/SKILL.md", "x").unwrap();
    setup::write(&root, ".claude/skills/release/notes.txt", "y").unwrap();
    let (p, cx) = open(cx, &root, Some(".claude/skills/release/SKILL.md"));
    click(cx, "setup:Apagar o arquivo".into());
    assert!(overlay_is(&p, cx, |o| matches!(o, Overlay::Delete(s) if s.label == "release")));
    click(cx, "button:Apagar".into());
    assert!(!dir.0.join(".claude/skills/release").exists());
    assert_eq!(p.view.read_with(cx, |v, _| v.setup().selected.clone()), None);
}

#[gpui::test]
fn deleting_a_fixed_file_keeps_it_selected_with_its_template_back(cx: &mut TestAppContext) {
    let dir = Scratch::new();
    setup::write(&dir.root(), "CLAUDE.md", "mine").unwrap();
    let (p, cx) = open(cx, &dir.root(), Some("CLAUDE.md"));
    click(cx, "setup:Apagar o arquivo".into());
    click(cx, "button:Apagar".into());
    assert!(!Path::new(&setup::path_of(&dir.root(), "CLAUDE.md")).exists());
    assert_eq!(p.view.read_with(cx, |v, _| v.setup().selected.clone()).as_deref(), Some("CLAUDE.md"));
    assert!(text(&p, cx).starts_with("# mx-setup-"));
}

#[gpui::test]
fn adding_a_rule_writes_its_template_and_selects_it(cx: &mut TestAppContext) {
    let dir = Scratch::new();
    let (p, cx) = open(cx, &dir.root(), None);
    click(cx, "first:Regras".into());
    assert!(overlay_is(&p, cx, |o| matches!(o, Overlay::Prompt(_))));
    cx.simulate_input("Testes");
    cx.simulate_keystrokes("enter");
    let rel = ".claude/rules/testes.md";
    assert_eq!(std::fs::read_to_string(dir.0.join(rel)).unwrap(), "---\npaths:\n  - \"**/*\"\n---\n\n# Testes\n\n- \n");
    assert_eq!(p.view.read_with(cx, |v, _| v.setup().selected.clone()).as_deref(), Some(rel));
    assert!(p.view.read_with(cx, |v, _| v.overlay.is_none()));
}

#[gpui::test]
fn adding_a_name_with_nothing_left_or_an_existing_file_only_says_so(cx: &mut TestAppContext) {
    let dir = Scratch::new();
    setup::write(&dir.root(), ".claude/agents/revisor.md", "x").unwrap();
    let (p, cx) = open(cx, &dir.root(), None);
    p.view.update_in(cx, |v, w, cx| v.named(SetupSection::Rules, "***".into(), w, cx));
    assert_eq!(p.banners.borrow().last().unwrap().0, "Esse nome não sobra nada depois de virar arquivo");
    p.view.update_in(cx, |v, w, cx| v.named(SetupSection::Agents, "Revisor".into(), w, cx));
    assert_eq!(p.banners.borrow().last().unwrap().0, "Já existe: .claude/agents/revisor.md");
    assert_eq!(p.view.read_with(cx, |v, _| v.setup().selected.clone()).as_deref(), Some(".claude/agents/revisor.md"));
    assert_eq!(text(&p, cx), "x");
}

#[gpui::test]
fn adding_a_decision_numbers_it_after_the_last_adr(cx: &mut TestAppContext) {
    let dir = Scratch::new();
    setup::write(&dir.root(), "docs/adr/0007-x.md", "x").unwrap();
    let (p, cx) = open(cx, &dir.root(), None);
    p.view.update_in(cx, |v, w, cx| v.named(SetupSection::Decisions, "PTY próprio".into(), w, cx));
    let rel = "docs/adr/0008-pty-proprio.md";
    assert!(text(&p, cx).starts_with("# ADR 0008: PTY próprio\n"));
    assert!(dir.0.join(rel).exists());
}

#[gpui::test]
fn a_disk_change_reloads_an_untouched_file_and_flags_an_edited_one(cx: &mut TestAppContext) {
    let dir = Scratch::new();
    let root = dir.root();
    setup::write(&root, "CLAUDE.md", "one").unwrap();
    let (p, cx) = open(cx, &root, Some("CLAUDE.md"));
    let tick = |cx: &mut VisualTestContext, now: &str| {
        setup::write(&root, "CLAUDE.md", now).unwrap();
        let slots = setup::scan(&root);
        let now = Some(("CLAUDE.md".to_owned(), now.to_owned()));
        p.view.update(cx, |v, cx| v.tick(slots, now, cx));
    };
    tick(cx, "two");
    assert_eq!(text(&p, cx), "two");
    focus_editor(&p, cx);
    cx.simulate_input("x");
    tick(cx, "three");
    assert_eq!(text(&p, cx), "xtwo");
    assert!(p.view.read_with(cx, |v, _| v.doc.outside));
    click(cx, "setup:Voltar ao que está no disco".into());
    click(cx, "button:Descartar".into());
    assert_eq!(text(&p, cx), "three");
}

#[gpui::test]
fn the_question_mark_opens_the_section_help_and_entendi_closes_it(cx: &mut TestAppContext) {
    let dir = Scratch::new();
    let (p, cx) = open(cx, &dir.root(), None);
    click(cx, "help:Skills".into());
    assert!(overlay_is(&p, cx, |o| matches!(o, Overlay::Help(_))));
    click(cx, "button:Entendi".into());
    assert!(p.view.read_with(cx, |v, _| v.overlay.is_none()));
}

#[gpui::test]
fn the_title_names_the_folder(cx: &mut TestAppContext) {
    let dir = Scratch::new();
    let (p, cx) = open(cx, &dir.root(), None);
    let title = p.view.read_with(cx, |v, _| v.title());
    assert!(title.starts_with("Claude · mx-setup-"));
}
