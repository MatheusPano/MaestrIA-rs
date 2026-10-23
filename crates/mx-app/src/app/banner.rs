use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, px,
};
use mx_store::Store;

use crate::app::host::{StoreHost, update_store};
use crate::app::icons::{Icon, icon};
use crate::app::theme::Mx;

pub fn banner(mx: &Mx, host: &Entity<StoreHost>, text: String, top: bool) -> impl IntoElement {
    let host = host.clone();
    let place = div().absolute().right(px(8.)).left(px(8.)).flex().justify_end();
    let place = if top { place.top(px(8.)) } else { place.bottom(px(8.)) };
    place.child(
        div()
            .max_w(px(460.))
            .bg(mx.bg_sidebar)
            .rounded(px(8.))
            .border_1()
            .border_color(mx.accent.opacity(0.7))
            .shadow(mx.card_shadow(18., 6.))
            .pl(px(14.))
            .pt(px(12.))
            .pr(px(8.))
            .pb(px(12.))
            .flex()
            .items_start()
            .child(div().pt(px(1.)).child(icon(Icon::InfoOutline, 16., mx.accent)))
            .child(
                div()
                    .ml(px(10.))
                    .mr(px(12.))
                    .flex_1()
                    .text_size(px(12.5))
                    .line_height(px(12.5 * 1.4))
                    .text_color(mx.fg)
                    .child(text),
            )
            .child(
                div()
                    .id("banner-close")
                    .size(px(20.))
                    .rounded(px(4.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(mx.bg_hover))
                    .on_click(move |_, _, cx: &mut App| update_store(&host, cx, Store::clear_banner))
                    .child(icon(Icon::Close, 14., mx.fg_dim)),
            ),
    )
}
