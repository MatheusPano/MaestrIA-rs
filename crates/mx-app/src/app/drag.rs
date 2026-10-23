use gpui::{
    Context, FontWeight, IntoElement, ParentElement, Pixels, Point, Render, Styled, Window, div,
    px,
};

use crate::app::theme::Mx;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraggedTab {
    pub id: String,
    pub title: String,
    pub folder_root: String,
}

pub struct TabGhost {
    pub title: String,
    pub at: Point<Pixels>,
}

impl Render for TabGhost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mx = Mx::get(cx);
        div().pl(self.at.x - px(20.)).pt(self.at.y - px(16.)).child(
            div()
                .w(px(240.))
                .px(px(12.))
                .py(px(9.))
                .rounded(px(12.))
                .bg(mx.bg_sidebar)
                .border_1()
                .border_color(mx.accent.opacity(0.55))
                .shadow(mx.card_shadow(18., 8.))
                .text_size(px(14.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(mx.fg)
                .truncate()
                .child(self.title.clone()),
        )
    }
}
