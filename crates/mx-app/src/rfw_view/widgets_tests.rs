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

fn picture_dir(name: &str) -> std::path::PathBuf {
    let tmp = std::env::temp_dir().join(format!("mx-app-picture-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    tmp
}

#[test]
fn seventy_pictures_drawn_in_one_frame_are_decoded_once() {
    use super::picture::{decoded, frame_painted, load, set_budget};
    set_budget(100);
    let tmp = picture_dir("frame");
    let files: Vec<_> = (0..70).map(|i| tmp.join(format!("{i}.png"))).collect();
    for f in &files {
        write_png(f, 1, 1);
    }
    for f in &files {
        load(f).unwrap();
    }
    frame_painted();
    let before = decoded();
    for f in &files {
        load(f).unwrap();
    }
    assert_eq!(decoded(), before);
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn the_picture_cache_stays_within_its_byte_budget_across_frames() {
    use super::picture::{cached_bytes, frame_painted, load, set_budget};
    set_budget(1000);
    let tmp = picture_dir("budget");
    for i in 0..5 {
        let f = tmp.join(format!("{i}.png"));
        write_png(&f, 10, 10);
        load(&f).unwrap();
    }
    assert_eq!(cached_bytes(), 2000);
    frame_painted();
    let f = tmp.join("new.png");
    write_png(&f, 10, 10);
    load(&f).unwrap();
    assert!(cached_bytes() <= 1000, "{}", cached_bytes());
    assert!(super::picture::cached() >= 1);
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn a_deleted_picture_drops_its_cached_image() {
    use super::picture::{cached, load};
    let tmp = picture_dir("gone");
    let f = tmp.join("a.png");
    write_png(&f, 2, 2);
    load(&f).unwrap();
    assert_eq!(cached(), 1);
    std::fs::remove_file(&f).unwrap();
    assert!(load(&f).is_none());
    assert_eq!(cached(), 0);
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn box_size_fills_a_missing_side_from_the_natural_aspect() {
    use super::picture::box_size;
    assert_eq!(box_size(Some(10.0), Some(20.0), Some((4.0, 3.0))), (10.0, 20.0));
    assert_eq!(box_size(Some(40.0), None, Some((4.0, 3.0))), (40.0, 30.0));
    assert_eq!(box_size(None, Some(30.0), Some((4.0, 3.0))), (40.0, 30.0));
    assert_eq!(box_size(None, None, Some((4.0, 3.0))), (4.0, 3.0));
    assert_eq!(box_size(Some(40.0), None, None), (40.0, 0.0));
    assert_eq!(box_size(None, None, Some((0.0, 3.0))), (0.0, 0.0));
}

#[test]
fn placed_centers_the_fitted_image_and_fill_stretches_it() {
    use super::picture::placed;
    use mx_rfw::widgets::zoom_view::BoxFit;
    assert_eq!(placed(BoxFit::Fill, (4.0, 3.0), (100.0, 50.0)), (0.0, 0.0, 100.0, 50.0));
    assert_eq!(placed(BoxFit::Contain, (4.0, 2.0), (100.0, 100.0)), (0.0, 25.0, 100.0, 50.0));
    assert_eq!(placed(BoxFit::Cover, (4.0, 2.0), (100.0, 100.0)), (-50.0, 0.0, 200.0, 100.0));
    assert_eq!(placed(BoxFit::None, (10.0, 10.0), (100.0, 50.0)), (45.0, 20.0, 10.0, 10.0));
}
