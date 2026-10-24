use std::cell::Cell;
use std::rc::Rc;

use gpui::{Bounds, Entity, Pixels, TestAppContext, VisualTestContext, px, size};
use mx_theme::{Typography, default_theme};

use super::view::RfwView;

const LONG: &str = "api-de-pagamentos-recorrentes-com-um-nome-bem-comprido-que-nao-cabe-em-lugar-nenhum";
const WORDS: &str = "texto com muitas palavras que precisa quebrar em varias linhas quando o espaco acaba";

fn open<'a>(cx: &'a mut TestAppContext, body: &str, width: f32) -> (Entity<RfwView>, &'a mut VisualTestContext) {
    let library = format!("import core.widgets;\nwidget root = {body};");
    let (view, vcx) = cx.add_window_view(move |_, _| {
        let mut v = RfwView::new(default_theme().clone(), &Typography::standard(), None, Rc::new(|_, _, _| {}));
        v.set_library(&library);
        v.set_root("root");
        v
    });
    vcx.simulate_resize(size(px(width), px(400.0)));
    vcx.run_until_parked();
    (view, vcx)
}

fn text(cx: &mut VisualTestContext, content: &str) -> Bounds<Pixels> {
    let selector: &'static str = Box::leak(format!("text:{content}").into_boxed_str());
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(selector).unwrap_or_else(|| panic!("no text {content}"))
}

fn right(b: Bounds<Pixels>) -> f32 {
    f32::from(b.origin.x + b.size.width)
}

#[gpui::test]
fn an_ellipsis_text_in_an_expanded_column_stays_inside_its_card(cx: &mut TestAppContext) {
    let body = format!(
        r#"Container(width: 200.0, child: Row(children: [SizedBox(width: 20.0), Expanded(child: Column(crossAxisAlignment: "start", children: [Text(text: "{LONG}", maxLines: 1, overflow: "ellipsis")]))]))"#
    );
    let (_, cx) = open(cx, &body, 400.0);
    let b = text(cx, LONG);
    assert!(f32::from(b.origin.x) >= 20.0, "{b:?}");
    assert!(right(b) <= 200.0, "{b:?}");
}

#[gpui::test]
fn a_fixed_width_container_shrinks_to_a_narrower_parent(cx: &mut TestAppContext) {
    let body = format!(r#"Column(crossAxisAlignment: "start", children: [Container(width: 320.0, child: Text(text: "{WORDS}"))])"#);
    let (_, cx) = open(cx, &body, 200.0);
    assert!(right(text(cx, WORDS)) <= 200.0);
}

#[gpui::test]
fn a_max_row_inside_padding_in_a_start_column_fills_the_width(cx: &mut TestAppContext) {
    let body = r#"Column(crossAxisAlignment: "start", children: [Padding(padding: [0.0], child: Row(children: [SizedBox(width: 74.0, child: Text(text: "k")), Expanded(child: Text(text: "valor", maxLines: 1, overflow: "ellipsis"))]))])"#;
    let (_, cx) = open(cx, body, 300.0);
    let b = text(cx, "valor");
    assert_eq!(f32::from(b.origin.x), 74.0);
    assert_eq!(f32::from(b.size.width), 300.0 - 74.0);
}

#[gpui::test]
fn a_text_in_a_row_without_expanded_is_not_squeezed(cx: &mut TestAppContext) {
    let body = format!(r#"Column(children: [Row(children: [SizedBox(width: 50.0), Text(text: "{WORDS}")]), Text(text: "uma")])"#);
    let (_, cx) = open(cx, &body, 200.0);
    let line = text(cx, "uma").size.height;
    let b = text(cx, WORDS);
    assert!(right(b) > 200.0, "{b:?}");
    assert!(b.size.height < line * 2.0, "{b:?}");
}

#[gpui::test]
fn a_fixed_box_in_a_horizontal_scroll_keeps_its_width(cx: &mut TestAppContext) {
    let body = format!(r#"SingleChildScrollView(scrollDirection: "horizontal", child: SizedBox(width: 600.0, child: Text(text: "{WORDS}")))"#);
    let (_, cx) = open(cx, &body, 200.0);
    assert!(right(text(cx, WORDS)) > 300.0);
}

#[gpui::test]
fn a_positioned_child_without_a_width_keeps_its_fixed_width(cx: &mut TestAppContext) {
    let body = format!(r#"Stack(children: [SizedBox(width: 10.0, height: 10.0), Positioned(start: 10.0, top: 0.0, child: Container(width: 236.0, child: Text(text: "{WORDS}")))])"#);
    let (_, cx) = open(cx, &body, 150.0);
    assert!(right(text(cx, WORDS)) > 150.0);
}

#[gpui::test]
fn a_horizontal_scroll_of_a_stretched_row_fills_the_height_under_an_align(cx: &mut TestAppContext) {
    let body = r#"Column(crossAxisAlignment: "stretch", children: [Expanded(child: Align(alignment: { x: 0.0, y: -1.0 }, child: SingleChildScrollView(scrollDirection: "horizontal", child: Row(mainAxisSize: "min", crossAxisAlignment: "stretch", children: [SizedBox(width: 500.0, child: Column(crossAxisAlignment: "stretch", children: [Text(text: "topo"), Expanded(child: SizedBox()), Text(text: "fundo")]))]))))])"#;
    let (_, cx) = open(cx, body, 300.0);
    assert_eq!(f32::from(text(cx, "topo").origin.y), 0.0);
    assert!(f32::from(text(cx, "fundo").origin.y) > 350.0);
}

#[gpui::test]
fn shrinking_the_window_reflows_an_ellipsis_text(cx: &mut TestAppContext) {
    let body = format!(r#"Row(children: [Expanded(child: Text(text: "{LONG}", maxLines: 1, overflow: "ellipsis"))])"#);
    let (_, cx) = open(cx, &body, 500.0);
    assert!(right(text(cx, LONG)) <= 500.0);
    cx.simulate_resize(size(px(240.0), px(400.0)));
    cx.run_until_parked();
    assert!(right(text(cx, LONG)) <= 240.0);
}

#[gpui::test]
fn resizing_a_grid_view_lays_it_out_without_a_second_frame(cx: &mut TestAppContext) {
    let body = r#"GridView(gridDelegate: { type: "maxCrossAxisExtent", maxCrossAxisExtent: 100.0, mainAxisExtent: 30.0 }, children: [Text(text: "a"), Text(text: "b"), Text(text: "c")])"#;
    let (view, cx) = open(cx, body, 330.0);
    assert_eq!(f32::from(text(cx, "b").origin.x), 330.0 / 4.0);
    let notified = Rc::new(Cell::new(0));
    let n = notified.clone();
    let _sub = cx.update(|_, app| app.observe(&view, move |_, _| n.set(n.get() + 1)));
    cx.simulate_resize(size(px(150.0), px(400.0)));
    cx.run_until_parked();
    assert_eq!(f32::from(text(cx, "c").origin.y), 30.0);
    assert_eq!(notified.get(), 0);
}
