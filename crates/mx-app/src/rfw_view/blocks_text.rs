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

fn inline(text: &str, code_bg: Hsla) -> StyledText {
    let mut out = String::new();
    let mut marks: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let (marker, style) = if rest.starts_with("**") {
            ("**", HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() })
        } else if rest.starts_with('`') {
            ("`", HighlightStyle { background_color: Some(code_bg), ..Default::default() })
        } else {
            let ch = rest.chars().next().unwrap_or(' ');
            out.push(ch);
            rest = &rest[ch.len_utf8()..];
            continue;
        };
        let body = &rest[marker.len()..];
        match body.find(marker) {
            Some(end) => {
                let start = out.len();
                out.push_str(&body[..end]);
                marks.push((start..out.len(), style));
                rest = &body[end + marker.len()..];
            }
            None => {
                out.push_str(marker);
                rest = body;
            }
        }
    }
    StyledText::new(out).with_highlights(marks)
}

pub fn markdown(p: &Paint, text: &str) -> AnyElement {
    let th = &p.view.theme;
    let s = p.size();
    let code_bg = hsla(th.bg_active);
    let mut col = div().flex().flex_col().gap(px(8.0)).text_size(px(s)).line_height(relative(1.5)).text_color(hsla(th.fg));
    let mut fence: Option<String> = None;
    let mut para = String::new();
    let flush = |para: &mut String, col: gpui::Div| {
        if para.trim().is_empty() {
            para.clear();
            return col;
        }
        let el = div().child(inline(para.trim(), code_bg));
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
        } else if let Some(item) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            col = flush(&mut para, col);
            col = col.child(div().flex().flex_row().gap(px(8.0)).pl(px(4.0)).child("•").child(div().flex_1().child(inline(item, code_bg))));
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
