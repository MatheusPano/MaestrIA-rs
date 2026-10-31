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

fn write_png(path: &std::path::Path, w: u32, h: u32) {
    image::RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 255])).save(path).unwrap();
}

#[test]
fn a_picture_rewritten_with_a_different_png_loads_the_new_image() {
    let tmp = std::env::temp_dir().join(format!("mx-app-picture-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let file = tmp.join("home.light.png");
    write_png(&file, 4, 3);
    let first = super::picture::load(&file).expect("first png");
    assert_eq!(first.size, (4, 3));
    let again = super::picture::load(&file).unwrap();
    assert!(std::sync::Arc::ptr_eq(&first.image, &again.image));
    write_png(&file, 7, 5);
    let second = super::picture::load(&file).expect("rewritten png");
    assert_eq!(second.size, (7, 5));
    assert!(!std::sync::Arc::ptr_eq(&first.image, &second.image));
    std::fs::remove_file(&file).unwrap();
    assert!(super::picture::load(&file).is_none());
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn the_picture_cache_keeps_at_most_64_images() {
    let tmp = std::env::temp_dir().join(format!("mx-app-picture-lru-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    for i in 0..70 {
        let file = tmp.join(format!("{i}.png"));
        write_png(&file, 1, 1);
        super::picture::load(&file).unwrap();
    }
    assert_eq!(super::picture::cached(), 64);
    std::fs::remove_dir_all(&tmp).ok();
}
