use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext, px, size};
use mx_theme::{Typography, default_theme};
use serde_json::{Value, json};

use super::view::RfwView;
use super::{ActionSink, ViewAction};

type Log = Rc<RefCell<Vec<ViewAction>>>;

fn e2e_library() -> String {
    std::fs::read_to_string(format!("{}/../../scripts/e2e/fixtures/widgets-plugin/ui.rfwtxt", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn open(cx: &mut TestAppContext, data: Value) -> (Entity<RfwView>, Log, &mut VisualTestContext) {
    let log: Log = Rc::default();
    let l = log.clone();
    let sink: ActionSink = Rc::new(move |a, _, _| l.borrow_mut().push(a));
    let (view, vcx) = cx.add_window_view(move |_, _| {
        let mut v = RfwView::new(default_theme().clone(), &Typography::standard(), None, sink);
        v.set_library(&e2e_library());
        v.set_root("root");
        v.merge_data(data.as_object().unwrap());
        v
    });
    vcx.simulate_resize(size(px(820.0), px(600.0)));
    vcx.run_until_parked();
    (view, log, vcx)
}

fn update(view: &Entity<RfwView>, cx: &mut VisualTestContext, d: Value) {
    view.update(cx, |v, cx| {
        v.merge_data(d.as_object().unwrap());
        cx.notify();
    });
    cx.run_until_parked();
}

fn field_text(view: &Entity<RfwView>, cx: &mut VisualTestContext) -> String {
    let f = view.read_with(cx, |v, _| v.text_field("chat")).expect("no TextInput field");
    f.read_with(cx, |f, _| f.text().to_owned())
}

#[gpui::test]
fn in1_the_plugin_clears_the_field_by_changing_clear(cx: &mut TestAppContext) {
    let (view, _, cx) = open(cx, json!({ "sent": 0, "zoomLabel": "mapa" }));
    let f = view.read_with(cx, |v, _| v.text_field("chat")).unwrap();
    f.update(cx, |f, cx| f.type_text("roda os testes", cx));
    update(&view, cx, json!({ "zoomLabel": "outro" }));
    assert_eq!(field_text(&view, cx), "roda os testes");
    update(&view, cx, json!({ "sent": 1 }));
    assert_eq!(field_text(&view, cx), "");
}
