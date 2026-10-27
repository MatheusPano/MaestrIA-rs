use std::ops::Range;

use gpui::{
    AnyElement, FontWeight, HighlightStyle, Hsla, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    StyledText, div, prelude::*, px, relative,
};
use mx_plugins::ui::console::ConsoleLine;

use super::blocks::Paint;
use super::blocks_button::tone;
use super::style::hsla;
use super::view::mono_family;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub bold: bool,
    pub code: bool,
    pub link: bool,
}

pub fn parse_inline(text: &str) -> (String, Vec<(Range<usize>, Span)>) {
    let mut out = String::new();
    let mut marks = Vec::new();
    scan(text, false, &mut out, &mut marks);
    (out, marks)
}

fn push(out: &mut String, marks: &mut Vec<(Range<usize>, Span)>, text: &str, span: Span) {
    if text.is_empty() {
        return;
    }
    let start = out.len();
    out.push_str(text);
    if span == Span::default() {
        return;
    }
    match marks.last_mut() {
        Some((r, last)) if r.end == start && *last == span => r.end = out.len(),
        _ => marks.push((start..out.len(), span)),
    }
}

fn link_at(rest: &str) -> Option<(&str, usize)> {
    let mut depth = 0usize;
    let mut close = None;
    for (i, c) in rest.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i);
                    break;
                }
            }
            '\n' => return None,
            _ => {}
        }
    }
    let close = close?;
    let target = rest[close + 1..].strip_prefix('(')?;
    let end = target.find(')')?;
    let label = &rest[1..close];
    let label = match label.strip_prefix("![").and_then(|l| l.find("](").map(|i| &l[..i])) {
        Some(alt) => alt,
        None => label,
    };
    (!label.is_empty()).then_some((label, close + 2 + end + 1))
}

fn scan(text: &str, bold: bool, out: &mut String, marks: &mut Vec<(Range<usize>, Span)>) {
    let plain = Span { bold, ..Span::default() };
    let mut rest = text;
    while let Some(ch) = rest.chars().next() {
        if let Some(body) = rest.strip_prefix('`')
            && let Some(end) = body.find('`')
        {
            push(out, marks, &body[..end], Span { code: true, ..plain });
            rest = &body[end + 1..];
        } else if !bold
            && let Some(body) = rest.strip_prefix("**")
            && let Some(end) = body.find("**")
        {
            scan(&body[..end], true, out, marks);
            rest = &body[end + 2..];
        } else if ch == '[' && let Some((label, used)) = link_at(rest) {
            push(out, marks, label, Span { link: true, ..plain });
            rest = &rest[used..];
        } else if ch == '!' && rest[1..].starts_with('[') && let Some((label, used)) = link_at(&rest[1..]) {
            push(out, marks, label, Span { link: true, ..plain });
            rest = &rest[1 + used..];
        } else {
            push(out, marks, &rest[..ch.len_utf8()], plain);
            rest = &rest[ch.len_utf8()..];
        }
    }
}

fn inline(text: &str, code_bg: Hsla, link: Hsla) -> StyledText {
    let (out, spans) = parse_inline(text);
    let marks = spans.into_iter().map(|(r, s)| {
        let mut style = HighlightStyle::default();
        if s.bold {
            style.font_weight = Some(FontWeight::BOLD);
        }
        if s.code {
            style.background_color = Some(code_bg);
        }
        if s.link {
            style.color = Some(link);
            style.underline = Some(gpui::UnderlineStyle { thickness: px(1.0), color: Some(link), wavy: false });
        }
        (r, style)
    });
    StyledText::new(out).with_highlights(marks)
}

fn list_item(t: &str) -> Option<(String, &str)> {
    if let Some(item) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
        return Some(("•".to_owned(), item));
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 9 {
        return None;
    }
    let after = &t[digits..];
    let item = after.strip_prefix(". ").or_else(|| after.strip_prefix(") "))?;
    Some((format!("{}.", &t[..digits]), item))
}

pub fn markdown(p: &Paint, text: &str) -> AnyElement {
    let th = &p.view.theme;
    let s = p.size();
    let code_bg = hsla(th.bg_active);
    let link = hsla(th.accent);
    let mut col = div().flex().flex_col().gap(px(8.0)).text_size(px(s)).line_height(relative(1.5)).text_color(hsla(th.fg));
    let mut fence: Option<String> = None;
    let mut para = String::new();
    let flush = |para: &mut String, col: gpui::Div| {
        if para.trim().is_empty() {
            para.clear();
            return col;
        }
        let el = div().child(inline(para.trim(), code_bg, link));
        para.clear();
        col.child(el)
    };
    for line in text.lines() {
        if let Some(code) = fence.as_mut() {
            if line.trim_start().starts_with("```") {
                let body = std::mem::take(code);
                fence = None;
                col = col.child(
                    div()
                        .p(px(12.0))
                        .bg(code_bg)
                        .rounded(px(8.0))
                        .font_family(mono_family())
                        .text_size(px(s - 1.7))
                        .child(body.trim_end().to_owned()),
                );
            } else {
                code.push_str(line);
                code.push('\n');
            }
            continue;
        }
        let t = line.trim_start();
        if t.starts_with("```") {
            col = flush(&mut para, col);
            fence = Some(String::new());
        } else if let Some(h) = t.strip_prefix('#') {
            col = flush(&mut para, col);
            let level = 1 + h.chars().take_while(|c| *c == '#').count();
            let size = s + [9.0, 6.0, 3.5, 2.0, 1.0, 0.5][level.min(6) - 1];
            col = col.child(div().text_size(px(size)).font_weight(FontWeight::SEMIBOLD).child(h.trim_start_matches('#').trim().to_owned()));
        } else if let Some((bullet, item)) = list_item(t) {
            col = flush(&mut para, col);
            col = col.child(div().flex().flex_row().gap(px(8.0)).pl(px(4.0)).child(bullet).child(div().flex_1().min_w(px(0.0)).child(inline(item, code_bg, link))));
        } else if t.is_empty() {
            col = flush(&mut para, col);
        } else {
            para.push_str(t);
            para.push(' ');
        }
    }
    flush(&mut para, col).into_any_element()
}

fn line(p: &Paint, l: &ConsoleLine) -> AnyElement {
    let base = if l.tone.is_none() { hsla(p.view.theme.fg) } else { tone(p, l.tone.as_deref()) };
    let el = div().text_color(base).whitespace_normal();
    let Some(spans) = &l.spans else { return el.child(l.text.clone()).into_any_element() };
    let mut text = String::new();
    let mut marks = Vec::new();
    for sp in spans {
        let start = text.len();
        text.push_str(&sp.text);
        let style = HighlightStyle {
            color: sp.tone.as_deref().map(|t| tone(p, Some(t))),
            font_weight: sp.bold.then_some(FontWeight::BOLD),
            ..Default::default()
        };
        marks.push((start..text.len(), style));
    }
    el.child(StyledText::new(text).with_highlights(marks)).into_any_element()
}

pub fn console(p: &Paint, id: &str, empty: &str) -> AnyElement {
    let th = &p.view.theme;
    let frame = div()
        .flex_1()
        .min_h(px(0.0))
        .bg(hsla(th.canvas))
        .rounded(px(8.0))
        .border_1()
        .border_color(hsla(th.border))
        .overflow_hidden();
    let lines = p.view.consoles.get(id).map(|c| &c.lines).filter(|l| !l.is_empty());
    let Some(lines) = lines else {
        return frame
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.0))
            .text_color(hsla(th.fg_faint))
            .child(empty.to_owned())
            .into_any_element();
    };
    let mut list = div()
        .id(gpui::ElementId::Name(format!("console-{id}").into()))
        .size_full()
        .overflow_y_scroll()
        .px(px(10.0))
        .py(px(8.0))
        .font_family(mono_family())
        .text_size(px(p.size() - 2.0))
        .line_height(relative(1.45));
    if let Some((handle, _)) = p.view.console_scroll.get(id) {
        list = list.track_scroll(handle);
    }
    for l in lines {
        list = list.child(line(p, l));
    }
    frame.child(list).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(text: &str) -> (String, Vec<(String, Span)>) {
        let (out, marks) = parse_inline(text);
        let named = marks.into_iter().map(|(r, s)| (out[r].to_owned(), s)).collect();
        (out, named)
    }

    #[test]
    fn code_inside_bold_is_both_bold_and_code_without_backticks() {
        let (out, marks) = spans("o else **para qualquer `$teamId` e ignorando**.");
        assert_eq!(out, "o else para qualquer $teamId e ignorando.");
        let bold = Span { bold: true, ..Span::default() };
        assert_eq!(
            marks,
            vec![
                ("para qualquer ".to_owned(), bold),
                ("$teamId".to_owned(), Span { code: true, ..bold }),
                (" e ignorando".to_owned(), bold),
            ]
        );
    }

    #[test]
    fn a_markdown_link_shows_only_its_label() {
        let (out, marks) = spans("veja [a tarefa](https://wiboor.com/t/1) agora");
        assert_eq!(out, "veja a tarefa agora");
        assert_eq!(marks, vec![("a tarefa".to_owned(), Span { link: true, ..Span::default() })]);
    }

    #[test]
    fn an_image_link_shows_the_image_name() {
        let (out, marks) = spans("anexo: [![print.png](https://x/p.png)](https://x/p.png) fim");
        assert_eq!(out, "anexo: print.png fim");
        assert_eq!(marks, vec![("print.png".to_owned(), Span { link: true, ..Span::default() })]);
        assert_eq!(spans("![foto](https://x/f.jpg)").0, "foto");
    }

    #[test]
    fn unclosed_markers_stay_literal() {
        assert_eq!(spans("2 ** 3 e `x").0, "2 ** 3 e `x");
        assert_eq!(spans("[sem link] aqui").0, "[sem link] aqui");
    }

    #[test]
    fn numbered_lines_are_list_items() {
        assert_eq!(list_item("1. Levantar"), Some(("1.".to_owned(), "Levantar")));
        assert_eq!(list_item("12) doze"), Some(("12.".to_owned(), "doze")));
        assert_eq!(list_item("- item"), Some(("•".to_owned(), "item")));
        assert_eq!(list_item("2026 foi"), None);
    }
}
