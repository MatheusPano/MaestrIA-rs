use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyView, AppContext, Context, Entity, InteractiveElement, IntoElement, Modifiers, ParentElement,
    Render, StatefulInteractiveElement, Styled, TestAppContext, VisualTestContext, Window, div,
    point, px, size,
};

use crate::chrome::anim::ConfettiLayer;
use crate::chrome::dialog::form::{FormDialog, FormResult};
use crate::chrome::dialog::specs;
use crate::chrome::flow::{FlowEditor, FlowResult, Trigger};
use crate::chrome::menu::{ContextMenu, MenuAction, builders, model::find};
use mx_core::model::FollowUpKind;

pub struct Host(pub Option<AnyView>, pub Vec<AnyView>);

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().relative().children(self.0.clone()).children(self.1.clone())
    }
}

pub fn host<V: Render>(cx: &mut TestAppContext, make: impl FnOnce(&mut Window, &mut gpui::App) -> Entity<V>) -> (Entity<V>, &mut VisualTestContext) {
    let slot: Rc<RefCell<Option<Entity<V>>>> = Rc::default();
    let s2 = slot.clone();
    let (_, vcx) = cx.add_window_view(move |window, cx| {
        let v = make(window, cx);
        *s2.borrow_mut() = Some(v.clone());
        Host(Some(v.into()), Vec::new())
    });
    let v = slot.borrow().clone().unwrap();
    (v, vcx)
}

pub fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

pub fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let b = cx.debug_bounds(selector).unwrap_or_else(|| panic!("no element {selector}"));
    cx.simulate_click(b.center(), Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn tint_submenu_in_a_short_window_stays_inside_and_picking_magenta_pops_the_menu(cx: &mut TestAppContext) {
    let picked: Rc<RefCell<Vec<MenuAction>>> = Rc::default();
    let dismissed = Rc::new(RefCell::new(0));
    let (p, d) = (picked.clone(), dismissed.clone());
    let entries = builders::workspace_menu(false, None);
    let cor = entries.iter().position(|e| e.label() == Some("Cor")).unwrap();
    assert!(find(&entries, "Cor").is_some());
    let (menu, cx) = host(cx, move |_, cx| {
        cx.new(|_| {
            let mut m = ContextMenu::new(entries, point(px(40.), px(20.)), move |a, _, _| p.borrow_mut().push(a), move |_, _| *d.borrow_mut() += 1);
            m.preopen(Some(cor));
            m
        })
    });
    cx.simulate_resize(size(px(800.), px(240.)));
    redraw(cx);
    redraw(cx);
    let sub = cx.debug_bounds("submenu").expect("submenu drawn");
    assert!(f32::from(sub.origin.y) >= 8. && f32::from(sub.bottom()) <= 232.);
    let main = cx.debug_bounds("menu").expect("menu drawn");
    assert!(f32::from(main.bottom()) <= 232.);
    assert_eq!(menu.read_with(cx, |m, _| m.open_submenu()), Some(cor));
    click(cx, "menu:Sem cor");
    assert_eq!(picked.borrow().as_slice(), &[MenuAction::Tint(None)]);
    assert_eq!(*dismissed.borrow(), 1);
}

struct Clicks {
    count: Rc<RefCell<usize>>,
    confetti: Entity<ConfettiLayer>,
}

impl Render for Clicks {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let count = self.count.clone();
        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("fire")
                    .debug_selector(|| "fire".into())
                    .size(px(120.))
                    .on_click(move |_, _, _| *count.borrow_mut() += 1),
            )
            .child(self.confetti.clone())
    }
}

#[gpui::test]
fn confetti_overlay_lives_its_life_and_lets_taps_through(cx: &mut TestAppContext) {
    let count = Rc::new(RefCell::new(0));
    let c2 = count.clone();
    let (view, cx) = host(cx, move |_, cx| {
        let confetti = cx.new(|_| ConfettiLayer::new());
        cx.new(|_| Clicks { count: c2, confetti })
    });
    let layer = view.read_with(cx, |v, _| v.confetti.clone());
    layer.update(cx, |l, cx| l.fire(cx));
    redraw(cx);
    cx.executor().advance_clock(Duration::from_millis(300));
    click(cx, "fire");
    assert_eq!(*count.borrow(), 1);
    cx.executor().advance_clock(Duration::from_millis(900));
    assert!(layer.read_with(cx, |l, _| l.active()));
    cx.executor().advance_clock(Duration::from_millis(1600));
    cx.run_until_parked();
    assert!(!layer.read_with(cx, |l, _| l.active()));
}

#[gpui::test]
fn new_workspace_named_atrium_with_two_folders_ticked_creates_it(cx: &mut TestAppContext) {
    let result: Rc<RefCell<Option<Option<FormResult>>>> = Rc::default();
    let r2 = result.clone();
    let folders = [("a".to_owned(), false), ("b".to_owned(), false), ("c".to_owned(), false)];
    let (dialog, cx) = host(cx, move |window, cx| {
        cx.new(|cx| FormDialog::new(specs::new_workspace(&folders), move |r, _, _| *r2.borrow_mut() = Some(r), window, cx))
    });
    click(cx, "button:Criar");
    assert!(result.borrow().is_none());
    cx.simulate_input("ATRIUM");
    click(cx, "check:a");
    click(cx, "check:c");
    assert_eq!(dialog.read_with(cx, |d, cx| d.field_text(0, cx)), "ATRIUM");
    click(cx, "button:Criar");
    let got = result.borrow().clone().unwrap().unwrap();
    assert_eq!(got.action, 1);
    assert_eq!(got.fields, vec!["ATRIUM".to_owned()]);
    assert_eq!(got.checks, vec![true, false, true]);
}

#[gpui::test]
fn prompt_submits_on_enter_and_cancels_on_escape(cx: &mut TestAppContext) {
    let result: Rc<RefCell<Vec<Option<FormResult>>>> = Rc::default();
    let r2 = result.clone();
    let (_, cx) = host(cx, move |window, cx| {
        cx.new(|cx| FormDialog::new(specs::prompt_text("Renomear feature", "Nome", "velho"), move |r, _, _| r2.borrow_mut().push(r), window, cx))
    });
    assert!(cx.debug_bounds("title:Renomear feature").is_some());
    cx.simulate_keystrokes("backspace backspace backspace backspace backspace");
    cx.simulate_input("novo");
    cx.simulate_keystrokes("enter");
    cx.simulate_keystrokes("escape");
    let r = result.borrow();
    assert_eq!(r[0].as_ref().unwrap().fields, vec!["novo".to_owned()]);
    assert!(r[1].is_none());
}

#[gpui::test]
fn ctrl_a_then_typing_replaces_the_prefilled_text(cx: &mut TestAppContext) {
    let result: Rc<RefCell<Vec<Option<FormResult>>>> = Rc::default();
    let r2 = result.clone();
    let (_, cx) = host(cx, move |window, cx| {
        cx.new(|cx| FormDialog::new(specs::prompt_text("Renomear painel", "Título", "proj"), move |r, _, _| r2.borrow_mut().push(r), window, cx))
    });
    cx.simulate_keystrokes("ctrl-a n o v o");
    cx.simulate_keystrokes("enter");
    cx.simulate_keystrokes("ctrl-a backspace enter");
    let r = result.borrow();
    assert_eq!(r[0].as_ref().unwrap().fields, vec!["novo".to_owned()]);
    assert_eq!(r[1].as_ref().unwrap().fields, vec![String::new()]);
}

#[gpui::test]
fn new_flow_opens_only_with_a_prompt_and_carries_the_review_step(cx: &mut TestAppContext) {
    let result: Rc<RefCell<Option<Option<FlowResult>>>> = Rc::default();
    let r2 = result.clone();
    let (editor, cx) = host(cx, move |window, cx| {
        cx.new(|cx| FlowEditor::new(Trigger::New { place: "meu-repo".into() }, vec![], vec![], move |r, _, _| *r2.borrow_mut() = Some(r), window, cx))
    });
    assert_eq!(editor.read_with(cx, |e, _| e.verb()), "Abrir a sessão");
    click(cx, "button:Abrir a sessão");
    assert!(result.borrow().is_none());
    cx.simulate_input("Arrume o login");
    click(cx, "add:Outra sessão");
    assert_eq!(editor.read_with(cx, |e, _| e.verb()), "Abrir e armar");
    click(cx, "button:Abrir e armar");
    let got = result.borrow().clone().unwrap().unwrap();
    assert_eq!(got.prompt, "Arrume o login");
    assert_eq!(got.steps.len(), 1);
    assert_eq!(got.steps[0].kind, FollowUpKind::NewSession);
    assert!(got.steps[0].text.contains("git diff"));
}
