use mx_claude::docs::{DocSource, LinkAction, MxDoc, decode_href, follow_link, is_markdown_path};
use mx_claude::markdown::{MdKind, MdNode, parse_markdown};

#[test]
fn only_markdown_gets_a_reader() {
    assert!(is_markdown_path("/repo/PLANO.md"));
    assert!(is_markdown_path("/repo/notas.markdown"));
    assert!(is_markdown_path("/repo/Doc.MDX"));
    assert!(!is_markdown_path("/repo/lib/main.dart"));
    assert!(!is_markdown_path("/repo/dados.csv"));
}

#[test]
fn file_doc_reloads_only_when_the_disk_changed() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("PLANO.md");
    std::fs::write(&file, "# antes").unwrap();
    let mut doc = MxDoc::file(&file.to_string_lossy(), None);
    assert_eq!(doc.title, "PLANO.md");
    assert!(doc.text.is_empty());
    assert!(doc.reload());
    assert_eq!(doc.text, "# antes");
    assert!(!doc.reload());
    std::fs::write(&file, "# depois").unwrap();
    assert!(doc.reload());
    assert_eq!(doc.text, "# depois");
}

#[test]
fn a_missing_path_is_not_read_and_keeps_what_was_on_screen() {
    let mut doc = MxDoc::file("/nao/existe/PLANO.md", None);
    doc.text = "# o que já estava aberto".into();
    assert!(!doc.reload());
    assert!(doc.missing());
    assert_eq!(doc.text, "# o que já estava aberto");
}

#[test]
fn a_plan_crosses_the_config_as_text_and_a_file_as_a_path() {
    let plan = MxDoc::new(DocSource::Plan, "plano", "# o plano");
    let back = MxDoc::from_json(&plan.to_json()).unwrap();
    assert_eq!(back.text, "# o plano");
    assert_eq!(back.at, plan.at);
    assert!(!plan.missing());

    let file = MxDoc::file("/repo/PLANO.md", Some("sessão".into()));
    let json = file.to_json();
    assert!(json.get("text").is_none());
    let keys: Vec<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["source", "title", "path", "origin", "at"]);
    assert_eq!(
        MxDoc::from_json(&json).unwrap().path.as_deref(),
        Some("/repo/PLANO.md")
    );
}

#[test]
fn from_json_rejects_unknown_sources_and_files_without_path() {
    use serde_json::json;
    assert_eq!(MxDoc::from_json(&json!({"source": "nada"})), None);
    assert_eq!(
        MxDoc::from_json(&json!({"source": "file", "path": ""})),
        None
    );
    let doc = MxDoc::from_json(&json!({"source": "message"})).unwrap();
    assert_eq!(doc.title, "recado");
    assert_eq!(DocSource::Report.label(), "relatório");
}

#[test]
fn saved_text_is_clipped_in_utf16_units() {
    let doc = MxDoc::new(DocSource::Message, "m", "😀".repeat(70_000));
    let text = doc.to_json()["text"].as_str().unwrap().to_owned();
    assert!(text.ends_with("\n\n…"));
    assert_eq!(text.chars().filter(|c| *c == '😀').count(), 65_536);
}

#[test]
fn links_resolve_to_external_reader_quick_look_or_missing() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_string_lossy().into_owned();
    std::fs::write(dir.path().join("decisões.md"), "x").unwrap();
    std::fs::write(dir.path().join("dados.csv"), "x").unwrap();
    assert_eq!(follow_link("  ", Some(&base), None), LinkAction::Nothing);
    assert_eq!(
        follow_link("#ancora", Some(&base), None),
        LinkAction::Nothing
    );
    assert_eq!(
        follow_link("https://x.dev", Some(&base), None),
        LinkAction::OpenExternal("https://x.dev".into())
    );
    let decoded = decode_href("decis%C3%B5es.md#topo");
    assert_eq!(decoded, "decisões.md#topo");
    assert_eq!(
        follow_link(&decoded, Some(&base), None),
        LinkAction::Read(format!("{base}/decisões.md"))
    );
    assert_eq!(
        follow_link("dados.csv", Some(&base), None),
        LinkAction::QuickLook(format!("{base}/dados.csv"))
    );
    let missing = follow_link("~/sumiu.md", Some(&base), Some("/home/x/"));
    assert_eq!(missing, LinkAction::Missing("/home/x/sumiu.md".into()));
    assert_eq!(
        missing.problem().as_deref(),
        Some("Esse link aponta pra um arquivo que não existe: /home/x/sumiu.md")
    );
    assert_eq!(decode_href("mailto:a%40b"), "mailto:a%40b");
}

fn find<'a>(node: &'a MdNode, pred: &dyn Fn(&MdKind) -> bool, out: &mut Vec<&'a MdNode>) {
    if pred(&node.kind) {
        out.push(node);
    }
    for child in &node.children {
        find(child, pred, out);
    }
}

#[test]
fn plan_markdown_parses_to_headings_code_and_lists_without_markup() {
    let doc =
        parse_markdown("# Trocar o middleware\n\nMexe em `jwt.dart`.\n\n- primeiro\n- segundo");
    let text = doc.plain_text();
    assert!(text.contains("Trocar o middleware"));
    assert!(text.contains("jwt.dart"));
    assert!(!text.contains('#') && !text.contains('`') && !text.contains("- "));
    let mut headings = Vec::new();
    find(
        &doc,
        &|k| matches!(k, MdKind::Heading { .. }),
        &mut headings,
    );
    assert_eq!(
        headings[0].kind,
        MdKind::Heading {
            level: 1,
            id: "trocar-o-middleware".into()
        }
    );
}

#[test]
fn github_extensions_are_on() {
    let doc = parse_markdown(
        "- [x] feito\n- [ ] falta\n\n~~velho~~ :rocket: https://x.dev\n\n| a | b |\n|:-|-:|\n| 1 | 2 |\n\n> [!NOTE]\n> nota\n\nfrase[^1]\n\n[^1]: rodapé\n",
    );
    let mut nodes = Vec::new();
    find(&doc, &|_| true, &mut nodes);
    let kinds: Vec<&MdKind> = nodes.iter().map(|n| &n.kind).collect();
    assert!(kinds.contains(&&MdKind::Item { task: Some(true) }));
    assert!(kinds.contains(&&MdKind::Item { task: Some(false) }));
    assert!(kinds.contains(&&MdKind::Strikethrough));
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, MdKind::Link { url, .. } if url == "https://x.dev"))
    );
    assert!(kinds.iter().any(|k| matches!(k, MdKind::Table { .. })));
    assert!(kinds.iter().any(|k| matches!(k, MdKind::Alert { .. })));
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, MdKind::FootnoteReference { .. }))
    );
    assert!(doc.plain_text().contains('🚀'));
}
