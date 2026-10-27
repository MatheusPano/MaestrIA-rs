use mx_rfw::ast::{Expr, Item, Part};
use mx_rfw::{ParseErrorKind, Value, parse_data_file, parse_library_file, parse_error_text};

fn root(src: &str) -> Expr {
    parse_library_file(src).unwrap().widgets.remove(0).root
}

fn args(e: &Expr) -> &Vec<(String, Expr)> {
    match e {
        Expr::Call { args, .. } => args,
        other => panic!("not a call: {other:?}"),
    }
}

fn err(src: &str) -> String {
    parse_library_file(src).unwrap_err().to_string()
}

#[test]
fn imports_and_widget_declarations_are_read_in_order() {
    let lib = parse_library_file("import core.widgets;\nimport maestria;\nwidget a = A();\nwidget b = switch 1 { default: B() };").unwrap();
    assert_eq!(lib.imports.iter().map(|i| i.dotted()).collect::<Vec<_>>(), ["core.widgets", "maestria"]);
    assert_eq!(lib.widgets.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    assert!(matches!(lib.widgets[1].root, Expr::Switch(_)));
}

#[test]
fn an_empty_file_is_an_empty_library() {
    let lib = parse_library_file("// nada\n").unwrap();
    assert!(lib.imports.is_empty() && lib.widgets.is_empty());
}

#[test]
fn a_widget_root_must_be_a_constructor_call_or_switch() {
    assert_eq!(err("widget A = args.child;"), "Expected symbol \"(\" but found . at line 1 column 17.");
}

#[test]
fn trailing_junk_reports_what_was_expected() {
    assert_eq!(err("foo"), "Expected keywords \"import\" or \"widget\", or end of file but found foo at line 1 column 3.");
    assert_eq!(err("widget a = A(); x"), "Expected keyword \"widget\" or end of file but found x at line 1 column 17.");
}

#[test]
fn the_parse_error_surface_text_matches_the_host() {
    let e = parse_library_file("widget root = Column(children: [").unwrap_err();
    assert!(parse_error_text(&e).starts_with("a interface do plugin não leu:\n"));
    assert!(parse_error_text(&e).ends_with(&format!("at line {} column {}.", e.line, e.column)));
}

#[test]
fn ints_and_doubles_stay_distinct_in_literals() {
    let e = root("widget a = A(i: 12, d: 12.0, h: 0xFFFFFFFF, n: -1.0);");
    let a = args(&e);
    assert_eq!(a[0].1, Expr::Int(12));
    assert_eq!(a[1].1, Expr::Double(12.0));
    assert_eq!(a[2].1, Expr::Int(0xFFFF_FFFF));
    assert_eq!(a[3].1, Expr::Double(-1.0));
}

#[test]
fn a_null_map_value_drops_the_key() {
    let e = root("widget a = A(x: null, y: 1);");
    assert_eq!(args(&e).len(), 1);
    assert_eq!(args(&e)[0].0, "y");
}

#[test]
fn null_outside_a_map_value_is_parsed_as_a_constructor_and_fails() {
    assert!(err("widget a = A(x: [null]);").starts_with("Expected symbol \"(\""));
}

#[test]
fn duplicate_keys_are_an_error() {
    assert_eq!(err("widget a = A(x: 1, x: 2);"), "Duplicate key \"x\" in map at line 1 column 21.");
}

#[test]
fn reserved_words_may_be_used_as_keys_and_strings_as_keys() {
    let e = root("widget a = A(data: 1, \"k k\": 2, event: 3);");
    assert_eq!(args(&e).iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), ["data", "k k", "event"]);
}

#[test]
fn list_items_need_commas_and_allow_a_trailing_comma() {
    assert!(parse_library_file("widget a = A(l: [1, 2,]);").is_ok());
    assert!(err("widget a = A(l: [1 2]);").starts_with("Expected comma but found 2"));
}

#[test]
fn references_parse_parts_including_int_and_string_parts() {
    let e = root("widget a = A(x: data.rows.0.\"k\", y: args.s.items);");
    assert_eq!(
        args(&e)[0].1,
        Expr::Data(vec![Part::Key("rows".into()), Part::Index(0), Part::Key("k".into())])
    );
    assert_eq!(args(&e)[1].1, Expr::Args(vec![Part::Key("s".into()), Part::Key("items".into())]));
}

#[test]
fn loop_variables_resolve_to_de_bruijn_depths() {
    let e = root("widget a = A(l: [...for x in data.a: [...for y in x.b: B(o: x, i: y.c)]]);");
    let Expr::List(items) = &args(&e)[0].1 else { panic!() };
    let Item::Loop { template: Expr::List(inner), .. } = &items[0] else { panic!() };
    let Item::Loop { input, template } = &inner[0] else { panic!() };
    assert_eq!(*input, Expr::LoopVar { depth: 0, parts: vec![Part::Key("b".into())] });
    let b = args(template);
    assert_eq!(b[0].1, Expr::LoopVar { depth: 1, parts: vec![] });
    assert_eq!(b[1].1, Expr::LoopVar { depth: 0, parts: vec![Part::Key("c".into())] });
}

#[test]
fn a_reserved_loop_variable_name_is_an_error() {
    assert!(err("widget a = A(l: [...for data in x: 1]);").starts_with("data is a reserved word"));
}

#[test]
fn switch_rejects_duplicate_cases_including_int_and_double_of_the_same_value() {
    assert!(err("widget a = switch 1 { 1: A(), 1.0: B() };").starts_with("Switch has duplicate cases for key 1.0"));
    assert!(err("widget a = switch 1 { default: A(), default: B() };").starts_with("Switch has multiple default cases"));
}

#[test]
fn events_need_a_string_name_and_a_map() {
    let e = root("widget a = A(onTap: event \"act\" { a: \"x\" });");
    assert!(matches!(&args(&e)[0].1, Expr::Event { name, args } if name == "act" && args.len() == 1));
    assert!(err("widget a = A(onTap: event \"act\");").starts_with("Expected symbol \"{\""));
}

#[test]
fn unsupported_features_fail_with_a_clear_unsupported_error() {
    for (src, what) in [
        ("widget a { x: 1 } = A();", "Stateful widgets"),
        ("widget a = A(x: state.x);", "State references"),
        ("widget a = A(x: set state.x = 1);", "set state handlers"),
        ("widget a = A(b: (x) => B());", "Widget builders"),
    ] {
        let e = parse_library_file(src).unwrap_err();
        assert_eq!(e.kind, ParseErrorKind::Unsupported, "{src}");
        assert!(e.message.starts_with(what), "{src}: {}", e.message);
    }
}

#[test]
fn absurd_nesting_is_rejected_instead_of_overflowing_the_stack() {
    let src = format!("widget a = A(x: {}{});", "[".repeat(5000), "]".repeat(5000));
    assert_eq!(parse_library_file(&src).unwrap_err().kind, ParseErrorKind::TooDeep);
}

#[test]
fn data_files_hold_only_literals() {
    let m = parse_data_file("{ a: 1, b: [2.0, \"s\", true], c: { d: null } }").unwrap();
    assert_eq!(m["a"], Value::Int(1));
    assert_eq!(m["b"].at(0), &Value::Double(2.0));
    assert!(m["c"].as_map().unwrap().is_empty());
    assert!(parse_data_file("{ a: data.x }").is_err());
    assert!(parse_data_file("{ a: [...for x in y: 1] }").is_err());
}
