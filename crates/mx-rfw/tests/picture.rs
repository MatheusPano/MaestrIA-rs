mod common;

use common::content;
use mx_rfw::widgets::picture::Picture;
use mx_rfw::widgets::sprite::plugin_or_data_file_of;
use mx_rfw::widgets::zoom_view::BoxFit;
use mx_rfw::widgets::{Local, decode};
use mx_rfw::{LocalLibrary, Widget, evaluate, parse_library_file};

fn tree(src: &str, data: &str) -> Widget {
    let lib = parse_library_file(&format!("import core.widgets;\nimport maestria;\n{src}")).unwrap();
    evaluate(&lib, "root", &content(data))
}

#[test]
fn picture_resolves_inside_plugin_then_data_dir() {
    let tmp = std::env::temp_dir().join(format!("mx-rfw-picture-{}", std::process::id()));
    let plugin = tmp.join("plugin");
    let data = tmp.join("data");
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::create_dir_all(data.join("png")).unwrap();
    std::fs::write(data.join("a.png"), b"x").unwrap();
    std::fs::write(data.join("png/b.png"), b"x").unwrap();
    std::fs::write(plugin.join("c.png"), b"x").unwrap();
    std::fs::write(data.join("c.png"), b"x").unwrap();
    std::fs::write(tmp.join("x.png"), b"x").unwrap();
    assert_eq!(plugin_or_data_file_of(Some("a.png"), Some(&plugin), Some(&data)), Some(data.join("a.png")));
    assert_eq!(plugin_or_data_file_of(Some("png/b.png"), Some(&plugin), Some(&data)), Some(data.join("png/b.png")));
    assert_eq!(plugin_or_data_file_of(Some("c.png"), Some(&plugin), Some(&data)), Some(plugin.join("c.png")));
    assert_eq!(plugin_or_data_file_of(Some("a.png"), None, Some(&data)), Some(data.join("a.png")));
    let absolute = data.join("a.png").to_string_lossy().into_owned();
    for rel in ["../x.png", "png/../../x.png", absolute.as_str(), "", "nao-existe.png"] {
        assert_eq!(plugin_or_data_file_of(Some(rel), Some(&plugin), Some(&data)), None, "{rel}");
    }
    assert_eq!(plugin_or_data_file_of(None, Some(&plugin), Some(&data)), None);
    assert_eq!(plugin_or_data_file_of(Some("a.png"), Some(&plugin), None), None);
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn picture_parses_args() {
    assert!(LocalLibrary::Maestria.has("Picture"));
    let t = tree(
        r#"widget root = Picture(path: data.p, width: 320, height: 200.5, fit: "contain");"#,
        r#"{"p": "png/home.light.png"}"#,
    );
    let Local::Picture(p) = decode(t.as_node().unwrap()) else { panic!("not a Picture") };
    assert_eq!(
        *p,
        Picture { path: Some("png/home.light.png".into()), width: Some(320.0), height: Some(200.5), fit: BoxFit::Contain }
    );
}

#[test]
fn picture_defaults_to_contain_with_no_size() {
    let t = tree(r#"widget root = Picture(path: "a.png", fit: "cover");"#, "{}");
    let Local::Picture(p) = decode(t.as_node().unwrap()) else { panic!("not a Picture") };
    assert_eq!((p.width, p.height, p.fit), (None, None, BoxFit::Cover));
    let t = tree("widget root = Picture();", "{}");
    let Local::Picture(p) = decode(t.as_node().unwrap()) else { panic!("not a Picture") };
    assert_eq!(*p, Picture { path: None, width: None, height: None, fit: BoxFit::Contain });
}
