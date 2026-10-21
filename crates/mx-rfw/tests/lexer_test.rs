use mx_rfw::lexer::{Lexer, Tok};
use mx_rfw::{ParseError, parse_library_file};

fn toks(src: &str) -> Vec<Tok> {
    Lexer::tokenize(src).unwrap().into_iter().map(|t| t.tok).collect()
}

fn lex_err(src: &str) -> ParseError {
    Lexer::tokenize(src).unwrap_err()
}

fn id(s: &str) -> Tok {
    Tok::Ident(s.into())
}

#[test]
fn only_space_and_lf_are_whitespace() {
    assert_eq!(toks("a \n b"), vec![id("a"), id("b"), Tok::Eof]);
}

#[test]
fn a_tab_outside_strings_is_a_hard_error() {
    let e = lex_err("a\tb");
    assert_eq!(e.message, "Unexpected character U+0009 inside identifier");
    assert_eq!(lex_err("\tb").message, "Unexpected character U+0009");
    assert_eq!(lex_err("\tb").to_string(), "Unexpected character U+0009 at line 1 column 1.");
}

#[test]
fn a_crlf_file_fails_to_parse() {
    let e = parse_library_file("import core.widgets;\r\n").unwrap_err();
    assert_eq!(e.message, "Unexpected character U+000D");
    assert_eq!((e.line, e.column), (1, 21));
}

#[test]
fn tab_and_cr_inside_strings_and_comments_are_literal() {
    assert_eq!(toks("\"a\tb\r\""), vec![Tok::Str("a\tb\r".into()), Tok::Eof]);
    assert_eq!(toks("// a\tb\r\nx"), vec![id("x"), Tok::Eof]);
}

#[test]
fn symbols_dot_and_triple_dot() {
    assert_eq!(
        toks("( ) , : ; = > [ ] { } . ..."),
        vec![
            Tok::Symbol('('), Tok::Symbol(')'), Tok::Symbol(','), Tok::Symbol(':'), Tok::Symbol(';'),
            Tok::Symbol('='), Tok::Symbol('>'), Tok::Symbol('['), Tok::Symbol(']'), Tok::Symbol('{'),
            Tok::Symbol('}'), Tok::Symbol('.'), Tok::TripleDot, Tok::Eof,
        ]
    );
}

#[test]
fn greater_than_glued_to_an_identifier_is_an_error_but_arrow_works() {
    assert!(Lexer::tokenize("a>").is_err());
    assert_eq!(toks("=>"), vec![Tok::Symbol('='), Tok::Symbol('>'), Tok::Eof]);
}

#[test]
fn a_comment_glued_to_an_identifier_number_or_string_fails() {
    assert!(Lexer::tokenize("foo// x").is_err());
    assert!(Lexer::tokenize("\"a\"//x").is_err());
    assert!(Lexer::tokenize("1// x").is_err());
    assert_eq!(toks("foo // x"), vec![id("foo"), Tok::Eof]);
}

#[test]
fn a_slash_not_followed_by_slash_or_star_is_an_error() {
    assert_eq!(lex_err("/x").message, "Unexpected character U+0078 (\"x\") inside comment delimiter");
}

#[test]
fn block_comments_do_not_nest_and_eof_inside_is_an_error() {
    assert_eq!(toks("/* a /* b */ c"), vec![id("c"), Tok::Eof]);
    assert_eq!(lex_err("/* open").message, "Unexpected end of file in block comment");
}

#[test]
fn a_double_star_before_the_slash_does_not_close_a_block_comment() {
    assert_eq!(lex_err("/* a **/ b").message, "Unexpected end of file in block comment");
}

#[test]
fn strings_take_non_ascii_literally_and_reject_raw_newlines() {
    assert_eq!(toks("\"› próprio\""), vec![Tok::Str("› próprio".into()), Tok::Eof]);
    assert_eq!(lex_err("\"a\nb\"").message, "Unexpected end of line inside string");
    assert_eq!(lex_err("'abc").message, "Unexpected end of file inside string");
}

#[test]
fn escapes_cover_the_json_set_and_unicode_code_units() {
    assert_eq!(
        toks(r#""\" \' \\ \/ \b \f \n \r \t é""#),
        vec![Tok::Str("\" ' \\ / \u{8} \u{c} \n \r \t é".into()), Tok::Eof]
    );
    assert_eq!(toks(r#"'😀'"#), vec![Tok::Str("😀".into()), Tok::Eof]);
    assert!(lex_err(r#""\x""#).message.contains("after backslash in string"));
    assert!(lex_err(r#""\u12g4""#).message.contains("in Unicode escape"));
}

#[test]
fn after_a_closing_quote_only_space_lf_symbol_or_dot_may_follow() {
    assert_eq!(toks("\"a\".b"), vec![Tok::Str("a".into()), Tok::Symbol('.'), id("b"), Tok::Eof]);
    assert!(lex_err("\"a\"b").message.contains("after end quote"));
}

#[test]
fn decimal_integers_allow_leading_zeros_and_negative_zero() {
    assert_eq!(toks("007 -0 -12 0"), vec![Tok::Int(7), Tok::Int(0), Tok::Int(-12), Tok::Int(0), Tok::Eof]);
}

#[test]
fn hex_integers_are_positive_64_bit_values() {
    assert_eq!(toks("0xFFFFFFFF 0X00000000 0xff"), vec![Tok::Int(0xFFFF_FFFF), Tok::Int(0), Tok::Int(255), Tok::Eof]);
    assert!(Lexer::tokenize("-0x1").is_err());
    assert_eq!(toks("0x7FFFFFFFFFFFFFFF"), vec![Tok::Int(i64::MAX), Tok::Eof]);
    assert!(Lexer::tokenize("0x8000000000000000").is_err());
}

#[test]
fn a_fraction_or_exponent_makes_a_double() {
    assert_eq!(
        toks("1.0 -84.0 1e3 2.5E-2 -0.0"),
        vec![Tok::Double(1.0), Tok::Double(-84.0), Tok::Double(1000.0), Tok::Double(0.025), Tok::Double(-0.0), Tok::Eof]
    );
}

#[test]
fn malformed_numbers_are_errors() {
    assert_eq!(lex_err("1.").message, "Unexpected end of file after decimal point");
    assert_eq!(lex_err("+1").message, "Unexpected character U+002B (\"+\")");
    assert!(lex_err("1e+3").message.contains("after exponent separator"));
    assert!(lex_err("1.x").message.contains("in fraction component"));
    assert!(lex_err("- 1").message.contains("after minus sign (expected digit)"));
}

#[test]
fn a_bare_leading_dot_number_tokenizes_as_dot_then_int() {
    assert_eq!(toks(".5"), vec![Tok::Symbol('.'), Tok::Int(5), Tok::Eof]);
}

#[test]
fn numbers_after_a_dot_are_integer_parts() {
    assert_eq!(
        toks("data.rows.0.name"),
        vec![id("data"), Tok::Symbol('.'), id("rows"), Tok::Symbol('.'), Tok::Int(0), Tok::Symbol('.'), id("name"), Tok::Eof]
    );
}

#[test]
fn line_and_column_count_code_points() {
    let e = lex_err("x\n  é\t");
    assert_eq!((e.line, e.column), (2, 3));
}

#[test]
fn tokens_carry_the_position_of_the_character_that_ended_them() {
    let t = Lexer::tokenize("abc ;").unwrap();
    assert_eq!((t[0].line, t[0].column), (1, 4));
    assert_eq!((t[1].line, t[1].column), (1, 5));
}
