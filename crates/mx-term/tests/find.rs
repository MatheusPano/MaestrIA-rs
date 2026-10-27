use mx_term::{FindSession, GridSize, Hit, TerminalSession, Viewport};
use mx_theme::TerminalPalette;

fn term(cols: u16, text: &str) -> TerminalSession {
    let size = GridSize { cols, rows: 6, cell_width: 8, cell_height: 16 };
    let t = TerminalSession::new(size, TerminalPalette::default(), 100);
    t.feed(text.as_bytes());
    t
}

fn hits(t: &TerminalSession, query: &str) -> Vec<Hit> {
    t.find(query, 1000)
}

#[test]
fn finds_every_occurrence_with_its_cells() {
    let t = term(40, "erro aqui\r\nnada\r\noutro erro");
    assert_eq!(hits(&t, "erro"), [Hit::new((0, 0), (4, 0)), Hit::new((6, 2), (10, 2))]);
}

#[test]
fn case_only_counts_when_the_query_has_a_capital() {
    let t = term(40, "Erro ERRO erro");
    assert_eq!(hits(&t, "erro").len(), 3);
    assert_eq!(hits(&t, "Erro"), [Hit::new((0, 0), (4, 0))]);
}

#[test]
fn a_word_the_terminal_wrapped_is_found_across_the_rows() {
    let t = term(10, "0123456789abc");
    assert_eq!(hits(&t, "89ab"), [Hit::new((8, 0), (2, 1))]);
}

#[test]
fn wide_characters_take_two_cells() {
    let t = term(40, "a你好b");
    assert_eq!(hits(&t, "好b"), [Hit::new((3, 0), (6, 0))]);
    assert_eq!(hits(&t, "你好"), [Hit::new((1, 0), (5, 0))]);
}

#[test]
fn punctuation_is_text_not_a_pattern() {
    let t = term(40, "a.b axb");
    assert_eq!(hits(&t, "a.b").len(), 1);
}

#[test]
fn the_limit_caps_the_hits_and_an_empty_query_finds_nothing() {
    let t = term(40, "x x x x x");
    assert_eq!(t.find("x", 3).len(), 3);
    assert!(hits(&t, "").is_empty());
}

#[test]
fn the_search_starts_at_the_last_hit_above_the_bottom_of_the_screen() {
    let t = term(40, "primeiro erro\r\nnada aqui\r\nsegundo erro\r\n");
    let mut f = FindSession::default();
    f.search(&t, "erro", t.viewport().bottom());
    assert_eq!(f.counter().as_deref(), Some("2 de 2"));
    f.step(&t, -1);
    assert_eq!(f.counter().as_deref(), Some("1 de 2"));
    f.step(&t, 1);
    assert_eq!(f.counter().as_deref(), Some("2 de 2"));
    f.step(&t, 1);
    assert_eq!(f.counter().as_deref(), Some("1 de 2"));
}

#[test]
fn no_hits_reads_nenhum() {
    let t = term(40, "primeiro erro\r\n");
    let mut f = FindSession::default();
    f.search(&t, "inexistente", 0);
    assert_eq!(f.counter().as_deref(), Some("Nenhum"));
    assert!(f.hits().is_empty());
    f.step(&t, 1);
    assert_eq!(f.index(), None);
}

#[test]
fn new_output_does_not_move_the_current_hit() {
    let t = term(40, "primeiro erro\r\nnada aqui\r\nsegundo erro\r\n");
    let mut f = FindSession::default();
    f.search(&t, "erro", t.viewport().bottom());
    t.feed(b"terceiro erro\r\n");
    f.refresh(&t);
    assert_eq!(f.hits().len(), 3);
    assert_eq!(f.counter().as_deref(), Some("2 de 3"));
}

#[test]
fn the_current_hit_stays_put_while_the_scrollback_trims_its_top() {
    let size = GridSize { cols: 20, rows: 4, cell_width: 8, cell_height: 16 };
    let t = TerminalSession::new(size, TerminalPalette::default(), 10);
    for i in 0..14 {
        t.feed(format!("linha {i}\r\n").as_bytes());
    }
    let mut f = FindSession::default();
    f.search(&t, "linha 9", t.viewport().bottom());
    let before = f.current().unwrap();
    for i in 14..17 {
        t.feed(format!("linha {i}\r\n").as_bytes());
    }
    f.refresh(&t);
    let after = f.current().unwrap();
    assert_eq!(after.from.y + 3, before.from.y);
}

#[test]
fn a_hit_off_screen_is_centred_by_the_reveal() {
    let v = Viewport { history: 100, display_offset: 0, rows: 10 };
    assert_eq!(v.top(), 100);
    assert!(!v.shows(&Hit::new((0, 40), (3, 40))));
    assert_eq!(v.centring(40), 36);
    assert_eq!(v.centring(2), 0);
    assert_eq!(v.centring(108), 100);
    assert!(v.shows(&Hit::new((0, 105), (3, 105))));
}
