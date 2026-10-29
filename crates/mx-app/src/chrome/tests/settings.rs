use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext, TestAppContext};

use super::ui::{click, host};
use crate::chrome::demo::settings_data;
use crate::chrome::settings::{ChordCheck, KeyPress, Section, SettingsAction, SettingsDialog};

fn validate(p: &KeyPress) -> ChordCheck {
    match p.key.as_str() {
        "shift" | "control" => ChordCheck::Ignore,
        "v" if p.ctrl && p.shift => ChordCheck::Rejected("o painel cola com Ctrl+Shift+V".into()),
        _ if !(p.ctrl || p.alt) => ChordCheck::Rejected("precisa de Ctrl ou Alt — sem modificador a tecla sumiria do terminal".into()),
        k => ChordCheck::Accepted { id: format!("ctrl+{k}"), label: format!("Ctrl+{}", k.to_uppercase()) },
    }
}

type Log = Rc<RefCell<Vec<SettingsAction>>>;

fn open(cx: &mut TestAppContext, section: Section) -> (gpui::Entity<SettingsDialog>, &mut gpui::VisualTestContext, Log) {
    let log: Log = Rc::default();
    let l2 = log.clone();
    let (d, vcx) = host(cx, move |_, cx| cx.new(|cx| SettingsDialog::new(settings_data(), section, Rc::new(validate), move |a, _, _| l2.borrow_mut().push(a), cx)));
    (d, vcx, log)
}

#[gpui::test]
fn appearance_shows_typography_and_more_steps_the_size_to_14(cx: &mut TestAppContext) {
    let (d, cx, log) = open(cx, Section::Appearance);
    cx.simulate_resize(gpui::size(gpui::px(1200.), gpui::px(820.)));
    super::ui::redraw(cx);
    d.read_with(cx, |d, _| d.scroll.set_offset(gpui::point(gpui::px(0.), gpui::px(-1000.))));
    super::ui::redraw(cx);
    for s in ["heading:Tipografia do pty", "value:13px", "value:1.20", "face:Hack"] {
        assert!(cx.debug_bounds(s).is_some(), "{s}");
    }
    click(cx, "step:corpo-more");
    let got = log.borrow().clone();
    let Some(SettingsAction::SetTypography(t)) = got.last() else { panic!("{got:?}") };
    assert_eq!(t.size(), 14.);
    assert_eq!(crate::chrome::settings::size_label(t), "14px");
}

#[gpui::test]
fn picking_a_theme_applies_it(cx: &mut TestAppContext) {
    let (_, cx, log) = open(cx, Section::Appearance);
    click(cx, "theme:Ayu Dark");
    assert_eq!(log.borrow().last(), Some(&SettingsAction::SetTheme("ayu-dark".into())));
}

#[gpui::test]
fn gallery_splits_dark_and_light_themes(cx: &mut TestAppContext) {
    let (_, cx, _) = open(cx, Section::Appearance);
    cx.simulate_resize(gpui::size(gpui::px(1200.), gpui::px(820.)));
    super::ui::redraw(cx);
    assert!(cx.debug_bounds("group:Escuros").is_some());
    assert!(cx.debug_bounds("group:Claros").is_some());
    assert!(cx.debug_bounds("group:De plugins").is_none());
}

#[gpui::test]
fn veil_is_light_over_appearance_and_darker_elsewhere(cx: &mut TestAppContext) {
    let (d, cx, _) = open(cx, Section::Appearance);
    assert_eq!(d.read_with(cx, |d, _| d.veil()), 0.2);
    click(cx, "section:Plugins");
    assert_eq!(d.read_with(cx, |d, _| d.veil()), 0.5);
}

#[gpui::test]
fn rail_switches_sections(cx: &mut TestAppContext) {
    let (d, cx, _) = open(cx, Section::Appearance);
    click(cx, "section:Plugins");
    assert_eq!(d.read_with(cx, |d, _| d.section()), Section::Plugins);
    assert!(cx.debug_bounds("heading:Plugins").is_some());
    click(cx, "section:Sessões");
    assert!(cx.debug_bounds("heading:Hibernação").is_some());
}

#[gpui::test]
fn recorder_ignores_modifiers_rejects_reserved_and_binds_valid_chords(cx: &mut TestAppContext) {
    let (d, cx, log) = open(cx, Section::Shortcuts);
    click(cx, "keycap:F2");
    assert!(d.read_with(cx, |d, _| d.recording().is_some()));
    cx.simulate_keystrokes("ctrl-shift-v");
    let why = d.read_with(cx, |d, _| d.recording().and_then(|r| r.rejection.clone()));
    assert_eq!(why.as_deref(), Some("o painel cola com Ctrl+Shift+V"));
    cx.simulate_keystrokes("escape");
    assert!(d.read_with(cx, |d, _| d.recording().is_none()));
    click(cx, "keycap:F2");
    cx.simulate_keystrokes("ctrl-k");
    assert_eq!(
        log.borrow().last(),
        Some(&SettingsAction::Bind { action: "rename".into(), chord: "ctrl+k".into(), replacing: Some("f2".into()) })
    );
    assert!(d.read_with(cx, |d, _| d.recording().is_none()));
}
