use gpui::{Entity, TestAppContext, VisualTestContext, px, size};

use super::comment::{comment_style, toggle_comment};
use super::fold::chunks;
use super::syntax::highlight;
use super::{EditorStyle, Gutter, TextEditor};

fn toggled(text: &str, sel: std::ops::Range<usize>, lang: &str) -> (String, std::ops::Range<usize>) {
    let edit = toggle_comment(text, sel, comment_style(lang).unwrap()).unwrap();
    let mut out = text.to_owned();
    out.replace_range(edit.range, &edit.text);
    (out, edit.selection)
}

#[test]
fn slash_comments_lines_at_the_shallowest_indent_and_back() {
    let (on, sel) = toggled("  a\n    b\n", 0..9, "javascript");
    assert_eq!(on, "  // a\n  //   b\n");
    let (off, _) = toggled(&on, sel, "javascript");
    assert_eq!(off, "  a\n    b\n");
}

#[test]
fn a_collapsed_cursor_comments_its_own_line_and_keeps_its_column() {
    let (on, sel) = toggled("x = 1\ny = 2\n", 8..8, "python");
    assert_eq!(on, "x = 1\n# y = 2\n");
    assert_eq!(sel, 10..10);
}

#[test]
fn blank_lines_are_left_alone_and_a_mixed_block_gets_commented() {
    let (on, _) = toggled("-- a\n\nb\n", 0..8, "sql");
    assert_eq!(on, "-- -- a\n\n-- b\n");
}

#[test]
fn markup_and_css_wrap_the_line_in_a_block_comment() {
    let (on, _) = toggled("  <b>x</b>\n", 3..3, "xml");
    assert_eq!(on, "  <!-- <b>x</b> -->\n");
    let (off, _) = toggled(&on, 4..4, "xml");
    assert_eq!(off, "  <b>x</b>\n");
    let (css, _) = toggled("a { }", 0..5, "css");
    assert_eq!(css, "/* a { } */");
}

#[test]
fn plaintext_and_diff_have_no_comment_and_unknown_names_no_language() {
    assert!(toggle_comment("a", 0..0, comment_style("plaintext").unwrap()).is_none());
    assert!(toggle_comment("a", 0..0, comment_style("diff").unwrap()).is_none());
    assert!(comment_style("klingon").is_none());
}

#[test]
fn chunks_span_brackets_across_lines_but_skip_strings_and_comments() {
    let text = "fn a() {\n  let s = \"{\";\n  // }\n  x\n}\n[1,\n2]\n";
    assert_eq!(chunks(text, &highlight(text, "rust")), [(0, 4)]);
    let list = "x = [\n  1,\n  2,\n]\n";
    assert_eq!(chunks(list, &highlight(list, "python")), [(0, 3)]);
}

fn code_editor<'a>(cx: &'a mut TestAppContext, text: &str, lang: &str) -> (Entity<TextEditor>, &'a mut VisualTestContext) {
    let text = text.to_owned();
    let lang = lang.to_owned();
    let (e, cx) = cx.add_window_view(move |_, cx| {
        let mut e = TextEditor::new(text, cx);
        let style = EditorStyle {
            code: true,
            language: lang,
            gutter: Some(Gutter { color: gpui::white(), current: gpui::white(), font_size: 12., fold: Some(gpui::white()) }),
            ..EditorStyle::default()
        };
        e.set_style(style, cx);
        e
    });
    cx.simulate_resize(size(px(600.), px(400.)));
    cx.update(|window, cx| e.read(cx).focus(window));
    cx.run_until_parked();
    (e, cx)
}

#[gpui::test]
fn ctrl_slash_toggles_the_line_comment(cx: &mut TestAppContext) {
    let (e, cx) = code_editor(cx, "let a = 1;", "rust");
    cx.simulate_keystrokes("ctrl-/");
    assert_eq!(e.read_with(cx, |e, _| e.text().to_owned()), "// let a = 1;");
    cx.simulate_keystrokes("ctrl-/");
    assert_eq!(e.read_with(cx, |e, _| e.text().to_owned()), "let a = 1;");
    cx.simulate_keystrokes("ctrl-/ ctrl-z");
    assert_eq!(e.read_with(cx, |e, _| e.text().to_owned()), "let a = 1;");
}

#[gpui::test]
fn folding_a_chunk_hides_its_inner_lines_and_moving_into_it_unfolds(cx: &mut TestAppContext) {
    let (e, cx) = code_editor(cx, "fn a() {\n  x;\n  y;\n}\nz\n", "rust");
    let folded = e.update(cx, |e, cx| e.toggle_fold(0, cx));
    assert!(folded);
    cx.run_until_parked();
    assert_eq!(e.read_with(cx, |e, _| e.layout.as_ref().map(|l| l.rows.len())), Some(4));
    assert_eq!(e.read_with(cx, |e, _| e.folded_lines()), [0]);
    e.update(cx, |e, cx| e.place(1, 0, false, cx));
    cx.run_until_parked();
    assert!(e.read_with(cx, |e, _| e.folded_lines()).is_empty());
    assert_eq!(e.read_with(cx, |e, _| e.layout.as_ref().map(|l| l.rows.len())), Some(6));
}

#[gpui::test]
fn the_gutter_click_on_a_chunk_start_folds_it(cx: &mut TestAppContext) {
    let (e, cx) = code_editor(cx, "{\n1\n2\n}\n", "json");
    let (origin, numbers, top) = e.read_with(cx, |e, _| {
        let l = e.layout.as_ref().unwrap();
        (e.bounds.origin, l.numbers, e.style.pad.top)
    });
    let at = gpui::point(origin.x + numbers + px(9.), origin.y + px(top + 4.));
    cx.simulate_click(at, gpui::Modifiers::none());
    assert_eq!(e.read_with(cx, |e, _| e.folded_lines()), [0]);
}
