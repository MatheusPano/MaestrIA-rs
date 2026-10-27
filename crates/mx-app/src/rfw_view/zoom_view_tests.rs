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
