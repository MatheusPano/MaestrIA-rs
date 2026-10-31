mod common;

use std::time::Duration;

use common::{content, find, fixtures_dir, problems};
use mx_rfw::widgets::maestria::{MARKDOWN_BASE_SIZE, anchor_key};
use mx_rfw::widgets::sprite::{Sprite, plugin_file_of, sprite_frame};
use mx_rfw::widgets::text_input::{ClearSeen, completed};
use mx_rfw::widgets::zoom_view::{
    BoxFit, Transform, ZoomView, clamp_translation, command_transform, fitted_scale, fitted_placement, focus_in_placement, focus_transform, should_report,
    wheel_factor, zoom_around, zoom_target,
};
use mx_rfw::widgets::{Local, decode};
use mx_rfw::{DynamicContent, LocalLibrary, Node, Value, Widget, evaluate, parse_library_file};

fn tree(src: &str, data: &str) -> Widget {
    let lib = parse_library_file(&format!("import core.widgets;\nimport maestria;\n{src}")).unwrap();
    evaluate(&lib, "root", &content(data))
}

fn root(w: &Widget) -> &Node {
    w.as_node().unwrap_or_else(|| panic!("not a node: {w}"))
}

fn escritorio(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join("escritorio").join(name)).unwrap()
}

#[test]
fn the_new_widgets_are_part_of_the_maestria_library() {
    for name in ["Sprite", "ZoomView", "TextInput", "Anchor", "Selectable", "SelectionLines", "Markdown"] {
        assert!(LocalLibrary::Maestria.has(name), "{name}");
    }
}

#[test]
fn sp1_fps4_frames4_walks_the_frames_in_time() {
    let at = [0, 249, 250, 500, 750, 1000, 1250];
    let frames: Vec<i64> = at.iter().map(|ms| sprite_frame(Duration::from_millis(*ms), 4.0, 4, true)).collect();
    assert_eq!(frames, [0, 0, 1, 2, 3, 0, 1]);
}

#[test]
fn sp2_without_loop_it_stops_at_the_last_frame() {
    assert_eq!(sprite_frame(Duration::from_secs(9), 4.0, 3, false), 2);
}

#[test]
fn sp3_one_frame_or_zero_fps_is_always_the_first_frame() {
    assert_eq!(sprite_frame(Duration::from_secs(3), 8.0, 1, true), 0);
    assert_eq!(sprite_frame(Duration::from_secs(3), 0.0, 4, true), 0);
}

#[test]
fn sp4_the_sheet_must_be_a_file_inside_the_plugin_dir() {
    let tmp = std::env::temp_dir().join(format!("mx-rfw-sprite-{}", std::process::id()));
    let dir = tmp.join("plugin");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.png"), b"x").unwrap();
    std::fs::write(tmp.join("plugin-irmao.png"), b"x").unwrap();
    assert_eq!(plugin_file_of(Some("a.png"), Some(&dir)), Some(dir.join("a.png")));
    for rel in ["../a.png", "/etc/hosts", "", "nao-existe.png", "../plugin-irmao.png"] {
        assert_eq!(plugin_file_of(Some(rel), Some(&dir)), None, "{rel}");
    }
    assert_eq!(plugin_file_of(Some("a.png"), None), None);
    assert_eq!(plugin_file_of(None, Some(&dir)), None);
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn sp5_sprite_numbers_accept_ints_and_the_cell_is_clamped_to_the_grid() {
    let t = tree(
        "widget root = Sprite(sheet: data.s, columns: data.c, rows: 2, row: 1, frames: 4, fps: 10, width: 16, height: 24.0);",
        r#"{"s": "x.png", "c": 4.0}"#,
    );
    let Local::Sprite(s) = decode(root(&t)) else { panic!("not a Sprite") };
    assert_eq!((s.columns, s.rows, s.row, s.frames), (4, 2, 1, 4));
    assert_eq!((s.fps, s.width, s.height), (10.0, Some(16.0), Some(24.0)));
    assert!(s.looping && !s.paused && !s.flip && s.animates());
    assert_eq!(s.frame_at(Duration::from_millis(260)), 2);
    assert_eq!(s.cell(2), (2, 1));
    assert_eq!(s.cell(9), (3, 1));
    let paused = Sprite { paused: true, ..(*s).clone() };
    assert_eq!(paused.frame_at(Duration::from_millis(500)), 0);
    assert_ne!(paused.motion(), s.motion());
}

#[test]
fn sprite_defaults_follow_dart() {
    let t = tree("widget root = Sprite();", "{}");
    let Local::Sprite(s) = decode(root(&t)) else { panic!() };
    assert_eq!((s.columns, s.rows, s.row, s.start, s.frames, s.fps), (1, 1, 0, 0, 1, 6.0));
    assert!(s.looping && s.sheet.is_none() && s.width.is_none());
}

const CHAT: &str = r#"widget root = TextInput(
  id: "chat", multiline: true, clear: data.sent, disabled: data.locked, hint: "responda no terminal",
  placeholder: "fale", sendIcon: "send", allowEmpty: data.empty, maxLines: 4.4,
  commands: [{ name: " /clear ", description: "limpa" }, { name: "compact" }, { name: "context" }, { name: "  " }],
  onSubmit: event "send" {}, onPasteImage: event "image" {}, onArrow: event "arrow" {},
);"#;

fn input(data: &str) -> Widget {
    tree(CHAT, data)
}

#[test]
fn text_input_args_follow_dart() {
    let t = input(r#"{"sent": 0, "locked": false}"#);
    let Local::TextInput(i) = decode(root(&t)) else { panic!("not a TextInput") };
    assert_eq!(i.id, "chat");
    assert!(i.multiline && !i.disabled && !i.autofocus && !i.allow_empty);
    assert_eq!(i.max_lines, 4);
    assert_eq!(i.lines(), 4);
    assert_eq!(i.clear, Some(0));
    assert_eq!(i.font_size, 13.0);
    let names: Vec<&str> = i.commands.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["clear", "compact", "context"]);
    assert_eq!(i.commands[0].description, "limpa");
    assert_eq!(i.on_submit.len(), 1);
    let plain = tree(r#"widget root = TextInput(maxLines: 3);"#, "{}");
    let Local::TextInput(p) = decode(root(&plain)) else { panic!() };
    assert_eq!((p.id.as_str(), p.lines(), p.max_lines), ("", 1, 3));
}

#[test]
fn in1_submit_sends_trimmed_text_and_clear_empties_only_when_it_changes() {
    let t = input(r#"{"sent": 0, "locked": false}"#);
    let Local::TextInput(i) = decode(root(&t)) else { panic!() };
    let sent = i.submitted("roda os testes").unwrap();
    assert_eq!(sent.get("text"), Some(&Value::from("roda os testes")));
    assert_eq!(sent.get("id"), Some(&Value::from("chat")));
    assert_eq!(i.submitted("   "), None);
    let mut seen = ClearSeen::default();
    assert!(!seen.should_clear("chat", Some(0)));
    assert!(!seen.should_clear("chat", Some(0)));
    assert!(seen.should_clear("chat", Some(1)));
    assert!(!seen.should_clear("chat", Some(1)));
    assert!(!seen.should_clear("outro", Some(7)));
}

#[test]
fn in2_a_locked_field_shows_the_hint_and_sends_nothing() {
    let t = input(r#"{"sent": 0, "locked": true}"#);
    let Local::TextInput(i) = decode(root(&t)) else { panic!() };
    assert_eq!(i.shown_hint(), Some("responda no terminal"));
    assert_eq!(i.submitted("sim"), None);
    assert!(i.matches("/c", None).is_empty());
    let open = input(r#"{"sent": 0, "locked": false}"#);
    let Local::TextInput(o) = decode(root(&open)) else { panic!() };
    assert_eq!(o.shown_hint(), Some("fale"));
}

#[test]
fn in3_allow_empty_lets_enter_send_an_empty_text() {
    let t = input(r#"{"empty": false}"#);
    let Local::TextInput(i) = decode(root(&t)) else { panic!() };
    assert_eq!(i.submitted(""), None);
    assert_eq!(i.image_extra().get("id"), Some(&Value::from("chat")));
    let t = input(r#"{"empty": true}"#);
    let Local::TextInput(i) = decode(root(&t)) else { panic!() };
    assert_eq!(i.submitted("").unwrap().get("text"), Some(&Value::from("")));
}

#[test]
fn in4_arrows_go_to_the_plugin_only_with_an_empty_field() {
    let t = input("{}");
    let Local::TextInput(i) = decode(root(&t)) else { panic!() };
    let keys: Vec<Value> =
        ["down", "down", "up"].iter().map(|k| i.arrow(k, "").unwrap().get("key").unwrap().clone()).collect();
    assert_eq!(keys, [Value::from("down"), Value::from("down"), Value::from("up")]);
    assert_eq!(i.arrow("down", "oi"), None);
    assert_eq!(i.arrow("left", ""), None);
}

#[test]
fn in5_slash_commands_filter_starts_first_and_esc_dismisses_until_the_text_changes() {
    let t = input("{}");
    let Local::TextInput(i) = decode(root(&t)) else { panic!() };
    let names = |text: &str, dismissed: Option<&str>| -> Vec<String> {
        i.matches(text, dismissed).iter().map(|c| c.name.clone()).collect()
    };
    assert_eq!(names("/co", None), ["compact", "context"]);
    assert_eq!(names("/C", None), ["clear", "compact", "context"]);
    assert_eq!(names("/x", None), ["context"]);
    assert_eq!(names("/ext", None), ["context"]);
    assert!(names("/c", Some("/c")).is_empty());
    assert_eq!(names("/cl", Some("/c")), ["clear"]);
    assert!(names("/compact x", None).is_empty());
    assert!(names("co", None).is_empty());
    assert_eq!(names("/", None).len(), 3);
    assert_eq!(completed(&i.commands[2]), "/context ");
}

#[test]
fn slash_list_shows_at_most_eight() {
    let cmds: Vec<String> = (0..12).map(|n| format!("{{ name: \"c{n}\" }}")).collect();
    let t = tree(&format!("widget root = TextInput(commands: [{}]);", cmds.join(", ")), "{}");
    let Local::TextInput(i) = decode(root(&t)) else { panic!() };
    assert_eq!(i.matches("/c", None).len(), 8);
}

#[test]
fn md1_markdown_reads_text_and_font_size() {
    let t = tree(r#"widget root = Markdown(text: "Pronto, o **login** voltou.\n\n- um\n- dois");"#, "{}");
    let Local::Markdown(m) = decode(root(&t)) else { panic!("not Markdown") };
    assert!(m.text.contains("**login**"));
    assert_eq!(m.font_size, MARKDOWN_BASE_SIZE);
    let t = tree(r#"widget root = Markdown(text: "x", fontSize: 15.0);"#, "{}");
    let Local::Markdown(m) = decode(root(&t)) else { panic!() };
    assert_eq!(m.font_size, 15.0);
}

#[test]
fn selectable_selection_lines_and_anchor_wrap_one_child() {
    let t = tree(
        r#"widget root = Selectable(child: SelectionLines(child: Anchor(id: "agente:1", child: Text(text: "oi"))));"#,
        "{}",
    );
    let Local::Selectable(s) = decode(root(&t)) else { panic!() };
    assert!(!s.lines_only);
    let Local::SelectionLines(l) = decode(root(s.child.unwrap())) else { panic!() };
    assert!(l.lines_only);
    let Local::Anchor(a) = decode(root(l.child.unwrap())) else { panic!() };
    assert_eq!(a.id, "agente:1");
    assert_eq!(a.key("wiboor"), "wiboor/agente:1");
    assert_eq!(anchor_key("p", "x"), "p/x");
    let bad = tree(r#"widget root = Anchor(id: "a");"#, "{}");
    let Local::Anchor(a) = decode(root(&bad)) else { panic!() };
    assert!(a.child.is_err());
}

#[test]
fn zoom_view_args_follow_dart() {
    let t = tree(
        r#"widget root = ZoomView(maxScale: 0.5, command: "in", commandRev: 2.0, onZoom: event "z" {}, child: Text(text: "x"));"#,
        "{}",
    );
    let Local::ZoomView(z) = decode(root(&t)) else { panic!() };
    assert_eq!((z.max_scale, z.command.as_deref(), z.command_rev), (1.0, Some("in"), 2));
    assert!(z.child.is_some() && z.on_zoom.len() == 1);
    let t = tree("widget root = ZoomView();", "{}");
    let Local::ZoomView(ZoomView { max_scale, command_rev, child, .. }) = decode(root(&t)) else { panic!() };
    assert_eq!((max_scale, command_rev, child.is_none()), (6.0, 0, true));
}

#[test]
fn zm1_a_wheel_notch_up_zooms_in_around_the_cursor() {
    let size = (400.0, 200.0);
    let m = zoom_around(Transform::IDENTITY, (200.0, 100.0), wheel_factor(-100.0), 4.0, size);
    assert!(m.scale > 1.2, "{m:?}");
    assert!((m.x + 200.0 * (m.scale - 1.0)).abs() < 1e-9 && (m.y + 100.0 * (m.scale - 1.0)).abs() < 1e-9);
    assert!(should_report(m.scale, 1.0));
    assert!(!should_report(1.004, 1.0));
}

#[test]
fn zm2_zooming_out_never_goes_below_the_whole_content() {
    let m = zoom_around(Transform::IDENTITY, (10.0, 10.0), wheel_factor(300.0), 4.0, (400.0, 200.0));
    assert_eq!(m, Transform::IDENTITY);
}

#[test]
fn zm3_commands_step_by_one_and_a_half_clamp_and_fit_back_to_one() {
    let size = (400.0, 200.0);
    let mut m = Transform::IDENTITY;
    let mut scales = Vec::new();
    for _ in 0..7 {
        let to = zoom_target(Some("in"), m.scale, 4.0).unwrap();
        m = command_transform(m, size, to);
        scales.push(m.scale);
    }
    assert_eq!(scales[0], 1.5);
    assert_eq!(*scales.last().unwrap(), 4.0);
    let to = zoom_target(Some("fit"), m.scale, 4.0).unwrap();
    assert_eq!(command_transform(m, size, to), Transform::IDENTITY);
    assert_eq!(zoom_target(Some("out"), 1.2, 4.0), Some(1.0));
    assert_eq!(zoom_target(Some("nada"), 1.0, 4.0), None);
    assert_eq!(zoom_target(None, 1.0, 4.0), None);
}

#[test]
fn the_command_keeps_the_centre_of_the_view_on_the_same_content_point() {
    let size = (400.0, 200.0);
    let start = clamp_translation(Transform { scale: 2.0, x: -100.0, y: -50.0 }, size);
    let centre = start.to_scene((200.0, 100.0));
    let next = command_transform(start, size, 3.0);
    let after = next.to_scene((200.0, 100.0));
    assert!((centre.0 - after.0).abs() < 1e-9 && (centre.1 - after.1).abs() < 1e-9);
}

#[test]
fn translation_is_clamped_so_nothing_outside_the_content_shows() {
    let m = clamp_translation(Transform { scale: 2.0, x: 50.0, y: -500.0 }, (400.0, 200.0));
    assert_eq!((m.x, m.y), (0.0, -200.0));
}

#[test]
fn fitted_box_scale_follows_box_fit() {
    assert_eq!(fitted_scale(BoxFit::Contain, (200.0, 100.0), (400.0, 400.0)), 2.0);
    assert_eq!(fitted_scale(BoxFit::Cover, (200.0, 100.0), (400.0, 400.0)), 4.0);
    assert_eq!(fitted_scale(BoxFit::ScaleDown, (200.0, 100.0), (400.0, 400.0)), 1.0);
    assert_eq!(fitted_scale(BoxFit::Contain, (0.0, 100.0), (400.0, 400.0)), 1.0);
    let t = tree(r#"widget root = FittedBox(child: SizedBox(width: 1.0));"#, "{}");
    let Local::FittedBox(f) = decode(root(&t)) else { panic!() };
    assert_eq!(f.fit, BoxFit::Contain);
}

fn chat_data() -> DynamicContent {
    content(
        r#"{
          "selectable": true, "markdown": true, "sprites": true, "anchors": true,
          "chat": {
            "inputId": "chat:1", "placeholder": "fale com ele", "sent": 0, "commands": [{"name": "compact"}],
            "items": [{"who": "you", "text": "corrige o login"}, {"who": "claude", "text": "Pronto, o **login** voltou."}]
          }
        }"#,
    )
}

#[test]
fn the_escritorio_chat_window_uses_the_new_widgets_without_errors() {
    let lib = parse_library_file(&escritorio("chat.rfwtxt")).unwrap();
    let tree = evaluate(&lib, "chat", &chat_data());
    assert!(problems(&tree).is_empty(), "{:#?}", problems(&tree));
    for name in ["Selectable", "SelectionLines", "Markdown", "TextInput"] {
        assert_eq!(find(&tree, |n| n.name == name).len(), 1, "{name}");
    }
    let input = find(&tree, |n| n.name == "TextInput")[0];
    let Local::TextInput(i) = decode(input) else { panic!() };
    assert_eq!((i.id.as_str(), i.multiline, i.max_lines, i.autofocus), ("chat:1", true, 6, true));
    assert_eq!(i.send_icon.as_deref(), Some("send"));
    assert_eq!(i.commands.len(), 1);
}

#[test]
fn every_escritorio_library_parses() {
    for name in ["chat.rfwtxt", "escritorio.rfwtxt", "sidebar.rfwtxt"] {
        parse_library_file(&escritorio(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn focus_on_whole_child_is_identity() {
    assert_eq!(focus_transform((1000.0, 800.0), (0.0, 0.0, 1.0, 1.0), 6.0), Transform::IDENTITY);
}

#[test]
fn focus_on_quarter_centers_it() {
    let size = (1000.0, 800.0);
    let m = focus_transform(size, (0.5, 0.5, 0.25, 0.25), 6.0);
    assert!((m.scale - 3.6).abs() < 1e-9, "{m:?}");
    let c = (0.625 * size.0 * m.scale + m.x, 0.625 * size.1 * m.scale + m.y);
    assert!((c.0 - 500.0).abs() < 0.5 && (c.1 - 400.0).abs() < 0.5, "{m:?} {c:?}");
}

#[test]
fn focus_respects_max_scale() {
    assert_eq!(focus_transform((1000.0, 800.0), (0.4, 0.4, 0.01, 0.01), 6.0).scale, 6.0);
}

#[test]
fn focus_is_not_a_zoom_target_and_its_rect_is_parsed() {
    assert_eq!(zoom_target(Some("focus"), 1.0, 6.0), None);
    let t = tree(
        r#"widget root = ZoomView(command: "focus", focus: {x: 0.5, y: 0.25, w: 0.25, h: 0.5}, onMove: event "m" {}, child: Text(text: "x"));"#,
        "{}",
    );
    let Local::ZoomView(z) = decode(root(&t)) else { panic!() };
    assert_eq!(z.focus, Some((0.5, 0.25, 0.25, 0.5)));
    assert_eq!(z.on_move.len(), 1);
    let t = tree("widget root = ZoomView();", "{}");
    let Local::ZoomView(z) = decode(root(&t)) else { panic!() };
    assert_eq!(z.focus, None);
}

#[test]
fn focus_is_relative_to_the_fitted_content_not_the_viewport() {
    let view = (1000.0, 800.0);
    let placed = fitted_placement(BoxFit::Contain, (0.0, 0.0), (2000.0, 500.0), view);
    assert_eq!(placed, (0.0, 0.34375, 1.0, 0.3125));
    let rect = focus_in_placement((0.5, 0.0, 0.5, 1.0), placed);
    let m = focus_transform(view, rect, 6.0);
    // The content's right half spans x 500..1000, y 275..525 in the viewport (centre y 400).
    // At the right edge the x translation clamps, so the half is fully visible but not centred in x.
    assert!((m.scale - 1.8).abs() < 1e-9, "{m:?}");
    assert!((400.0 * m.scale + m.y - 400.0).abs() < 0.5, "{m:?}");
    let (left, right) = (500.0 * m.scale + m.x, 1000.0 * m.scale + m.x);
    assert!(left >= -0.5 && right <= 1000.5, "{m:?} {left} {right}");
    // A rect that can be centred is: the left quarter of the content.
    let q = focus_transform(view, focus_in_placement((0.375, 0.0, 0.25, 1.0), placed), 6.0);
    assert!(((500.0 * q.scale + q.x) - 500.0).abs() < 0.5 && ((400.0 * q.scale + q.y) - 400.0).abs() < 0.5, "{q:?}");
}
