use mx_sys::code_workspace::{
    CodeWorkspace, WorkspaceImport, looks_like_one, name_of, normalize, parse_with, read_with,
    strip_jsonc,
};

const HOME: Option<&str> = Some("/home/eu");

fn parse(source: &str) -> Option<CodeWorkspace> {
    parse_with(source, "/repos/cefis.code-workspace", HOME)
}

fn paths(ws: &CodeWorkspace) -> Vec<&str> {
    ws.folders.iter().map(|f| f.path.as_str()).collect()
}

#[test]
fn looks_like_one_by_extension_ignoring_case_and_spaces() {
    assert!(looks_like_one("/repos/cefis.code-workspace"));
    assert!(looks_like_one("  /repos/Cefis.Code-Workspace  "));
    assert!(!looks_like_one("/repos/cefis"));
    assert!(!looks_like_one("/repos/settings.json"));
}

#[test]
fn folders_come_in_order_and_the_name_is_the_files() {
    let ws = parse(r#" { "folders": [ {"path": "/a"}, {"path": "/b"} ] } "#).unwrap();
    assert_eq!(ws.name, "cefis");
    assert_eq!(paths(&ws), ["/a", "/b"]);
}

#[test]
fn relative_paths_resolve_against_the_files_folder() {
    let src = r#"{"folders": [{"path": "api"}, {"path": "../outro/web"}, {"path": "./aqui"}]}"#;
    let ws = parse_with(src, "/Volumes/repos/cefis/tudo.code-workspace", HOME).unwrap();
    assert_eq!(
        paths(&ws),
        ["/Volumes/repos/cefis/api", "/Volumes/repos/outro/web", "/Volumes/repos/cefis/aqui"]
    );
}

#[test]
fn absolute_passes_and_inner_dot_dot_collapses() {
    let ws = parse(r#"{"folders": [{"path": "/a/b/../b/./c"}, {"path": "/../x"}]}"#).unwrap();
    assert_eq!(paths(&ws), ["/a/b/c", "/x"]);
}

#[test]
fn tilde_of_a_hand_written_file_becomes_home() {
    let ws = parse(r#"{"folders": [{"path": "~/repos/api"}]}"#).unwrap();
    assert_eq!(paths(&ws), ["/home/eu/repos/api"]);
}

#[test]
fn the_vscode_alias_comes_along_when_present() {
    let ws = parse(r#"{"folders": [{"path": "/a", "name": "API"}, {"path": "/b"}]}"#).unwrap();
    assert_eq!(ws.folders[0].name.as_deref(), Some("API"));
    assert_eq!(ws.folders[1].name, None);
}

#[test]
fn an_entry_without_path_is_skipped_instead_of_failing_the_file() {
    let ws = parse(r#"{"folders": [{"name": "só nome"}, {"path": ""}, {"path": "/b"}]}"#).unwrap();
    assert_eq!(paths(&ws), ["/b"]);
}

#[test]
fn no_folders_is_a_workspace_without_folders_not_an_error() {
    assert!(parse(r#"{"settings": {}}"#).unwrap().folders.is_empty());
}

#[test]
fn broken_json_and_non_object_json_are_none() {
    assert_eq!(parse(r#"{"folders": ["#), None);
    assert_eq!(parse("[1, 2]"), None);
    assert_eq!(parse(""), None);
}

#[test]
fn line_comments_block_comments_and_trailing_commas() {
    let src = r#"
        {
          // as pastas do cliente
          "folders": [
            {"path": "/a"}, /* o de sempre */
            {"path": "/b"},
          ],
          "settings": {},
        }
    "#;
    assert_eq!(paths(&parse(src).unwrap()), ["/a", "/b"]);
}

#[test]
fn slashes_of_a_url_inside_a_string_are_not_a_comment() {
    let src = r#"{"folders": [{"path": "/a", "name": "http://x/y"}],
         "settings": {"u": "https://exemplo.com//x"}}"#;
    assert_eq!(parse(src).unwrap().folders[0].name.as_deref(), Some("http://x/y"));
}

#[test]
fn comma_and_brace_inside_a_string_stay_put() {
    let ws = parse(r#"{"folders": [{"path": "/a", "name": "b, }"}]}"#).unwrap();
    assert_eq!(ws.folders[0].name.as_deref(), Some("b, }"));
}

#[test]
fn an_escaped_backslash_does_not_swallow_the_next_quote() {
    let ws = parse(r#"{"folders": [{"path": "/a", "name": "c:\\"}, {"path": "/b"}]}"#).unwrap();
    assert_eq!(paths(&ws), ["/a", "/b"]);
}

#[test]
fn an_unterminated_block_comment_runs_to_the_end() {
    assert_eq!(strip_jsonc("{} /* nada"), "{} ");
    assert_eq!(strip_jsonc("[1, // x\n2,\n]"), "[1, \n2\n]");
}

#[test]
fn normalize_keeps_excess_dot_dot_only_in_relative_paths() {
    assert_eq!(normalize("../a/../../b"), "../../b");
    assert_eq!(normalize("a/.."), ".");
    assert_eq!(normalize("/.."), "/");
}

#[test]
fn name_of_strips_the_extension_case_insensitively_unless_nothing_remains() {
    assert_eq!(name_of("/r/Tudo.CODE-WORKSPACE"), "Tudo");
    assert_eq!(name_of("/r/.code-workspace"), ".code-workspace");
    assert_eq!(name_of("/r/plain.json"), "plain.json");
}

#[tokio::test]
async fn read_expands_home_and_missing_files_are_none() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap();
    std::fs::write(dir.path().join("w.code-workspace"), r#"{"folders":[{"path":"api"}]}"#).unwrap();
    let ws = read_with("  ~/w.code-workspace ", Some(home)).await.unwrap();
    assert_eq!(ws.path, format!("{home}/w.code-workspace"));
    assert_eq!(ws.name, "w");
    assert_eq!(paths(&ws), [format!("{home}/api")]);
    assert_eq!(read_with("/nao/existe.code-workspace", Some(home)).await, None);
}

fn import(added: usize, already: usize, missing: usize) -> WorkspaceImport<()> {
    WorkspaceImport {
        workspace: CodeWorkspace { path: "/r/cefis.code-workspace".into(), name: "cefis".into(), folders: vec![] },
        added: vec![(); added],
        already: vec![(); already],
        missing: (0..missing).map(|i| format!("/r/sumiu{i}")).collect(),
    }
}

#[test]
fn import_summary_counts_in_portuguese() {
    assert_eq!(import(2, 0, 0).summary(), "Workspace \"cefis\": 2 pastas adicionadas");
    assert_eq!(import(1, 1, 0).summary(), "Workspace \"cefis\": 1 pasta adicionada, 1 já estava aqui");
    assert_eq!(import(1, 0, 1).summary(), "Workspace \"cefis\": 1 pasta adicionada, 1 não existe no disco");
    assert_eq!(import(0, 2, 0).summary(), "Workspace \"cefis\": 2 já estavam aqui");
    assert_eq!(import(0, 0, 2).summary(), "Workspace \"cefis\": 2 não existem no disco");
    assert_eq!(import(0, 0, 0).summary(), "Workspace \"cefis\": nada pra adicionar");
}

#[test]
fn import_is_sticky_only_when_something_is_missing() {
    assert!(import(1, 0, 1).sticky());
    assert!(!import(1, 1, 0).sticky());
}
