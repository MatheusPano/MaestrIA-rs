use super::Buffer;

fn at(text: &str, cursor: usize) -> Buffer {
    let mut b = Buffer::new(text);
    b.set_cursor(cursor, false);
    b
}

#[test]
fn typing_replaces_the_selection_and_leaves_the_cursor_after_it() {
    let mut b = Buffer::new("hello world");
    b.select(0..5);
    b.insert("bye");
    assert_eq!(b.text(), "bye world");
    assert_eq!(b.selection(), 3..3);
}

#[test]
fn backspace_and_delete_remove_whole_characters() {
    let mut b = at("ação", 5);
    b.backspace(false);
    assert_eq!(b.text(), "aço");
    b.set_cursor(0, false);
    b.delete(false);
    assert_eq!(b.text(), "ço");
}

#[test]
fn word_erase_stops_at_class_changes() {
    let mut b = at("foo.bar baz  ", 13);
    b.backspace(true);
    assert_eq!(b.text(), "foo.bar ");
    b.backspace(true);
    assert_eq!(b.text(), "foo.");
    b.set_cursor(0, false);
    b.delete(true);
    assert_eq!(b.text(), ".");
}

#[test]
fn consecutive_typing_undoes_as_one_step_and_redo_brings_it_back() {
    let mut b = Buffer::new("");
    for c in ["a", "b", "c"] {
        b.insert(c);
    }
    b.insert(" ");
    b.insert("d");
    assert!(b.undo());
    assert_eq!(b.text(), "abc ");
    assert!(b.undo());
    assert_eq!(b.text(), "abc");
    assert!(b.undo());
    assert_eq!(b.text(), "");
    assert!(!b.undo());
    assert!(b.redo());
    assert_eq!(b.text(), "abc");
}

#[test]
fn moving_the_cursor_breaks_the_undo_group() {
    let mut b = Buffer::new("");
    b.insert("a");
    b.set_cursor(0, false);
    b.insert("b");
    b.undo();
    assert_eq!(b.text(), "a");
}

#[test]
fn cut_returns_the_selection_and_removes_it() {
    let mut b = Buffer::new("one two");
    b.select(4..7);
    assert_eq!(b.cut().as_deref(), Some("two"));
    assert_eq!(b.text(), "one ");
    assert_eq!(Buffer::new("x").cut(), None);
}

#[test]
fn vertical_moves_keep_the_column_and_clamp_to_short_lines() {
    let b = at("abcdef\nxy\n123456", 4);
    let down = b.vertical(4, true);
    assert_eq!(down, 9);
    assert_eq!(b.vertical(down, true), 12);
    assert_eq!(b.vertical(1, false), 0);
    assert_eq!(b.vertical(14, true), 16);
}

#[test]
fn smart_home_toggles_between_indent_and_column_zero() {
    let b = Buffer::new("x\n    let a");
    assert_eq!(b.smart_home(11), 6);
    assert_eq!(b.smart_home(6), 2);
    assert_eq!(b.line_at(4), 2..11);
    assert_eq!(b.line_at(0), 0..2);
}

#[test]
fn word_at_selects_the_run_under_the_point() {
    let b = Buffer::new("name: release-app");
    assert_eq!(b.word_at(1), 0..4);
    assert_eq!(b.word_at(8), 6..13);
}

#[test]
fn utf16_offsets_round_trip_through_astral_characters() {
    let b = Buffer::new("a😀é\nb");
    assert_eq!(b.to_utf16(b.text().len()), 6);
    assert_eq!(b.offset_of_utf16(3), 5);
    assert_eq!(b.range_from_utf16(&(1..4)), 1..7);
    assert_eq!(b.range_to_utf16(&(1..7)), 1..4);
}

#[test]
fn ime_composition_marks_then_commits() {
    let mut b = Buffer::new("x");
    b.set_cursor(1, false);
    b.replace_and_mark(1..1, "´", None);
    assert_eq!(b.marked(), Some(1..3));
    b.insert("é");
    assert_eq!(b.text(), "xé");
    assert_eq!(b.marked(), None);
}

#[test]
fn reset_bumps_the_version_and_clears_history() {
    let mut b = Buffer::new("a");
    b.insert("b");
    let v = b.version();
    b.reset("new");
    assert!(b.version() > v);
    assert!(!b.undo());
    assert_eq!(b.selection(), 0..0);
}
