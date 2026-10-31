use std::rc::Rc;

use gpui::{
    Bounds, Pixels, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point, px, size,
};
use mx_theme::{Typography, default_theme};

use crate::rfw_view::view::RfwView;

const PLAN: &str = r#"ZoomView(maxScale: 6.0, child: FittedBox(fit: "contain",
  child: SizedBox(width: 280.0, height: 140.0, child: Center(child: Text(text: "planta")))))"#;

fn open(cx: &mut TestAppContext) -> &mut VisualTestContext {
    let library = format!("import core.widgets;\nimport maestria;\nwidget root = {PLAN};");
    let (_, vcx) = cx.add_window_view(move |_, _| {
        let mut v = RfwView::new(default_theme().clone(), &Typography::standard(), None, Rc::new(|_, _, _| {}));
        v.set_library(&library);
        v.set_root("root");
        v
    });
    vcx.simulate_resize(size(px(560.0), px(400.0)));
    vcx.run_until_parked();
    vcx
}

fn label(cx: &mut VisualTestContext) -> Bounds<Pixels> {
    cx.run_until_parked();
    cx.debug_bounds("text:planta").expect("the plan text is not painted")
}

fn notches(cx: &mut VisualTestContext, at: gpui::Point<Pixels>, lines: f32) {
    cx.simulate_event(ScrollWheelEvent {
        position: at,
        delta: ScrollDelta::Lines(point(0.0, lines)),
        ..Default::default()
    });
    cx.run_until_parked();
}

fn width(b: Bounds<Pixels>) -> f32 {
    f32::from(b.size.width)
}

#[gpui::test]
fn wheel_zoom_scales_a_fitted_child_on_both_axes_around_the_pointer(cx: &mut TestAppContext) {
    let cx = open(cx);
    let fit = label(cx);
    let at = fit.center();
    notches(cx, at, 3.0);
    let one = label(cx);
    let k = width(one) / width(fit);
    assert!((1.2..1.4).contains(&k), "one notch should zoom ~30%, got {k} ({fit:?} -> {one:?})");
    assert!((f32::from(one.center().x) - f32::from(at.x)).abs() < 3.0, "{at:?} {one:?}");
    assert!((f32::from(one.center().y) - f32::from(at.y)).abs() < 3.0, "{at:?} {one:?}");
    for _ in 0..4 {
        notches(cx, at, 3.0);
    }
    let deep = label(cx);
    assert!(width(deep) / width(fit) > 3.0, "{fit:?} -> {deep:?}");
    assert!(f32::from(deep.origin.x) > 0.0 && f32::from(deep.origin.x + deep.size.width) < 560.0, "{deep:?}");
    for _ in 0..12 {
        notches(cx, at, -3.0);
    }
    let back = label(cx);
    assert!((width(back) - width(fit)).abs() < 1.0, "{fit:?} -> {back:?}");
    assert!((back.origin.x - fit.origin.x).abs() < px(1.0), "{fit:?} -> {back:?}");
}

#[test]
fn focus_command_animates_once_per_rev_and_never_counts_as_a_user_move() {
    use std::time::{Duration, Instant};
    let mut st = super::ZoomState { bounds: Some(Bounds::new(point(px(0.), px(0.)), size(px(1000.), px(800.)))), ..Default::default() };
    let t0 = Instant::now();
    let rect = Some((0.5, 0.5, 0.25, 0.25));
    st.command(Some("focus"), rect, 0, 6.0, t0);
    assert!(!st.animating(), "the mount revision is ignored");
    st.command(Some("focus"), rect, 1, 6.0, t0);
    assert!(st.animating());
    let end = st.current(t0 + Duration::from_secs(1));
    assert!((end.scale - 3.6).abs() < 1e-9, "{end:?}");
    assert_eq!(st.settled_move(), None, "programmatic commands never report onMove");
    st.command(Some("focus"), rect, 1, 6.0, t0);
    assert!(!st.animating(), "same rev does nothing");
    assert!(!st.drag(point(px(0.), px(0.))));
    st.wheel(point(px(500.), px(400.)), -100.0, 6.0);
    let m = st.settled_move().expect("a user wheel reports");
    assert_eq!(m, st.matrix);
    assert_eq!(st.settled_move(), None);
}

#[test]
fn a_programmatic_command_discards_a_pending_user_move() {
    use std::time::{Duration, Instant};
    let mut st = super::ZoomState { bounds: Some(Bounds::new(point(px(0.), px(0.)), size(px(1000.), px(800.)))), ..Default::default() };
    let t0 = Instant::now();
    st.command(Some("focus"), None, 0, 6.0, t0);
    st.wheel(point(px(500.), px(400.)), -100.0, 6.0);
    st.command(Some("focus"), Some((0.5, 0.5, 0.25, 0.25)), 1, 6.0, t0);
    st.current(t0 + Duration::from_secs(1));
    assert_eq!(st.settled_move(), None);
}
