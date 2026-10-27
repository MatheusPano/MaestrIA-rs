use std::path::Path;

use crate::links::resolve_link_path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkFollow {
    Nothing,
    OpenUrl(String),
    Reader(String),
    OpenFile(String),
    Missing(String),
}

impl LinkFollow {
    pub fn banner(&self) -> Option<String> {
        match self {
            LinkFollow::Missing(target) => {
                Some(format!("Esse link aponta pra um arquivo que não existe: {target}"))
            }
            _ => None,
        }
    }
}

pub fn url_failed_banner(href: &str) -> String {
    format!("Não consegui abrir {href}")
}

fn has_web_scheme(href: &str) -> bool {
    let lower = href.to_ascii_lowercase();
    ["http://", "https://", "mailto:"].iter().any(|s| lower.starts_with(s))
}

pub fn follow_link(href: &str, base: Option<&str>) -> LinkFollow {
    if href.trim().is_empty() {
        return LinkFollow::Nothing;
    }
    if has_web_scheme(href) {
        return LinkFollow::OpenUrl(href.to_owned());
    }
    let path = href.split('#').next().unwrap_or_default();
    if path.is_empty() {
        return LinkFollow::Nothing;
    }
    let target = resolve_link_path(path, base);
    if !Path::new(&target).exists() {
        return LinkFollow::Missing(target);
    }
    if mx_core::model::is_markdown_path(&target) {
        LinkFollow::Reader(target)
    } else {
        LinkFollow::OpenFile(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_relative_to_cwd_opens_reader() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("PLANO.md"), "x").unwrap();
        let base = dir.path().to_str();
        let want = format!("{}/PLANO.md", dir.path().display());
        assert_eq!(follow_link("PLANO.md", base), LinkFollow::Reader(want));
    }

    #[test]
    fn missing_file_reports_banner() {
        let dir = tempfile::tempdir().unwrap();
        let out = follow_link("SUMIU.md", dir.path().to_str());
        assert!(matches!(out, LinkFollow::Missing(_)));
        assert!(out.banner().unwrap().contains("não existe"));
    }

    #[test]
    fn web_links_open_with_system() {
        assert_eq!(follow_link("https://x.dev/a", None), LinkFollow::OpenUrl("https://x.dev/a".into()));
        assert_eq!(follow_link("mailto:a@b.c", None), LinkFollow::OpenUrl("mailto:a@b.c".into()));
        assert_eq!(url_failed_banner("https://x"), "Não consegui abrir https://x");
    }

    #[test]
    fn blank_and_bare_fragment_do_nothing() {
        assert_eq!(follow_link("  ", None), LinkFollow::Nothing);
        assert_eq!(follow_link("#top", None), LinkFollow::Nothing);
    }

    #[test]
    fn other_files_open_with_system_opener() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.png"), "x").unwrap();
        let out = follow_link("a.png#frag", dir.path().to_str());
        assert_eq!(out, LinkFollow::OpenFile(format!("{}/a.png", dir.path().display())));
    }
}
