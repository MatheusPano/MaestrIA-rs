use mx_rfw::lexer::{Lexer, Tok};
use mx_rfw::{DynamicContent, evaluate, parse_data_file, parse_library_file};
use proptest::prelude::*;

const ALPHABET: &str = "abcdefxyz_09.-+eE\"'\\/*(){}[],:;=> \n\t\r…éfor in switch default event data args widget import";

fn rfw_like() -> impl Strategy<Value = String> {
    let pieces: Vec<&'static str> = vec![
        "widget ", "import ", "root", " = ", "A(", ")", "[", "]", "{", "}", ", ", ": ", ";", "...for x in ", "x",
        "data.", "args.", "switch ", "default", "event \"e\" ", "\"s\"", "1", "-2.5", "0xFF", "true", "null", ".",
        "// c\n", "/* b */", "\n", " ", "\t", "\r", "'q'", "\\u00e9", "1e3", "x.0",
    ];
    prop::collection::vec(prop::sample::select(pieces), 0..60).prop_map(|v| v.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn the_lexer_never_panics_on_arbitrary_text(s in any::<String>()) {
        let _ = Lexer::tokenize(&s);
    }

    #[test]
    fn the_lexer_never_panics_on_rfw_flavoured_characters(s in proptest::string::string_regex(&format!("[{}]{{0,80}}", regex_escape(ALPHABET))).unwrap()) {
        let _ = Lexer::tokenize(&s);
    }

    #[test]
    fn parsing_and_evaluating_rfw_like_soup_never_panics(s in rfw_like()) {
        if let Ok(lib) = parse_library_file(&s) {
            let _ = evaluate(&lib, "root", &DynamicContent::new());
        }
        let _ = parse_data_file(&s);
    }

    #[test]
    fn decimal_ints_round_trip(v in any::<i64>()) {
        prop_assert_eq!(Lexer::tokenize(&v.to_string()).unwrap()[0].tok.clone(), Tok::Int(v));
    }

    #[test]
    fn hex_ints_round_trip(v in 0..=i64::MAX) {
        prop_assert_eq!(Lexer::tokenize(&format!("0x{v:X}")).unwrap()[0].tok.clone(), Tok::Int(v));
        prop_assert_eq!(Lexer::tokenize(&format!("0X{v:x}")).unwrap()[0].tok.clone(), Tok::Int(v));
    }

    #[test]
    fn argb_colors_round_trip_through_hex_literals(v in any::<u32>()) {
        prop_assert_eq!(Lexer::tokenize(&format!("0x{v:08X}")).unwrap()[0].tok.clone(), Tok::Int(i64::from(v)));
    }

    #[test]
    fn finite_doubles_round_trip_and_stay_doubles(v in any::<f64>().prop_filter("finite", |v| v.is_finite())) {
        let text = format!("{v:?}");
        let text = if text.contains('.') || text.contains('e') { text } else { format!("{text}.0") };
        let tok = Lexer::tokenize(&text).unwrap()[0].tok.clone();
        prop_assert_eq!(tok, Tok::Double(v));
    }

    #[test]
    fn a_whole_double_never_becomes_an_int(v in -1_000_000i64..1_000_000) {
        prop_assert_eq!(Lexer::tokenize(&format!("{v}.0")).unwrap()[0].tok.clone(), Tok::Double(v as f64));
    }
}

fn regex_escape(s: &str) -> String {
    s.chars()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|c| match c {
            '\\' | ']' | '[' | '^' | '-' => format!("\\{c}"),
            '\n' => "\\n".into(),
            '\t' => "\\t".into(),
            '\r' => "\\r".into(),
            c => c.to_string(),
        })
        .collect()
}
