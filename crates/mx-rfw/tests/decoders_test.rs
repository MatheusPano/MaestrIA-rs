mod common;

use common::content;
use mx_rfw::decoders::*;
use mx_rfw::widgets::core_layout::{Stack, Text};
use mx_rfw::widgets::core_widgets::{Expanded, Opacity, Positioned, ScrollList};
use mx_rfw::widgets::maestria::{Glyph, IconButton, Pressable};
use mx_rfw::widgets::{CircularProgressIndicator, Local, decode};
use mx_rfw::{Node, Value, Widget, evaluate, parse_library_file};
use serde_json::json;

fn node(call: &str) -> Node {
    let src = format!("import core.widgets;\nimport core.material;\nimport maestria;\nwidget root = {call};");
    let lib = parse_library_file(&src).unwrap();
    match evaluate(&lib, "root", &content(&json!({"i": 12, "d": 12.0}).to_string())) {
        Widget::Node(n) => n,
        Widget::Error(e) => panic!("{e}"),
    }
}

fn arg(call_args: &str, key: &str) -> Value {
    node(&format!("SizedBox({call_args})")).arg(key).clone()
}

#[test]
fn edge_insets_lists_follow_start_top_end_bottom_with_fallbacks() {
    let e = |l: &str| edge_insets(&arg(&format!("x: {l}"), "x"));
    assert_eq!(e("[8.0]"), Some(EdgeInsets { start: 8.0, top: 8.0, end: 8.0, bottom: 8.0 }));
    assert_eq!(e("[1.0, 2.0]"), Some(EdgeInsets { start: 1.0, top: 2.0, end: 1.0, bottom: 2.0 }));
    assert_eq!(e("[1.0, 2.0, 3.0]"), Some(EdgeInsets { start: 1.0, top: 2.0, end: 3.0, bottom: 2.0 }));
    assert_eq!(e("[1.0, 2.0, 3.0, 4.0]"), Some(EdgeInsets { start: 1.0, top: 2.0, end: 3.0, bottom: 4.0 }));
}

#[test]
fn edge_insets_reject_a_bare_number_and_int_elements() {
    assert_eq!(edge_insets(&arg("x: 8.0", "x")), None);
    assert_eq!(edge_insets(&arg("x: [8]", "x")), None);
    assert_eq!(edge_insets(&arg("x: [8.0, 2]", "x")), Some(EdgeInsets { start: 8.0, top: 8.0, end: 8.0, bottom: 8.0 }));
}

#[test]
fn color_reads_an_int_as_argb_and_rejects_doubles_and_strings() {
    assert_eq!(color(&arg("x: 0xFFF2762E", "x")), Some(Color(0xFFF2_762E)));
    assert_eq!(color(&arg("x: 0x00000000", "x")).map(Color::alpha), Some(0));
    assert_eq!(color(&arg("x: 1.0", "x")), None);
    assert_eq!(color(&arg("x: \"#fff\"", "x")), None);
    assert_eq!(color(&Value::Int(0x1_FFFF_FFFF)), Some(Color(0xFFFF_FFFF)));
}

#[test]
fn alignment_needs_y_and_x_or_start() {
    assert_eq!(alignment(&arg("x: { x: 0.0, y: -1.0 }", "x")), Some(Alignment::Absolute { x: 0.0, y: -1.0 }));
    assert_eq!(alignment(&arg("x: { start: 1.0, y: 0.0 }", "x")), Some(Alignment::Directional { start: 1.0, y: 0.0 }));
    assert_eq!(alignment(&arg("x: { x: 1.0 }", "x")), None);
    assert_eq!(alignment(&arg("x: { x: 1, y: 0 }", "x")), None);
}

#[test]
fn border_radius_takes_radius_maps_with_y_defaulting_to_x() {
    let r = border_radius(&arg("x: [{ x: 1.0 }, { x: 1.0 }, { x: 0.0 }, { x: 0.0 }]", "x")).unwrap();
    assert_eq!(r.top_start, Radius { x: 1.0, y: 1.0 });
    assert_eq!(r.bottom_end, Radius { x: 0.0, y: 0.0 });
    let all = border_radius(&arg("x: [{ x: 10.0, y: 4.0 }]", "x")).unwrap();
    assert_eq!(all.bottom_end, Radius { x: 10.0, y: 4.0 });
    assert_eq!(border_radius(&arg("x: 10.0", "x")), None);
}

#[test]
fn border_sides_default_to_black_one_pixel_solid_and_none_style_hides_a_side() {
    let b = border(&arg("x: [{ style: \"none\" }, { color: 0xFF112233, width: 2.0 }]", "x")).unwrap();
    assert_eq!(b.start.style, BorderStyle::None);
    assert_eq!(b.top, BorderSide { color: Color(0xFF11_2233), width: 2.0, style: BorderStyle::Solid });
    assert_eq!(b.bottom, b.top);
    assert_eq!(b.end.style, BorderStyle::None);
    let int_width = border(&arg("x: [{ width: 2 }]", "x")).unwrap();
    assert_eq!(int_width.start, BorderSide { color: Color::BLACK, width: 1.0, style: BorderStyle::Solid });
}

#[test]
fn box_decoration_reads_only_the_box_type() {
    let d = decoration(&arg("x: { type: \"box\", shape: \"circle\", color: 0xFF000001, borderRadius: [{ x: 3.0 }] }", "x")).unwrap();
    assert_eq!(d.shape, BoxShape::Circle);
    assert_eq!(d.color, Some(Color(0xFF00_0001)));
    assert!(d.border.is_none());
    assert_eq!(decoration(&arg("x: { type: \"shape\" }", "x")), None);
    assert_eq!(decoration(&arg("x: { color: 1 }", "x")), None);
}

#[test]
fn text_style_reads_overflow_from_the_widget_root_like_rfw() {
    let n = node("Text(text: \"a\", overflow: \"ellipsis\", style: { fontSize: 12.0, fontWeight: \"w600\", overflow: \"clip\", height: 1.2, letterSpacing: 0.5, fontFamily: \"mono\" })");
    let s = text_style(&n, "style").unwrap();
    assert_eq!(s.overflow, Some(TextOverflow::Ellipsis));
    assert_eq!(s.font_size, Some(12.0));
    assert_eq!(s.font_weight, Some(FontWeight(600)));
    assert_eq!(s.font_family.as_deref(), Some("mono"));
    assert_eq!(s.decoration, TextDecoration::default());
    assert_eq!(text_style(&n, "missing"), None);
}

#[test]
fn font_weight_accepts_hundreds_names_and_rejects_the_rest() {
    assert_eq!(font_weight(&Value::Int(700)), Some(FontWeight(700)));
    assert_eq!(font_weight(&Value::from("bold")), Some(FontWeight(700)));
    assert_eq!(font_weight(&Value::from("normal")), Some(FontWeight(400)));
    assert_eq!(font_weight(&Value::Int(650)), None);
    assert_eq!(font_weight(&Value::from("W700")), None);
}

#[test]
fn enum_values_compare_case_sensitively() {
    assert_eq!(CrossAxisAlignment::decode(&Value::from("stretch")), Some(CrossAxisAlignment::Stretch));
    assert_eq!(CrossAxisAlignment::decode(&Value::from("Stretch")), None);
}

#[test]
fn duration_defaults_to_200_ms_and_curve_to_fast_out_slow_in() {
    assert_eq!(duration_ms(&Value::Int(140)), 140);
    assert_eq!(duration_ms(&Value::Double(140.0)), 200);
    assert_eq!(curve(&Value::Missing), Curve::FastOutSlowIn);
    assert_eq!(curve(&Value::from("easeIn")), Curve::EaseIn);
}

#[test]
fn grid_delegate_max_cross_axis_extent() {
    let n = node("GridView(gridDelegate: { type: \"maxCrossAxisExtent\", maxCrossAxisExtent: 320.0, mainAxisExtent: 58.0, mainAxisSpacing: 10.0, crossAxisSpacing: 10.0 }, shrinkWrap: true, primary: false)");
    let g = ScrollList::grid_view(&n);
    assert!(g.shrink_wrap);
    assert_eq!(g.primary, Some(false));
    assert_eq!(
        g.grid_delegate,
        Some(GridDelegate::MaxCrossAxisExtent { max_cross_axis_extent: 320.0, main_axis_spacing: 10.0, cross_axis_spacing: 10.0, child_aspect_ratio: 1.0, main_axis_extent: Some(58.0) })
    );
    assert_eq!(ScrollList::grid_view(&node("GridView()")).grid_delegate, Some(GridDelegate::DEFAULT));
}

#[test]
fn positioned_reads_start_and_end_and_ignores_left_and_right() {
    let n = node("Positioned(right: 0.0, left: 3.0, bottom: 0.0, child: Spacer())");
    let p = Positioned::from_node(&n);
    assert_eq!((p.start, p.end, p.bottom, p.top), (None, None, Some(0.0), None));
    let m = node("Positioned(start: -84.0, end: 2.0, child: Spacer())");
    let q = Positioned::from_node(&m);
    assert_eq!((q.start, q.end), (Some(-84.0), Some(2.0)));
}

#[test]
fn opacity_defaults_to_zero() {
    assert_eq!(Opacity::from_node(&node("Opacity(child: Spacer())")).opacity, 0.0);
    assert_eq!(Opacity::from_node(&node("Opacity(opacity: 0.72)")).opacity, 0.72);
}

#[test]
fn a_missing_required_child_is_an_error_and_a_missing_child_list_is_empty() {
    let n = node("Expanded()");
    let e = Expanded::from_node(&n);
    assert_eq!(e.child.unwrap_err(), "Not a widget at [child] (got <missing>) for core.widgets:Expanded.");
    assert!(Stack::from_node(&node("Stack()")).children.is_empty());
    let m = node("Stack(children: 3)");
    let bad = Stack::from_node(&m);
    assert_eq!(bad.children[0].clone().unwrap_err(), "Not a widget list at [children] (got 3) for core.widgets:Stack.");
}

#[test]
fn text_concatenates_string_list_elements_and_defaults_to_empty() {
    assert_eq!(Text::from_node(&node("Text(text: [\"a\", 1, \"b\"])")).text, "ab");
    assert_eq!(Text::from_node(&node("Text(text: 3)")).text, "");
    assert_eq!(Text::from_node(&node("Text(text: data.d, maxLines: 1)")).max_lines, Some(1));
}

#[test]
fn sizes_read_from_json_ints_are_rejected_as_doubles() {
    let n = node("SizedBox(width: data.i, height: data.d)");
    assert_eq!(n.arg("width").as_double(), None);
    assert_eq!(n.arg("height").as_double(), Some(12.0));
}

#[test]
fn circular_progress_with_no_value_is_indeterminate() {
    let c = CircularProgressIndicator::from_node(&node("CircularProgressIndicator(strokeWidth: 3.5)"));
    assert_eq!((c.value, c.stroke_width), (None, 3.5));
    assert_eq!(CircularProgressIndicator::from_node(&node("CircularProgressIndicator()")).stroke_width, 4.0);
}

#[test]
fn icon_button_is_disabled_by_the_flag_or_by_having_no_handler() {
    let enabled = node("IconButton(icon: \"stop\", onPressed: event \"act\" {})");
    assert!(IconButton::from_node(&enabled).enabled());
    let flagged = node("IconButton(disabled: true, onPressed: event \"act\" {})");
    assert!(!IconButton::from_node(&flagged).enabled());
    let b = node("IconButton()");
    let bare = IconButton::from_node(&b);
    assert!(!bare.enabled());
    assert_eq!((bare.size, bare.extent), (16.0, 26.0));
}

#[test]
fn glyph_defaults_to_size_16_and_no_color() {
    let g = Glyph::from_node(&node("Glyph(icon: \"icons/argo.svg\")"));
    assert_eq!((g.icon.as_deref(), g.size, g.color), (Some("icons/argo.svg"), 16.0, None));
}

#[test]
fn a_menu_entry_without_an_action_string_is_a_divider_and_the_label_falls_back_to_the_action() {
    let n = node("Pressable(menu: [{ divider: true, action: \"x\" }, { label: \"\" }, { action: \"go\" }])");
    let p = Pressable::from_node(&n, false);
    assert!(p.menu[0].is_divider() && p.menu[1].is_divider());
    assert_eq!(p.menu[2].shown_label(), "go");
}

#[test]
fn menu_button_never_reads_a_child() {
    let n = node("MenuButton(icon: \"more\", child: Spacer())");
    assert!(matches!(decode(&n), Local::MenuButton(p) if p.child.is_none() && p.button));
}

#[test]
fn decode_maps_every_catalog_widget_and_flags_the_rest_as_unsupported() {
    assert!(matches!(decode(&node("Spacer()")), Local::Spacer { flex: 1 }));
    assert!(matches!(decode(&node("Image()")), Local::Unsupported(_)));
    assert!(matches!(decode(&node("Divider()")), Local::Unsupported(_)));
}
