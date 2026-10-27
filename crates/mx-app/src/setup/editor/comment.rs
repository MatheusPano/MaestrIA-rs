use std::ops::Range;

use gpui::Context;

use super::TextEditor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommentStyle {
    pub line: Option<&'static str>,
    pub block: Option<(&'static str, &'static str)>,
}

const SLASHES: CommentStyle = CommentStyle { line: Some("//"), block: Some(("/*", "*/")) };
const HASH: CommentStyle = CommentStyle { line: Some("#"), block: None };
const DASHES: CommentStyle = CommentStyle { line: Some("--"), block: None };
const MARKUP: CommentStyle = CommentStyle { line: None, block: Some(("<!--", "-->")) };
const CSS: CommentStyle = CommentStyle { line: None, block: Some(("/*", "*/")) };
const NONE: CommentStyle = CommentStyle { line: None, block: None };

pub fn comment_style(language: &str) -> Option<CommentStyle> {
    let style = match language {
        "dart" | "javascript" | "typescript" | "json" | "go" | "rust" | "java" | "kotlin" | "swift" | "c" | "cpp"
        | "csharp" | "objectivec" | "php" | "scss" | "less" | "protobuf" | "gradle" | "groovy" | "scala" => SLASHES,
        "yaml" | "python" | "ruby" | "bash" | "powershell" | "dockerfile" | "makefile" | "ini" | "properties"
        | "graphql" | "elixir" | "perl" | "r" | "nginx" => HASH,
        "sql" | "lua" | "haskell" => DASHES,
        "markdown" | "xml" | "vue" => MARKUP,
        "css" => CSS,
        "diff" | "plaintext" => NONE,
        _ => return None,
    };
    Some(style)
}

pub struct CommentEdit {
    pub range: Range<usize>,
    pub text: String,
    pub selection: Range<usize>,
}

fn line_start(text: &str, at: usize) -> usize {
    text[..at].rfind('\n').map_or(0, |i| i + 1)
}

fn line_end(text: &str, at: usize) -> usize {
    text[at..].find('\n').map_or(text.len(), |i| at + i)
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches([' ', '\t']).len()
}

fn lines_of(text: &str, sel: &Range<usize>) -> Range<usize> {
    let mut end = sel.end;
    if sel.end > sel.start && line_start(text, sel.end) == sel.end {
        end -= 1;
    }
    line_start(text, sel.start)..line_end(text, end.max(sel.start))
}

struct Shift {
    at: usize,
    col: usize,
    added: usize,
    removed: usize,
}

fn map(offset: usize, shifts: &[Shift]) -> usize {
    let mut delta: isize = 0;
    for s in shifts {
        let pos = s.at + s.col;
        if offset < pos {
            break;
        }
        if s.added > 0 {
            delta += s.added as isize;
        } else if offset >= pos + s.removed {
            delta -= s.removed as isize;
        } else {
            delta -= (offset - pos) as isize;
        }
    }
    (offset as isize + delta).max(0) as usize
}

fn line_comment(text: &str, sel: &Range<usize>, prefix: &str) -> CommentEdit {
    let range = lines_of(text, sel);
    let block = &text[range.clone()];
    let filled: Vec<&str> = block.split('\n').filter(|l| !l.trim().is_empty()).collect();
    let remove = !filled.is_empty() && filled.iter().all(|l| l.trim_start().starts_with(prefix));
    let indent = filled.iter().map(|l| indent_of(l)).min().unwrap_or(0);
    let mut out = String::with_capacity(block.len() + 8);
    let mut shifts = Vec::new();
    let mut at = range.start;
    for (i, line) in block.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if line.trim().is_empty() {
            out.push_str(line);
        } else if remove {
            let col = indent_of(line);
            let rest = &line[col + prefix.len()..];
            let removed = prefix.len() + usize::from(rest.starts_with(' '));
            out.push_str(&line[..col]);
            out.push_str(&line[col + removed..]);
            shifts.push(Shift { at, col, added: 0, removed });
        } else {
            out.push_str(&line[..indent]);
            out.push_str(prefix);
            out.push(' ');
            out.push_str(&line[indent..]);
            shifts.push(Shift { at, col: indent, added: prefix.len() + 1, removed: 0 });
        }
        at += line.len() + 1;
    }
    let selection = map(sel.start, &shifts)..map(sel.end, &shifts);
    CommentEdit { range, text: out, selection }
}

fn block_comment(text: &str, sel: &Range<usize>, open: &str, close: &str) -> CommentEdit {
    let range = if sel.is_empty() {
        let line = lines_of(text, sel);
        let start = line.start + indent_of(&text[line.clone()]);
        let end = line.start + text[line.clone()].trim_end().len();
        start..end.max(start)
    } else {
        sel.clone()
    };
    let target = &text[range.clone()];
    let trimmed = target.trim();
    if trimmed.starts_with(open) && trimmed.ends_with(close) && trimmed.len() >= open.len() + close.len() {
        let inner = &trimmed[open.len()..trimmed.len() - close.len()];
        let inner = inner.strip_prefix(' ').unwrap_or(inner);
        let inner = inner.strip_suffix(' ').unwrap_or(inner);
        let selection = if sel.is_empty() {
            let c = sel.start.saturating_sub(open.len() + 1).max(range.start);
            c..c
        } else {
            range.start..range.start + inner.len()
        };
        return CommentEdit { range, text: inner.to_owned(), selection };
    }
    let wrapped = format!("{open} {target} {close}");
    let selection = if sel.is_empty() {
        let c = if sel.start >= range.start { sel.start + open.len() + 1 } else { sel.start };
        c..c
    } else {
        range.start..range.start + wrapped.len()
    };
    CommentEdit { range, text: wrapped, selection }
}

pub fn toggle_comment(text: &str, sel: Range<usize>, style: CommentStyle) -> Option<CommentEdit> {
    match (style.line, style.block) {
        (Some(prefix), _) => Some(line_comment(text, &sel, prefix)),
        (None, Some((open, close))) => Some(block_comment(text, &sel, open, close)),
        (None, None) => None,
    }
}

impl TextEditor {
    pub fn toggle_comment(&mut self, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let Some(style) = comment_style(&self.style.language) else { return };
        let Some(edit) = toggle_comment(self.buffer.text(), self.buffer.selection(), style) else { return };
        if self.buffer.text()[edit.range.clone()] == edit.text {
            return;
        }
        self.buffer.replace(edit.range, &edit.text);
        self.buffer.select(edit.selection);
        self.edited(cx);
    }
}
