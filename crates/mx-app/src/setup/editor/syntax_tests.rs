use super::{Tone, highlight};

fn tones<'a>(text: &'a str, lang: &str) -> Vec<(&'a str, Tone)> {
    highlight(text, lang).into_iter().map(|(r, t)| (&text[r], t)).collect()
}

#[test]
fn javascript_colours_keywords_strings_numbers_and_comments() {
    let t = tones("const a = 'x'; // nota\nreturn 42 /* b */", "javascript");
    assert_eq!(
        t,
        [
            ("const", Tone::Keyword),
            ("'x'", Tone::String),
            ("// nota", Tone::Comment),
            ("return", Tone::Keyword),
            ("42", Tone::Number),
            ("/* b */", Tone::Comment),
        ]
    );
}

#[test]
fn a_block_comment_spans_lines() {
    let t = tones("a /* um\ndois */ b", "c");
    assert_eq!(t, [("/* um\ndois */", Tone::Comment)]);
}

#[test]
fn an_unclosed_string_stops_at_the_end_of_the_line() {
    let t = tones("x = \"aberta\ny = 1", "python");
    assert_eq!(t, [("\"aberta", Tone::String), ("1", Tone::Number)]);
}

#[test]
fn template_literals_cross_lines_in_javascript() {
    let t = tones("`a\nb` + c", "typescript");
    assert_eq!(t, [("`a\nb`", Tone::String)]);
}

#[test]
fn yaml_and_json_keys_differ_from_values() {
    assert_eq!(
        tones("name: web # comentário\nreplicas: 3", "yaml"),
        [("name", Tone::Key), ("# comentário", Tone::Comment), ("replicas", Tone::Key), ("3", Tone::Number)]
    );
    assert_eq!(
        tones("{\"a\": \"b\", \"c\": true}", "json"),
        [("\"a\"", Tone::Key), ("\"b\"", Tone::String), ("\"c\"", Tone::Key), ("true", Tone::Literal)]
    );
}

#[test]
fn identifiers_with_digits_are_not_numbers_and_types_are_capitalised() {
    assert_eq!(tones("let x2 = Vec::new();", "rust"), [("let", Tone::Keyword), ("Vec", Tone::Type), ("new", Tone::Function)]);
}

#[test]
fn sql_and_dockerfile_keywords_ignore_case() {
    assert_eq!(tones("SELECT x FROM t", "sql"), [("SELECT", Tone::Keyword), ("FROM", Tone::Keyword)]);
    assert_eq!(tones("FROM node:20", "dockerfile")[0], ("FROM", Tone::Keyword));
}

#[test]
fn markup_colours_tags_attributes_and_comments() {
    assert_eq!(
        tones("<a href=\"x\">t</a><!-- c -->", "xml"),
        [
            ("<", Tone::Meta),
            ("a", Tone::Variable),
            ("href", Tone::Key),
            ("\"x\"", Tone::String),
            (">", Tone::Meta),
            ("</", Tone::Meta),
            ("a", Tone::Variable),
            (">", Tone::Meta),
            ("<!-- c -->", Tone::Comment)
        ]
    );
}

#[test]
fn markdown_headings_fences_and_inline_code() {
    assert_eq!(
        tones("# Título\nuse `x`\n```\ncode\n```", "markdown"),
        [("# Título", Tone::Section), ("`x`", Tone::String), ("```", Tone::String), ("code", Tone::String), ("```", Tone::String)]
    );
}

#[test]
fn diff_lines_are_added_removed_or_hunks() {
    assert_eq!(
        tones("@@ -1 +1 @@\n-a\n+b\n c", "diff"),
        [("@@ -1 +1 @@", Tone::Meta), ("-a", Tone::Removed), ("+b", Tone::Added)]
    );
}

#[test]
fn unknown_languages_and_plaintext_have_no_colour() {
    assert!(highlight("const x = 1", "plaintext").is_empty());
    assert!(highlight("const x = 1", "klingon").is_empty());
}

#[test]
fn spans_always_fall_on_char_boundaries() {
    let text = "ção = 'é' // ü\n\"ñ";
    for lang in ["javascript", "python", "yaml", "xml", "markdown", "sql"] {
        for (r, _) in highlight(text, lang) {
            assert!(text.is_char_boundary(r.start) && text.is_char_boundary(r.end), "{lang} {r:?}");
        }
    }
}

#[test]
fn literals_function_names_and_shell_variables_get_their_own_tones() {
    assert_eq!(
        tones("function soma(x) { return null; }", "javascript"),
        [("function", Tone::Keyword), ("soma", Tone::Function), ("return", Tone::Keyword), ("null", Tone::Literal)]
    );
    assert_eq!(tones("echo $HOME", "bash"), [("echo", Tone::Keyword), ("$HOME", Tone::Variable)]);
}

#[test]
fn markdown_bullets_emphasis_strong_and_links() {
    assert_eq!(
        tones("- *a* **b** [t](u)", "markdown"),
        [("-", Tone::Bullet), ("*a*", Tone::Emphasis), ("**b**", Tone::Strong), ("[t]", Tone::String), ("(u)", Tone::Link)]
    );
}
