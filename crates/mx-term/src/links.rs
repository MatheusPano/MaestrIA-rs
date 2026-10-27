use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

const BREAKS: &str = " \t\"'`<>|()[]{},;*";
const URL_BREAKS: &str = " \t\"'`<>|[]{},;*";
const TAIL: &str = ".,;:!?'\"";

static URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:https?://|mailto:)\S").expect("url regex"));
static AT_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?):\d+(?::\d+)?$").expect("at-line regex"));

pub fn trim_tail(s: &str) -> &str {
    s.trim_end_matches(|c| TAIL.contains(c))
}

fn word(line: &[char], column: usize, breaks: &str) -> (usize, usize) {
    let mut start = column;
    while start > 0 && !breaks.contains(line[start - 1]) {
        start -= 1;
    }
    let mut end = column;
    while end < line.len() && !breaks.contains(line[end]) {
        end += 1;
    }
    (start, end)
}

fn url_at(line: &[char], column: usize) -> Option<String> {
    let (ws, we) = word(line, column, URL_BREAKS);
    let text: String = line[ws..we].iter().collect();
    let m = URL.find(&text)?;
    let start = ws + text[..m.start()].chars().count();
    let mut depth = 0usize;
    let mut end = start;
    while end < we {
        match line[end] {
            '(' => depth += 1,
            ')' if depth == 0 => break,
            ')' => depth -= 1,
            c if BREAKS.contains(c) => break,
            _ => {}
        }
        end += 1;
    }
    if column < start || column >= end {
        return None;
    }
    let raw: String = line[start..end].iter().collect();
    let trimmed = trim_tail(&raw);
    URL.is_match(trimmed).then(|| trimmed.to_owned())
}

pub fn link_at(line: &str, column: usize, base: Option<&str>) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();
    link_at_chars(&chars, column, base)
}

pub fn link_at_chars(line: &[char], column: usize, base: Option<&str>) -> Option<String> {
    if column >= line.len() || BREAKS.contains(line[column]) {
        return None;
    }
    if let Some(url) = url_at(line, column) {
        return Some(url);
    }
    let (s, e) = word(line, column, BREAKS);
    let token: String = line[s..e].iter().collect();
    if URL.is_match(&token) {
        return None;
    }
    let bare = trim_tail(&token);
    let path = trim_tail(
        AT_LINE
            .captures(bare)
            .and_then(|c| c.get(1))
            .map_or(bare, |m| m.as_str()),
    );
    if !path.contains('/') && !mx_core::model::is_markdown_path(path) {
        return None;
    }
    let file = path.split('#').next().unwrap_or_default();
    Path::new(&resolve_link_path(file, base))
        .exists()
        .then(|| path.to_owned())
}

pub fn resolve_link_path(path: &str, base: Option<&str>) -> String {
    let path = mx_core::paths::expand_home(path);
    if path.starts_with('/') {
        return path;
    }
    match base.filter(|b| !b.is_empty()) {
        Some(b) => format!("{}/{path}", b.trim_end_matches('/')),
        None => path,
    }
}

pub fn local_path(url: &str) -> String {
    let Some(rest) = url.strip_prefix("file://") else {
        return url.to_owned();
    };
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    if (host.is_empty() || host == "localhost") && !path.is_empty() {
        percent_decode(path)
    } else {
        url.to_owned()
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: &str, col: usize) -> Option<String> {
        link_at(line, col, None)
    }

    #[test]
    fn url_found_from_any_column_inside_it() {
        let line = "veja https://docs.anthropic.com/claude-code depois";
        for col in [5, 20, 42] {
            assert_eq!(at(line, col).as_deref(), Some("https://docs.anthropic.com/claude-code"));
        }
    }

    #[test]
    fn trailing_period_is_trimmed() {
        assert_eq!(at("abre https://x.dev/a.", 10).as_deref(), Some("https://x.dev/a"));
    }

    #[test]
    fn box_drawing_prefix_does_not_matter() {
        assert_eq!(at("│ https://x.dev/a", 12).as_deref(), Some("https://x.dev/a"));
    }

    #[test]
    fn enclosing_parens_are_not_part_of_the_url() {
        assert_eq!(at("(veja https://x.dev/a)", 10).as_deref(), Some("https://x.dev/a"));
    }

    #[test]
    fn balanced_parens_inside_url_are_kept() {
        let line = "veja https://en.wikipedia.org/wiki/Rust_(programming_language).";
        let want = Some("https://en.wikipedia.org/wiki/Rust_(programming_language)");
        assert_eq!(at(line, 10).as_deref(), want);
        let col = line.find("programming").unwrap();
        assert_eq!(at(line, col).as_deref(), want);
        let line = "(veja https://en.wikipedia.org/wiki/Rust_(x))";
        assert_eq!(at(line, 10).as_deref(), Some("https://en.wikipedia.org/wiki/Rust_(x)"));
    }

    #[test]
    fn mailto_is_a_link() {
        assert_eq!(
            at("escreva pra mailto:eu@marrow.com.br", 20).as_deref(),
            Some("mailto:eu@marrow.com.br")
        );
    }

    #[test]
    fn non_links_are_null() {
        assert_eq!(at("pegue em ftp://x.dev/a", 15), None);
        assert_eq!(at("escrevi o middleware e/ou o resto", 22), None);
        assert_eq!(at("nada aqui", 3), None);
        assert_eq!(at("", 0), None);
        assert_eq!(at("abc", 9), None);
    }

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("lib")).unwrap();
        std::fs::write(dir.path().join("lib/store.dart"), "x").unwrap();
        std::fs::write(dir.path().join("PLANO.md"), "x").unwrap();
        dir
    }

    #[test]
    fn existing_relative_path_is_a_link() {
        let dir = project();
        let base = dir.path().to_str();
        assert_eq!(link_at("escrevi lib/store.dart agora", 12, base).as_deref(), Some("lib/store.dart"));
        assert_eq!(link_at("escrevi lib/sumiu.dart agora", 12, base), None);
        assert_eq!(link_at("escrevi lib/store.dart agora", 12, None), None);
    }

    #[test]
    fn line_and_column_suffix_is_stripped() {
        let dir = project();
        let base = dir.path().to_str();
        assert_eq!(link_at("veja lib/store.dart:42:9", 10, base).as_deref(), Some("lib/store.dart"));
        let abs = format!("veja {}/PLANO.md:3", dir.path().display());
        let want = format!("{}/PLANO.md", dir.path().display());
        assert_eq!(link_at(&abs, 10, None), Some(want));
    }

    #[test]
    fn bare_markdown_name_is_a_link() {
        let dir = project();
        let base = dir.path().to_str();
        assert_eq!(link_at("atualizei o PLANO.md", 14, base).as_deref(), Some("PLANO.md"));
        assert_eq!(link_at("atualizei o PLANO.md", 3, base), None);
    }

    #[test]
    fn file_urls_become_paths() {
        assert_eq!(local_path("file:///tmp/PLANO.md"), "/tmp/PLANO.md");
        assert_eq!(local_path("file://localhost/tmp/a%20b.md"), "/tmp/a b.md");
        assert_eq!(local_path("file://outra-maquina/etc/hosts"), "file://outra-maquina/etc/hosts");
        assert_eq!(local_path("https://x.dev"), "https://x.dev");
    }

    #[test]
    fn resolve_relative_absolute_and_home() {
        assert_eq!(resolve_link_path("a.md", Some("/w")), "/w/a.md");
        assert_eq!(resolve_link_path("/x/a.md", Some("/w")), "/x/a.md");
        assert_eq!(resolve_link_path("a.md", None), "a.md");
        assert!(!resolve_link_path("~/a.md", None).starts_with('~'));
    }
}
