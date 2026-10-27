use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use mx_rfw::widgets::text_input::{SlashCommand, completed};

use super::FieldView;

impl FieldView {
    pub(super) fn matches(&self) -> Vec<SlashCommand> {
        self.input().matches(self.buffer.text(), self.dismissed.as_deref()).into_iter().cloned().collect()
    }

    pub(super) fn picked(&self, len: usize) -> usize {
        self.pick.min(len.saturating_sub(1))
    }

    pub(super) fn step_pick(&mut self, len: usize, up: bool) {
        let at = self.picked(len) as i64 + if up { -1 } else { 1 };
        self.pick = at.rem_euclid(len as i64) as usize;
    }

    pub(super) fn complete(&mut self, c: &SlashCommand, cx: &mut Context<Self>) {
        let text = completed(c);
        let end = text.len();
        self.buffer.reset(text);
        self.buffer.set_cursor(end, false);
        self.edited(cx);
    }

    pub(super) fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.dismissed = Some(self.buffer.text().to_owned());
        cx.notify();
    }
}

pub fn list(view: &FieldView, cx: &mut Context<FieldView>) -> Option<AnyElement> {
    let matches = view.matches();
    if matches.is_empty() {
        return None;
    }
    let c = view.colors;
    let pick = view.picked(matches.len());
    let mut col = div()
        .mb(px(6.))
        .py(px(4.))
        .flex()
        .flex_col()
        .bg(c.list)
        .border_1()
        .border_color(c.border)
        .rounded(px(10.));
    for (i, m) in matches.into_iter().enumerate() {
        let on = i == pick;
        let name = div()
            .min_w(px(140.))
            .text_size(px(12.5))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if on { c.accent } else { c.fg })
            .child(format!("/{}", m.name));
        let desc = div()
            .flex_1()
            .min_w(px(0.))
            .truncate()
            .text_size(px(12.))
            .text_color(c.faint)
            .child(m.description.clone());
        let mut row = div()
            .id(("slash-command", i))
            .pl(px(10.))
            .pt(px(5.))
            .pr(px(10.))
            .pb(px(5.))
            .rounded(px(7.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .cursor_pointer()
            .child(name)
            .child(desc);
        row = if on { row.bg(c.fill) } else { row.hover(move |s| s.bg(c.hover)) };
        let chosen = m.clone();
        row = row.on_click(cx.listener(move |v, _, window, cx| {
            cx.stop_propagation();
            v.complete(&chosen, cx);
            window.focus(&v.focus);
        }));
        col = col.child(div().px(px(4.)).child(row));
    }
    Some(col.into_any_element())
}
