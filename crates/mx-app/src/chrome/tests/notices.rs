use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{AppContext, IntoElement, ParentElement, Render, Styled, TestAppContext, div};

use super::ui::{click, host, redraw};
use crate::chrome::demo::demo_notices;
use crate::chrome::notices::{Banner, NoticeCenter, NoticeCenterProps, RowHandlers, ToastStack, cascade_total, meta, sentence};

fn center(cx: &mut TestAppContext) -> (gpui::Entity<NoticeCenter>, &mut gpui::VisualTestContext, Rc<RefCell<usize>>) {
    let cleared = Rc::new(RefCell::new(0));
    let c2 = cleared.clone();
    let (c, vcx) = host(cx, move |_, cx| {
        cx.new(|cx| {
            let mut c = NoticeCenter::new(NoticeCenterProps {
                on_toggle_dnd: Rc::new(|_, _| {}),
                on_read_all: Rc::new(|_, _| {}),
                on_clear_all: Rc::new(move |_| *c2.borrow_mut() += 1),
                on_close: Rc::new(|_, _| {}),
                on_open: Rc::new(|_, _, _| {}),
                on_dismiss: Rc::new(|_, _, _| {}),
            });
            c.set(demo_notices().into_iter().take(2).collect(), false, cx);
            c
        })
    });
    (c, vcx, cleared)
}

#[gpui::test]
fn clear_all_keeps_rows_during_the_cascade_then_shows_the_empty_text(cx: &mut TestAppContext) {
    let (c, cx, cleared) = center(cx);
    click(cx, "icon:Limpar tudo");
    cx.executor().advance_clock(Duration::from_millis(100));
    assert_eq!(c.read_with(cx, |c, _| c.len()), 2);
    assert_eq!(*cleared.borrow(), 0);
    cx.executor().advance_clock(cascade_total(2));
    redraw(cx);
    assert_eq!(*cleared.borrow(), 1);
    assert_eq!(c.read_with(cx, |c, _| c.len()), 0);
    assert!(cx.debug_bounds("notice-empty").is_some());
}

#[gpui::test]
fn closing_the_list_mid_cascade_still_clears(cx: &mut TestAppContext) {
    let (c, cx, cleared) = center(cx);
    click(cx, "icon:Limpar tudo");
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.update(|_, _| drop(c));
    cx.executor().advance_clock(cascade_total(2));
    cx.run_until_parked();
    assert_eq!(*cleared.borrow(), 1);
}

#[gpui::test]
fn a_removed_toast_slides_out_then_leaves(cx: &mut TestAppContext) {
    let (s, cx) = host(cx, |_, cx| cx.new(|_| ToastStack::new(RowHandlers { on_open: Rc::new(|_, _, _| {}), on_close: Rc::new(|_, _, _| {}), on_hover: None })));
    s.update(cx, |s, cx| s.set(demo_notices().into_iter().take(1).collect(), cx));
    redraw(cx);
    let b = cx.debug_bounds("toast:1").expect("toast drawn");
    assert_eq!(f32::from(b.size.width), 380.);
    s.update(cx, |s, cx| s.set(Vec::new(), cx));
    assert_eq!(s.read_with(cx, |s, _| s.ids()), vec![1]);
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
    assert!(s.read_with(cx, |s, _| s.ids()).is_empty());
}

#[test]
fn finished_notice_reads_current_title_and_verb() {
    let n = &demo_notices()[2];
    let (text, bold) = sentence(n);
    assert_eq!(text, "Plugin do ArgoCD terminou o trabalho");
    assert_eq!(&text[bold[0].clone()], "Plugin do ArgoCD");
    let gone = &demo_notices()[3];
    assert_eq!(meta(gone), "maestria · painel fechado · 12min");
    assert_eq!(sentence(gone).0, "Lint sugeriu uma tarefa: Remover código morto");
}

struct BannerHost;

impl Render for BannerHost {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let text = "backapply: o Claude saiu na largada (código 1) — abra o painel pra ver o motivo";
        div().size_full().flex().flex_col().items_end().child(Banner::new(text, Rc::new(|_, _| {})))
    }
}

#[gpui::test]
fn a_long_banner_wraps_inside_its_max_width(cx: &mut TestAppContext) {
    let (_, cx) = host(cx, |_, cx| cx.new(|_| BannerHost));
    cx.executor().advance_clock(Duration::from_millis(300));
    redraw(cx);
    let b = cx.debug_bounds("banner:backapply: o Claude saiu na largada (código 1) — abra o painel pra ver o motivo").expect("banner drawn");
    assert!(f32::from(b.size.width) <= 460.);
    assert!(f32::from(b.size.height) > 24. + 17.5 * 1.5, "text did not wrap: {b:?}");
}
