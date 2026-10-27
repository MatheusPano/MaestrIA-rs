use gpui::{
    App, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement,
    Styled, Window, canvas, div, px,
};

use super::cards_model as model;
use super::{plugin_tiles, session_cards};
use crate::app::drag::DraggedTab;
use crate::app::host::{StoreHost, update_store};
use crate::app::theme::Mx;

const PAD: f32 = 28.;

#[derive(IntoElement)]
pub struct EmptyPane {
    host: Entity<StoreHost>,
}

pub fn empty_pane(host: &Entity<StoreHost>) -> EmptyPane {
    EmptyPane { host: host.clone() }
}

impl RenderOnce for EmptyPane {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let measured = window.use_keyed_state("empty-pane-width", cx, |_, _| 0f32);
        let width = (*measured.read(cx) - 2. * PAD).max(0.);
        let mx = Mx::get(cx).clone();
        let store = &self.host.read(cx).store;
        let live = model::card_tabs(store).len();
        let content = if live == 0 {
            div()
                .min_h_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).text_color(mx.fg).child(model::APPS_TITLE))
                .child(div().h(px(6.)))
                .child(div().text_size(px(12.5)).text_color(mx.fg_faint).child(model::apps_subtitle(plugin_tiles::count(store))))
                .child(div().h(px(32.)))
                .child(plugin_tiles::plugin_tiles(&mx, &self.host, store, width, true))
        } else {
            div()
                .flex()
                .flex_col()
                .child(plugin_tiles::plugin_tiles(&mx, &self.host, store, width, false))
                .child(div().h(px(30.)))
                .child(session_cards::session_cards(&mx, &self.host, store, width))
                .child(
                    div()
                        .w(px(width))
                        .text_center()
                        .text_size(px(11.5))
                        .line_height(px(16.7))
                        .text_color(mx.fg_faint)
                        .child(model::DRAG_HINT),
                )
        };
        let host = self.host.clone();
        let accent = mx.accent;
        div()
            .id("empty-pane")
            .relative()
            .size_full()
            .rounded(px(10.))
            .bg(mx.bg)
            .border_1()
            .border_color(mx.border)
            .overflow_y_scroll()
            .p(px(PAD))
            .drag_over::<DraggedTab>(move |s, _, _, _| s.border_color(accent).bg(accent.opacity(0.08)))
            .on_drop(move |d: &DraggedTab, _, cx: &mut App| update_store(&host, cx, |s| s.select(&d.id)))
            .child(
                canvas(
                    move |bounds, _, cx| {
                        let w = f32::from(bounds.size.width);
                        if (w - *measured.read(cx)).abs() > 0.5 {
                            measured.update(cx, |m, cx| {
                                *m = w;
                                cx.notify();
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h(px(1.)),
            )
            .child(content)
    }
}

pub fn empty_screen(mx: &Mx, host: &Entity<StoreHost>, plugin: &str, cx: &App) -> impl IntoElement {
    let store = &host.read(cx).store;
    let info = store.plugins().and_then(|r| r.get(plugin));
    let name = info.as_ref().map_or_else(|| plugin.to_owned(), |p| p.name().to_owned());
    let glyph = info.as_ref().and_then(|p| p.manifest.as_ref().and_then(|m| m.icon.clone()));
    let dir = info.map(|p| p.dir);
    let home = store.screen_home();
    let host = host.clone();
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(14.))
        .child(crate::rfw_view::plugin_glyph(glyph.as_deref(), dir.as_deref(), 40., mx.fg_faint))
        .child(div().text_size(px(14.)).text_color(mx.fg_dim).child(mx_plugins::rail::empty_screen_text(&name)))
        .children(home.map(|cmd| {
            div()
                .id("screen-home")
                .px(px(14.))
                .py(px(7.))
                .rounded(px(8.))
                .bg(mx.accent)
                .text_color(crate::app::theme::on_color(mx.accent, mx.canvas))
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .on_click(move |_, _, cx: &mut App| update_store(&host, cx, |s| s.open_screen_home()))
                .child(cmd.title)
        }))
}
