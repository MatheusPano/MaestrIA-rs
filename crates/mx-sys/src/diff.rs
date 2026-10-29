use std::sync::LazyLock;

use regex::Regex;

use crate::runner::CommandRunner;
use crate::shell::{ShResult, q};

static HUNK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@").expect("valid regex"));
static HUNK_HEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^@@[^@]*@@\s?").expect("valid regex"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Meta,
    Hunk,
    Add,
    Del,
    Context,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub text: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
}

impl DiffLine {
    fn new(kind: DiffKind, text: &str, old_line: Option<u32>, new_line: Option<u32>) -> Self {
        Self { kind, text: text.to_owned(), old_line, new_line }
    }

    /// What git says after the `@@ … @@` — usually the enclosing function.
    pub fn hunk_context(&self) -> &str {
        HUNK_HEAD.find(&self.text).map_or("", |m| self.text[m.end()..].trim())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileDiff {
    pub lines: Vec<DiffLine>,
}

impl FileDiff {
    pub fn added(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == DiffKind::Add).count()
    }

    pub fn removed(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == DiffKind::Del).count()
    }

    pub fn is_empty(&self) -> bool {
        !self.lines.iter().any(|l| matches!(l.kind, DiffKind::Add | DiffKind::Del))
    }

    pub fn shown(&self) -> impl Iterator<Item = &DiffLine> {
        self.lines.iter().filter(|l| l.kind != DiffKind::Meta)
    }

    pub fn parse(raw: &str) -> FileDiff {
        let mut lines = Vec::new();
        let (mut old_no, mut new_no) = (0u32, 0u32);
        let mut in_hunk = false;
        for line in raw.split('\n') {
            let line = line.strip_suffix('\r').unwrap_or(line);
            if let Some(c) = HUNK.captures(line) {
                old_no = c[1].parse().unwrap_or(0);
                new_no = c[2].parse().unwrap_or(0);
                in_hunk = true;
                lines.push(DiffLine::new(DiffKind::Hunk, line, None, None));
                continue;
            }
            if !in_hunk {
                lines.push(DiffLine::new(DiffKind::Meta, line, None, None));
            } else if let Some(rest) = line.strip_prefix('+') {
                lines.push(DiffLine::new(DiffKind::Add, rest, None, Some(new_no)));
                new_no += 1;
            } else if let Some(rest) = line.strip_prefix('-') {
                lines.push(DiffLine::new(DiffKind::Del, rest, Some(old_no), None));
                old_no += 1;
            } else if line.starts_with('\\') {
                lines.push(DiffLine::new(DiffKind::Meta, line, None, None));
            } else if line.starts_with("diff --git") {
                in_hunk = false;
                lines.push(DiffLine::new(DiffKind::Meta, line, None, None));
            } else {
                let text = line.get(1..).unwrap_or("");
                lines.push(DiffLine::new(DiffKind::Context, text, Some(old_no), Some(new_no)));
                old_no += 1;
                new_no += 1;
            }
        }
        FileDiff { lines }
    }
}

fn binary(raw: &str) -> bool {
    raw.starts_with("Binary files ") || raw.contains("\nBinary files ")
}

fn read(r: &ShResult) -> Option<FileDiff> {
    (!binary(&r.stdout)).then(|| FileDiff::parse(&r.stdout))
}

/// What changed in `path` since the last commit, or None when git has nothing to say about it:
/// outside a repo, ignored, binary. A file git does not know yet is diffed against the void.
pub async fn diff_of(runner: &dyn CommandRunner, path: &str) -> Option<FileDiff> {
    let dir = match path.rfind('/') {
        Some(0) => "/".to_owned(),
        Some(i) => path[..i].to_owned(),
        None => ".".to_owned(),
    };
    let git = |args: String| runner.run(format!("git {args}"), Some(dir.clone()));
    let file = q(path);
    if git(format!("ls-files --error-unmatch -- {file}")).await.ok() {
        let r = git(format!("diff HEAD --no-color --no-ext-diff -- {file}")).await;
        return if r.ok() { read(&r) } else { None };
    }
    if !git("rev-parse --is-inside-work-tree".to_owned()).await.ok() {
        return None;
    }
    if git(format!("check-ignore -q -- {file}")).await.ok() {
        return None;
    }
    let r = git(format!("diff --no-index --no-color --no-ext-diff -- /dev/null {file}")).await;
    if r.code != 0 && r.code != 1 {
        return None;
    }
    read(&r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::FakeRunner;
    use crate::shell::output;

    const EDIT: &str = "diff --git a/x.rs b/x.rs\nindex 1..2 100644\n--- a/x.rs\n+++ b/x.rs\n\
@@ -10,4 +10,5 @@ fn main() {\n ctx\n-old\n+new\n+more\n tail\n\\ No newline at end of file";

    fn kinds(d: &FileDiff) -> Vec<DiffKind> {
        d.lines.iter().map(|l| l.kind).collect()
    }

    #[test]
    fn the_file_header_is_not_counted_as_a_removed_and_an_added_line() {
        let d = FileDiff::parse(EDIT);
        assert_eq!((d.added(), d.removed()), (2, 1));
        assert_eq!(&kinds(&d)[..4], &[DiffKind::Meta; 4]);
        assert!(!d.is_empty());
    }

    #[test]
    fn line_numbers_on_both_sides_come_from_the_hunk_header() {
        let d = FileDiff::parse(EDIT);
        let body: Vec<(DiffKind, &str, Option<u32>, Option<u32>)> =
            d.shown().map(|l| (l.kind, l.text.as_str(), l.old_line, l.new_line)).collect();
        assert_eq!(
            body,
            vec![
                (DiffKind::Hunk, "@@ -10,4 +10,5 @@ fn main() {", None, None),
                (DiffKind::Context, "ctx", Some(10), Some(10)),
                (DiffKind::Del, "old", Some(11), None),
                (DiffKind::Add, "new", None, Some(11)),
                (DiffKind::Add, "more", None, Some(12)),
                (DiffKind::Context, "tail", Some(12), Some(13)),
            ]
        );
    }

    #[test]
    fn the_no_newline_note_is_git_talking_not_a_line_of_the_file() {
        let d = FileDiff::parse(EDIT);
        let last = d.lines.last().unwrap();
        assert_eq!(last.kind, DiffKind::Meta);
        assert_eq!(d.shown().count(), 6);
    }

    #[test]
    fn a_hunk_header_says_the_function_it_sits_in() {
        let d = FileDiff::parse(EDIT);
        let hunk = d.shown().next().unwrap();
        assert_eq!(hunk.hunk_context(), "fn main() {");
        let bare = FileDiff::parse("@@ -1 +1 @@\n-a\n+b");
        assert_eq!(bare.lines[0].hunk_context(), "");
        assert_eq!((bare.lines[1].old_line, bare.lines[2].new_line), (Some(1), Some(1)));
    }

    #[test]
    fn a_second_file_header_is_header_again() {
        let d = FileDiff::parse("@@ -1 +1 @@\n-a\n+b\ndiff --git a/y b/y\n--- a/y\n+++ b/y");
        assert_eq!((d.added(), d.removed()), (1, 1));
    }

    #[test]
    fn nothing_but_a_header_is_an_empty_diff() {
        assert!(FileDiff::parse("").is_empty());
        assert!(FileDiff::parse("diff --git a/x b/x\nold mode 100644\nnew mode 100755").is_empty());
    }

    #[tokio::test]
    async fn a_tracked_file_is_diffed_against_head_from_its_own_folder() {
        let f = FakeRunner::new();
        f.respond("git ls-files", output(0, "x.rs", ""));
        f.respond("git diff HEAD", output(0, EDIT, ""));
        let d = diff_of(&f, "/repo/src/x.rs").await.unwrap();
        assert_eq!(d.added(), 2);
        let calls = f.calls();
        assert_eq!(calls[1].command, "git diff HEAD --no-color --no-ext-diff -- '/repo/src/x.rs'");
        assert_eq!(calls[1].cwd.as_deref(), Some("/repo/src"));
    }

    #[tokio::test]
    async fn an_untracked_file_is_diffed_against_nothing_and_exit_one_is_normal() {
        let f = FakeRunner::new();
        f.respond("git rev-parse", output(0, "true", ""));
        f.respond("git diff --no-index", output(1, "--- /dev/null\n+++ b/n.md\n@@ -0,0 +1,2 @@\n+a\n+b", ""));
        let d = diff_of(&f, "/repo/n.md").await.unwrap();
        assert_eq!((d.added(), d.removed()), (2, 0));
        assert_eq!(
            f.commands(),
            vec![
                "git ls-files --error-unmatch -- '/repo/n.md'",
                "git rev-parse --is-inside-work-tree",
                "git check-ignore -q -- '/repo/n.md'",
                "git diff --no-index --no-color --no-ext-diff -- /dev/null '/repo/n.md'",
            ]
        );
    }

    #[tokio::test]
    async fn outside_a_repo_ignored_or_binary_there_is_no_diff() {
        let outside = FakeRunner::new();
        assert_eq!(diff_of(&outside, "/tmp/a.txt").await, None);
        assert_eq!(outside.commands().len(), 2);

        let ignored = FakeRunner::new();
        ignored.respond("git rev-parse", output(0, "true", ""));
        ignored.respond("git check-ignore", output(0, "", ""));
        assert_eq!(diff_of(&ignored, "/repo/build/a.o").await, None);

        let bin = FakeRunner::new();
        bin.respond("git ls-files", output(0, "", ""));
        bin.respond("git diff HEAD", output(0, "diff --git a/i.png b/i.png\nBinary files a/i.png and b/i.png differ", ""));
        assert_eq!(diff_of(&bin, "/repo/i.png").await, None);

        let broken = FakeRunner::new();
        broken.respond("git rev-parse", output(0, "true", ""));
        broken.respond("git diff --no-index", output(128, "", "fatal"));
        assert_eq!(diff_of(&broken, "/repo/n.md").await, None);
    }
}
