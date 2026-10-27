use mx_plugins::ui::icons::{GENERIC_ICON, ICONS};
use mx_rfw::Value;
use mx_theme::{Color, Typography, default_theme};

use super::icons::{material_path, rasterize, sized_root};
use super::style::rfw_theme;

#[test]
fn every_plugin_icon_name_has_a_drawn_glyph() {
    for (name, material) in ICONS {
        assert!(material_path(material).is_some(), "{name} -> {material}");
    }
    for extra in [GENERIC_ICON, "circle_outlined", "more_horiz_rounded", "check_box", "check_box_outline_blank"] {
        assert!(material_path(extra).is_some(), "{extra}");
    }
}

#[test]
fn the_theme_reaches_rfw_as_argb_ints_with_the_mono_family() {
    let theme = default_theme();
    let v = rfw_theme(&theme, &Typography::standard());
    assert_eq!(v.get("fg").as_int(), Some(i64::from(theme.fg.to_argb())));
    assert_eq!(v.get("sidebar").as_int(), Some(i64::from(theme.bg_sidebar.to_argb())));
    assert_eq!(v.get("mono"), &Value::from("Hack"));
    assert_eq!(v.get("dark").as_bool(), Some(theme.dark));
}

#[test]
fn on_accent_is_white_over_a_dark_accent() {
    let mut p = default_theme().palette().clone();
    p.accent = Color::opaque(0x1F3A93);
    let v = rfw_theme(&p, &Typography::standard());
    assert_eq!(v.get("onAccent").as_int(), Some(0xFFFF_FFFF));
}

#[test]
fn a_plugin_svg_gets_the_requested_size_on_its_root_tag() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="M0 0h24v24H0z"/></svg>"#;
    let sized = sized_root(svg, 16.0).unwrap();
    assert!(sized.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="16" height="16">"#));
}

#[test]
fn rasterized_svgs_are_bgra_at_twice_the_size() {
    let img = rasterize(r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="#ff0000"/></svg>"##).unwrap();
    assert_eq!(img.size(0).width.0, 8);
    assert_eq!(&img.as_bytes(0).unwrap()[..4], &[0, 0, 255, 255]);
}
