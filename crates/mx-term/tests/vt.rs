use mx_term::{GridSize, SelectionKind, TerminalSession};
use mx_theme::TerminalPalette;

fn term(cols: u16, rows: u16, scrollback: usize) -> TerminalSession {
    let size = GridSize { cols, rows, cell_width: 8, cell_height: 16 };
    TerminalSession::new(size, TerminalPalette::default(), scrollback)
}

fn lines(t: &TerminalSession, n: usize, prefix: &str) {
    for i in 0..n {
        t.feed(format!("{prefix}{i}\r\n").as_bytes());
    }
}

fn select_row(t: &TerminalSession, row: usize) -> Option<String> {
    let cols = t.size().cols as usize;
    t.start_selection(0, row, false, SelectionKind::Cells);
    t.update_selection(cols - 1, row, true);
    t.selected_text()
}

#[test]
fn sgr_22_clears_bold() {
    let t = term(80, 10, 100);
    t.feed(b"\x1b[1mA\x1b[22mB");
    let snap = t.snapshot();
    assert!(snap.row(0)[0].style.bold);
    assert!(!snap.row(0)[1].style.bold);
}

#[test]
fn sgr_22_clears_faint() {
    let t = term(80, 10, 100);
    t.feed(b"\x1b[2mA\x1b[22mB");
    let snap = t.snapshot();
    assert_ne!(snap.row(0)[0].style.fg, snap.foreground);
    assert_eq!(snap.row(0)[1].style.fg, snap.foreground);
}

#[test]
fn leaving_alt_screen_resets_attributes() {
    let t = term(80, 10, 100);
    t.feed(b"\x1b[?1049h\x1b[4mTUI\x1b[?1049lA");
    let snap = t.snapshot();
    let a = snap.row(0).iter().find(|c| c.c == 'A').unwrap();
    assert!(!a.style.underline);
}

#[test]
fn private_sgr_is_scrubbed_before_the_emulator() {
    let t = term(80, 10, 100);
    t.feed(b"\x1b[>4;2mABC");
    let snap = t.snapshot();
    for cell in &snap.row(0)[..3] {
        assert!(!cell.style.underline && !cell.style.bold);
        assert_eq!(cell.style.fg, snap.foreground);
    }
    assert_eq!(t.row_text(0), "ABC");
}

#[test]
fn clear_scrollback_keeps_rows_addressable() {
    let t = term(80, 10, 100);
    lines(&t, 30, "linha ");
    t.feed(b"\x1b[3J");
    assert_eq!(select_row(&t, 2), Some(t.row_text(2)));
    assert_eq!(select_row(&t, 8), Some(t.row_text(8)));
    assert_eq!(t.row_text(8), "linha 29");
}

#[test]
fn selection_made_before_clear_scrollback_still_points_at_its_row() {
    let t = term(80, 10, 100);
    lines(&t, 30, "linha ");
    let row = 8;
    let before = select_row(&t, row);
    t.feed(b"\x1b[3J");
    assert_eq!(t.selected_text(), before);
    assert_eq!(before, Some(t.row_text(row)));
}

#[test]
fn alt_screen_resize_keeps_links_on_their_rows() {
    let t = term(80, 10, 100);
    t.feed(b"\x1b[?1049h");
    for i in 0..20 {
        t.feed(format!("https://x.dev/{i}\r\n").as_bytes());
    }
    t.resize(GridSize { cols: 80, rows: 6, cell_width: 8, cell_height: 16 });
    let row = t.row_text(1);
    assert!(row.starts_with("https://x.dev/"));
    assert_eq!(t.link_at(3, 1, None), Some(row));
}

#[test]
fn drag_selection_after_clear_scrollback_copies_that_row() {
    let t = term(90, 20, 100);
    lines(&t, 40, "conteudo da linha ");
    t.feed(b"\x1b[3J");
    let text = t.row_text(3);
    t.start_selection(0, 3, false, SelectionKind::Cells);
    t.update_selection(text.chars().count() - 1, 3, true);
    assert_eq!(t.selected_text(), Some(text));
}

#[test]
fn drag_selection_over_a_row_copies_it() {
    let t = term(90, 20, 100);
    lines(&t, 40, "conteudo da linha ");
    let row = 20 - 3;
    let text = t.row_text(row);
    t.start_selection(0, row, false, SelectionKind::Cells);
    t.update_selection(text.chars().count() - 1, row, true);
    assert_eq!(t.selected_text(), Some(text));
}

#[test]
fn copy_turns_blank_cells_into_spaces_and_trims_the_end() {
    let t = term(40, 5, 100);
    t.feed(b"a\x1b[5Gb");
    assert_eq!(select_row(&t, 0).as_deref(), Some("a   b"));
}

fn osc8(url: &str, label: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{label}\x1b]8;;\x1b\\")
}

#[test]
fn osc8_link_is_found_only_on_its_label() {
    let t = term(80, 10, 100);
    t.feed(format!("abra {} agora", osc8("https://x.dev/auth", "aqui")).as_bytes());
    assert_eq!(t.link_at(5, 0, None).as_deref(), Some("https://x.dev/auth"));
    assert_eq!(t.link_at(8, 0, None).as_deref(), Some("https://x.dev/auth"));
    assert_eq!(t.link_at(4, 0, None), None);
    assert_eq!(t.link_at(9, 0, None), None);
    assert_eq!(t.row_text(0), "abra aqui agora");
}

#[test]
fn osc8_label_wrapping_rows_is_one_link() {
    let t = term(20, 5, 100);
    let url = "https://x.dev/longo";
    t.feed(format!("leia {}!", osc8(url, "um rótulo bem comprido")).as_bytes());
    assert_eq!(t.link_at(6, 0, None).as_deref(), Some(url));
    assert_eq!(t.link_at(3, 1, None).as_deref(), Some(url));
    assert_eq!(t.link_at(7, 1, None), None);
}

#[test]
fn osc8_file_url_becomes_a_path() {
    let t = term(80, 5, 100);
    t.feed(osc8("file:///tmp/dir/PLANO.md", "plano").as_bytes());
    assert_eq!(t.link_at(2, 0, None).as_deref(), Some("/tmp/dir/PLANO.md"));
    let t = term(80, 5, 100);
    t.feed(osc8("file://outra-maquina/etc/hosts", "hosts").as_bytes());
    assert_eq!(t.link_at(2, 0, None).as_deref(), Some("file://outra-maquina/etc/hosts"));
}

#[test]
fn osc8_overwritten_label_is_no_longer_a_link() {
    let t = term(80, 5, 100);
    t.feed(format!("{}\r\x1b[2Koutra coisa", osc8("https://x.dev", "aqui")).as_bytes());
    assert_eq!(t.link_at(1, 0, None), None);
}

#[test]
fn osc8_scrolled_out_of_history_is_gone() {
    let t = term(20, 4, 30);
    t.feed(osc8("https://x.dev", "link").as_bytes());
    lines(&t, 40, "l");
    for y in 0..4 {
        assert_eq!(t.link_at(1, y, None), None);
    }
}

#[test]
fn osc8_on_last_line_survives_clear_scrollback() {
    let t = term(40, 6, 100);
    lines(&t, 20, "l");
    t.feed(format!("xxxxx{}", osc8("https://x.dev/z", "ultimo")).as_bytes());
    t.feed(b"\x1b[3J");
    assert_eq!(t.link_at(5, 5, None).as_deref(), Some("https://x.dev/z"));
}

#[test]
fn osc8_consecutive_links_keep_their_own_urls() {
    let t = term(80, 5, 100);
    t.feed(b"\x1b]8;;https://a\x1b\\um\x1b]8;;https://b\x1b\\dois\x1b]8;;\x1b\\");
    assert_eq!(t.link_at(0, 0, None).as_deref(), Some("https://a"));
    assert_eq!(t.link_at(3, 0, None).as_deref(), Some("https://b"));
}

#[test]
fn osc8_opened_and_closed_immediately_links_nothing() {
    let t = term(80, 5, 100);
    t.feed(b"\x1b]8;;https://c\x1b\\\x1b]8;;\x1b\\\r\ntexto");
    assert_eq!(t.link_at(0, 1, None), None);
}

#[test]
fn osc8_uri_may_contain_semicolons() {
    let t = term(80, 5, 100);
    t.feed(osc8("https://x.dev/a?b=1;c=2", "ab").as_bytes());
    assert_eq!(t.link_at(1, 0, None).as_deref(), Some("https://x.dev/a?b=1;c=2"));
}

#[test]
fn heuristic_link_on_second_line_of_buffer() {
    let t = term(80, 5, 100);
    t.feed(b"rode isto:\r\nabra https://x.dev/auth em outra aba");
    assert_eq!(t.link_at(10, 1, None).as_deref(), Some("https://x.dev/auth"));
    assert_eq!(t.link_at(3, 0, None), None);
}

#[test]
fn wrapped_url_is_found_from_both_rows() {
    let t = term(40, 5, 100);
    let url = "https://example.com/um/caminho/bem/comprido/que/quebra";
    t.feed(format!("veja {url}").as_bytes());
    assert_eq!(t.link_at(5, 1, None).as_deref(), Some(url));
    assert_eq!(t.link_at(10, 0, None).as_deref(), Some(url));
}

#[test]
fn link_lookup_outside_the_viewport_is_none() {
    let t = term(40, 5, 100);
    t.feed(b"https://x.dev/a");
    assert_eq!(t.link_at(0, 99, None), None);
    assert_eq!(t.link_at(99, 0, None), None);
}

#[test]
fn clicking_a_bare_markdown_name_resolves_against_cwd() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("PLANO.md"), "x").unwrap();
    let t = term(80, 5, 100);
    t.feed(b"escrevi PLANO.md agora");
    let href = t.link_at(8, 0, dir.path().to_str()).unwrap();
    let want = format!("{}/PLANO.md", dir.path().display());
    assert_eq!(mx_term::follow_link(&href, dir.path().to_str()), mx_term::LinkFollow::Reader(want));
    assert_eq!(t.link_at(2, 0, dir.path().to_str()), None);
}
