mod common;

use common::{content, find, fixture, fixtures_dir, problems, texts};
use mx_rfw::widgets::code_editor::{CodeEditor, DEFAULT_FONT_SIZE};
use mx_rfw::widgets::{Local, decode};
use mx_rfw::{DynamicContent, LocalLibrary, Value, evaluate, parse_library_file};

const EDITOR: &str = "maestria-editor__ui__editor.rfwtxt";

fn data() -> DynamicContent {
    content(&std::fs::read_to_string(fixtures_dir().join("data/editor.json")).unwrap())
}

fn with_mode(mode: &str) -> DynamicContent {
    let mut c = data();
    c.update("mode", Value::from(mode));
    c
}

#[test]
fn code_editor_is_part_of_the_maestria_library() {
    assert!(LocalLibrary::Maestria.has("CodeEditor"));
}

#[test]
fn every_editor_mode_evaluates_without_errors() {
    let lib = parse_library_file(&fixture(EDITOR)).unwrap();
    for mode in ["editor", "empty", "error"] {
        let tree = evaluate(&lib, "root", &with_mode(mode));
        assert!(problems(&tree).is_empty(), "mode {mode}: {:#?}", problems(&tree));
    }
}

#[test]
fn editor_mode_renders_one_code_editor_with_the_file_args() {
    let lib = parse_library_file(&fixture(EDITOR)).unwrap();
    let tree = evaluate(&lib, "root", &data());
    let found = find(&tree, |n| n.name == "CodeEditor");
    assert_eq!(found.len(), 1);
    let Local::CodeEditor(e) = decode(found[0]) else { panic!("CodeEditor did not decode") };
    assert_eq!(e.doc, "/proj/src/hello.js");
    assert_eq!(e.text.as_deref(), Some("const a = 1;\nfunction soma(x) {\n  return x + a;\n}\n"));
    assert_eq!((e.revision, e.jump), (2, 1));
    assert_eq!((e.line, e.column), (Some(3), Some(7)));
    assert_eq!(e.language, "javascript");
    assert!(!e.read_only && !e.word_wrap);
    assert_eq!(e.font_size, 13.0);
    let names = |v: &[&mx_rfw::Event]| v.iter().map(|e| e.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&e.on_change), ["change"]);
    assert_eq!(names(&e.on_save), ["save"]);
    assert_eq!(names(&e.on_cursor), ["cursor"]);
}

#[test]
fn a_file_without_a_jump_has_no_line_and_the_editor_defaults_apply() {
    let lib = parse_library_file(&fixture(EDITOR)).unwrap();
    let mut c = data();
    c.update("file", Value::from_json(&serde_json::json!({ "path": "/a", "text": "", "rev": 1, "jump": 0 })));
    c.update("opts", Value::from_json(&serde_json::json!({})));
    let tree = evaluate(&lib, "root", &c);
    let node = find(&tree, |n| n.name == "CodeEditor")[0];
    let e = CodeEditor::from_node(node);
    assert_eq!((e.line, e.column), (None, None));
    assert_eq!(e.font_size, DEFAULT_FONT_SIZE);
    assert_eq!(e.language, "plaintext");
}

#[test]
fn the_tabs_status_bar_and_banner_show_their_texts() {
    let lib = parse_library_file(&fixture(EDITOR)).unwrap();
    let t = texts(&evaluate(&lib, "root", &data()));
    for expected in ["hello.js", "lib", "src  ›  hello.js", "Ln 3, Col 7", "não salvo", "JavaScript", "Manter o meu"] {
        assert!(t.iter().any(|s| s == expected), "{expected} not in {t:?}");
    }
}

#[test]
fn empty_and_error_modes_show_their_texts() {
    let lib = parse_library_file(&fixture(EDITOR)).unwrap();
    let empty = texts(&evaluate(&lib, "root", &with_mode("empty")));
    assert!(empty.iter().any(|s| s == "PASTAS RECENTES"), "{empty:?}");
    let error = texts(&evaluate(&lib, "root", &with_mode("error")));
    assert!(error.iter().any(|s| s == "imagem.png"), "{error:?}");
}
