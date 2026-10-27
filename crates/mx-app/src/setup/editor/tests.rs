use gpui::{ClipboardItem, Entity, Modifiers, TestAppContext, VisualTestContext, point, px, size};

use super::TextEditor;

fn editor<'a>(cx: &'a mut TestAppContext, text: &str) -> (Entity<TextEditor>, &'a mut VisualTestContext) {
    let text = text.to_owned();
    let (e, cx) = cx.add_window_view(move |_, cx| TextEditor::new(text, cx));
    cx.simulate_resize(size(px(600.), px(400.)));
    cx.update(|window, cx| e.read(cx).focus(window));
    cx.run_until_parked();
    (e, cx)
}

fn text(e: &Entity<TextEditor>, cx: &mut VisualTestContext) -> String {
    e.read_with(cx, |e, _| e.text().to_owned())
}

#[gpui::test]
fn shift_arrows_select_and_typing_replaces_the_selection(cx: &mut TestAppContext) {
    let (e, cx) = editor(cx, "hello");
    cx.simulate_keystrokes("end shift-left shift-left");
    assert_eq!(e.read_with(cx, |e, _| e.buffer().selected_text().to_owned()), "lo");
    cx.simulate_input("p!");
    assert_eq!(text(&e, cx), "help!");
}

#[gpui::test]
fn copy_cut_and_paste_go_through_the_clipboard(cx: &mut TestAppContext) {
    let (e, cx) = editor(cx, "one two");
    cx.simulate_keystrokes("ctrl-a ctrl-c");
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("one two"));
    cx.simulate_keystrokes("ctrl-end ctrl-shift-left ctrl-x");
    assert_eq!(text(&e, cx), "one ");
    cx.write_to_clipboard(ClipboardItem::new_string("a\r\nb".into()));
    cx.simulate_keystrokes("ctrl-v");
    assert_eq!(text(&e, cx), "one a\nb");
}

#[gpui::test]
fn undo_and_redo_walk_the_history(cx: &mut TestAppContext) {
    let (e, cx) = editor(cx, "");
    cx.simulate_input("abc");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("d");
    cx.simulate_keystrokes("ctrl-z");
    assert_eq!(text(&e, cx), "abc\n");
    cx.simulate_keystrokes("ctrl-z ctrl-z");
    assert_eq!(text(&e, cx), "");
    cx.simulate_keystrokes("ctrl-shift-z");
    assert_eq!(text(&e, cx), "abc");
}

#[gpui::test]
fn up_and_down_move_by_rows_keeping_the_column(cx: &mut TestAppContext) {
    let (e, cx) = editor(cx, "abcdef\nxy\n123456");
    cx.simulate_keystrokes("right right right right down");
    assert_eq!(e.read_with(cx, |e, _| e.buffer().head()), 9);
    cx.simulate_keystrokes("down");
    assert_eq!(e.read_with(cx, |e, _| e.buffer().head()), 14);
    cx.simulate_keystrokes("up up");
    assert_eq!(e.read_with(cx, |e, _| e.buffer().head()), 4);
}

#[gpui::test]
fn backspace_word_and_delete_edit_around_the_cursor(cx: &mut TestAppContext) {
    let (e, cx) = editor(cx, "foo bar");
    cx.simulate_keystrokes("end ctrl-backspace");
    assert_eq!(text(&e, cx), "foo ");
    cx.simulate_keystrokes("home delete");
    assert_eq!(text(&e, cx), "oo ");
    cx.simulate_keystrokes("tab");
    assert_eq!(text(&e, cx), "  oo ");
}

#[gpui::test]
fn clicking_places_the_cursor_and_double_click_selects_a_word(cx: &mut TestAppContext) {
    let (e, cx) = editor(cx, "alpha beta\ngamma");
    let at = e.read_with(cx, |e, _| {
        let l = e.layout.as_ref().unwrap();
        e.to_window(point(l.position(8).x + px(1.), l.line_height * 0.5))
    });
    cx.simulate_click(at, Modifiers::none());
    assert_eq!(e.read_with(cx, |e, _| e.buffer().head()), 8);
    e.update_in(cx, |e, w, cx| e.mouse_down(at, 2, false, w, cx));
    assert_eq!(e.read_with(cx, |e, _| e.buffer().selected_text().to_owned()), "beta");
    let below = e.read_with(cx, |e, _| {
        let l = e.layout.as_ref().unwrap();
        e.to_window(point(px(0.), l.line_height * 1.5))
    });
    e.update_in(cx, |e, w, cx| e.mouse_down(below, 1, true, w, cx));
    assert_eq!(e.read_with(cx, |e, _| e.buffer().selection()), 6..11);
}

#[gpui::test]
fn a_long_line_wraps_into_several_rows(cx: &mut TestAppContext) {
    let long = "palavra ".repeat(60);
    let (e, cx) = editor(cx, &long);
    let (rows, height) = e.read_with(cx, |e, _| {
        let l = e.layout.as_ref().unwrap();
        (l.rows.len(), l.height / l.line_height)
    });
    assert_eq!(rows, 1);
    assert!(height >= 3.);
}
