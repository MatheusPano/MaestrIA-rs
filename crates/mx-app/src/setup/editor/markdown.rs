use super::syntax::{Spans, Tone};

fn lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut start = 0;
    text.split('\n').map(move |l| {
        let at = start;
        start += l.len() + 1;
        (at, l)
    })
}

fn closing(rest: &str, open: &str) -> Option<usize> {
    rest[open.len()..].find(open).filter(|e| *e > 0).map(|e| e + 2 * open.len())
}

fn inline(at: usize, line: &str, out: &mut Spans) {
    let mut i = 0;
    while i < line.len() {
        let rest = &line[i..];
        let mut hit = None;
        if let Some(after) = rest.strip_prefix('`') {
            hit = after.find('`').map(|e| (e + 2, Tone::String));
        } else if rest.starts_with("**") || rest.starts_with("__") {
            hit = closing(rest, &rest[..2]).map(|n| (n, Tone::Strong));
        } else if (rest.starts_with('*') || rest.starts_with('_')) && !rest[1..].starts_with([' ', '*', '_']) {
            hit = closing(rest, &rest[..1]).map(|n| (n, Tone::Emphasis));
        } else if rest.starts_with('[')
            && let Some(m) = rest.find("](")
            && let Some(e) = rest[m..].find(')')
        {
            out.push((at + i..at + i + m + 1, Tone::String));
            out.push((at + i + m + 1..at + i + m + e + 1, Tone::Link));
            i += m + e + 1;
            continue;
        }
        match hit {
            Some((len, tone)) => {
                out.push((at + i..at + i + len, tone));
                i += len;
            }
            None => i += rest.chars().next().map_or(1, char::len_utf8),
        }
    }
}

fn bullet(trimmed: &str) -> usize {
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
        return 1;
    }
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && trimmed[digits..].starts_with(". ") { digits + 1 } else { 0 }
}

pub fn markdown(text: &str) -> Spans {
    let mut out = Vec::new();
    let mut fenced = false;
    for (at, line) in lines(text) {
        let whole = at..at + line.len();
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            out.push((whole, Tone::String));
        } else if fenced {
            out.push((whole, Tone::String));
        } else if trimmed.starts_with('#') {
            out.push((whole, Tone::Section));
        } else if trimmed.starts_with('>') {
            out.push((whole, Tone::Comment));
        } else {
            let marker = bullet(trimmed);
            if marker > 0 {
                out.push((at + indent..at + indent + marker, Tone::Bullet));
            }
            inline(at + indent + marker, &trimmed[marker..], &mut out);
        }
    }
    out
}

pub fn diff(text: &str) -> Spans {
    lines(text)
        .filter_map(|(at, line)| {
            let tone = if line.starts_with("+++") || line.starts_with("---") || line.starts_with("diff ") {
                Tone::Comment
            } else if line.starts_with("@@") {
                Tone::Meta
            } else if line.starts_with('+') {
                Tone::Added
            } else if line.starts_with('-') {
                Tone::Removed
            } else {
                return None;
            };
            Some((at..at + line.len(), tone))
        })
        .collect()
}
