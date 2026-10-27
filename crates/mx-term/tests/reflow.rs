use mx_term::{GridSize, SelectionKind, TerminalSession};
use mx_theme::TerminalPalette;

const WIDTH: u16 = 40;

fn claude(rows: &[&str]) -> TerminalSession {
    let size = GridSize { cols: WIDTH, rows: 12, cell_width: 8, cell_height: 16 };
    let t = TerminalSession::new(size, TerminalPalette::default(), 100);
    t.feed(format!("\x1b[H● {}", rows[0]).as_bytes());
    for row in &rows[1..] {
        let rest = if row.is_empty() { String::new() } else { format!("\x1b[2C{row}") };
        t.feed(format!("\r\n{rest}").as_bytes());
    }
    t
}

fn select(t: &TerminalSession, from: usize, line: usize, last_line: usize) {
    t.start_selection(from, line, false, SelectionKind::Cells);
    t.update_selection(usize::from(WIDTH) - 1, last_line, true);
}

fn reflowed(t: &TerminalSession, from: usize, last_line: usize) -> String {
    select(t, from, 0, last_line);
    t.selection_text(true).unwrap_or_default()
}

const RIVERS: [&str; 2] = ["Rivers shape landscapes and sustain", "ecosystems across the whole globe."];
const ITEM: [&str; 4] = ["Rivers shape landscapes and sustain", "ecosystems.", "", "- An item."];

#[test]
fn the_rows_claude_broke_join_into_one_line_without_the_indent_or_the_bullet() {
    let t = claude(&RIVERS);
    assert_eq!(reflowed(&t, 0, 1), "Rivers shape landscapes and sustain ecosystems across the whole globe.");
}

#[test]
fn and_without_reflow_they_stay_as_the_screen_shows_them() {
    let t = claude(&RIVERS);
    select(&t, 0, 0, 1);
    assert_eq!(
        t.selection_text(false).as_deref(),
        Some("● Rivers shape landscapes and sustain\n  ecosystems across the whole globe.")
    );
    assert_eq!(t.selected_text(), t.selection_text(false));
}

#[test]
fn a_row_that_ends_early_ends_a_line() {
    let t = claude(&["First paragraph.", "", "Second one."]);
    assert_eq!(reflowed(&t, 0, 2), "First paragraph.\n\nSecond one.");
}

#[test]
fn a_row_whose_next_word_would_have_fit_was_a_line_of_its_own() {
    let t = claude(&["Short line", "x"]);
    assert_eq!(reflowed(&t, 0, 1), "Short line\nx");
}

#[test]
fn list_items_stay_apart_each_one_whole() {
    let t = claude(&["Two items:", "", "- The first item is long enough to", "  wrap.", "- The second."]);
    assert_eq!(reflowed(&t, 0, 4), "Two items:\n\n- The first item is long enough to wrap.\n- The second.");
}

#[test]
fn a_selection_that_starts_on_a_word_keeps_the_rest_dedented() {
    let t = claude(&ITEM);
    assert_eq!(reflowed(&t, 9, 3), "shape landscapes and sustain ecosystems.\n\n- An item.");
}

#[test]
fn a_word_longer_than_the_row_joins_without_a_space() {
    let t = claude(&["https://example.com/a/very/long/path/x", "yz"]);
    assert_eq!(reflowed(&t, 0, 1), "https://example.com/a/very/long/path/xyz");
}

#[test]
fn a_row_that_opens_with_a_number_is_still_the_middle_of_a_sentence() {
    let t = claude(&["The release shipped in the spring of", "2026. After that the whole team had", "moved on."]);
    assert_eq!(
        reflowed(&t, 0, 2),
        "The release shipped in the spring of 2026. After that the whole team had moved on."
    );
}

#[test]
fn a_selection_across_two_replies_loses_both_bullets() {
    let t = claude(&["First reply.", "", "\x1b[1G● Second reply.", "  - item"]);
    assert_eq!(reflowed(&t, 0, 3), "First reply.\n\nSecond reply.\n- item");
}

#[test]
fn a_selection_that_starts_on_the_space_between_two_words_is_not_an_indent() {
    let t = claude(&ITEM);
    assert_eq!(reflowed(&t, 8, 3), " shape landscapes and sustain ecosystems.\n\n- An item.");
}

#[test]
fn a_table_is_never_prose() {
    let t = claude(&[
        "┌──────────────────────────────────┐",
        "│ a long table row that fills it  │",
        "└──────────────────────────────────┘",
    ]);
    assert_eq!(reflowed(&t, 0, 2).split('\n').count(), 3);
}

#[test]
fn the_columns_a_tui_jumped_over_are_the_spaces_they_look_like() {
    let t = claude(&[""]);
    t.feed(b"\x1b[H\x1b[2K\x1b[1GTip:\x1b[10Gwrite\x1b[20Gcode");
    select(&t, 0, 0, 0);
    assert_eq!(t.selection_text(false).as_deref(), Some("Tip:     write     code"));
}

#[test]
fn a_wide_character_does_not_drag_a_space_behind_it() {
    let t = claude(&[""]);
    t.feed("\x1b[H\x1b[2K日本 ok".as_bytes());
    select(&t, 0, 0, 0);
    assert_eq!(t.selection_text(true).as_deref(), Some("日本 ok"));
}

#[test]
fn nothing_selected_copies_nothing() {
    let t = claude(&RIVERS);
    assert_eq!(t.selection_text(true), None);
}
