use gpui::{
    AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, StatefulInteractiveElement, Styled, Window, div, px, rgba,
};
use mx_store::Store;

use super::layer::{PluginLayer, full};
use super::surface::Surface;
use super::widgets::text_button;
use crate::app::host::update_store;
use crate::app::icons::{Icon, icon};
use crate::app::theme::{Mx, RADIUS};
use crate::app::tooltip::tip;
use crate::rfw_view::{BlocksMode, plugin_glyph};

pub const DOCK: &str = "abrir como painel";
pub const CLOSE_TIP: &str = "fechar (esc)";
const BACKDROP: u32 = 0x0000_006b;

pub fn card_size(w: f32, h: f32) -> (f32, f32) {
    let width = (w - 48.).clamp(320., 820.);
    let height = (h - 48.).min(h * 0.88).max(240.);
    (width, height)
}

pub fn render(layer: &mut PluginLayer, window: &mut Window, cx: &mut Context<PluginLayer>) -> Option<AnyElement> {
    let store = &layer.host.read(cx).store;
    let Some((tab, view)) = store.modal().map(|(t, v)| (t.id.clone(), v.clone())) else {
        layer.modal = None;
        return None;
    };
    let info = store.plugins().and_then(|r| r.get(&view.plugin_id));
    let glyph_name = info.as_ref().and_then(|p| p.manifest.as_ref().and_then(|m| m.icon.clone()));
    let dir = info.map(|p| p.dir);
    let env = layer.env(cx);
    let fresh = layer.modal.as_ref().is_none_or(|(t, _)| *t != tab);
    if fresh {
        layer.modal = Some((tab.clone(), Surface::new(&tab, BlocksMode::Modal).overlaid()));
        window.focus(&layer.focus);
    }
    let surface = &mut layer.modal.as_mut()?.1;
    surface.sync(&view, &env, cx);
    let body = surface.element();
    let mx = Mx::get(cx).clone();
    let size = window.viewport_size();
    let (w, h) = card_size(f32::from(size.width), f32::from(size.height));
    let host = layer.host.clone();
    let (close_host, dock_host, key_host) = (host.clone(), host.clone(), host.clone());
    let header = div()
        .h(px(44.))
        .flex_none()
        .px(px(14.))
        .flex()
        .items_center()
        .gap(px(9.))
        .bg(mx.bg_sidebar)
        .border_b_1()
        .border_color(mx.border)
        .child(plugin_glyph(glyph_name.as_deref(), dir.as_deref(), 17., mx.fg_dim))
        .child(div().flex_none().max_w(px(360.)).truncate().text_size(px(15.)).font_weight(FontWeight::BOLD).text_color(mx.fg).child(view.title.clone()))
        .child(div().flex_1().min_w_0().truncate().text_size(px(12.)).text_color(mx.fg_dim).child(view.subtitle()))
        .child(text_button("modal-dock", DOCK, mx.accent, None, move |_, cx| update_store(&dock_host, cx, Store::dock_modal)))
        .child(
            div()
                .id("modal-close")
                .size(px(26.))
                .rounded(px(6.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(mx.bg_hover))
                .tooltip(tip(CLOSE_TIP))
                .on_click(move |_, _, cx: &mut App| update_store(&close_host, cx, Store::close_modal))
                .child(icon(Icon::CloseRounded, 16., mx.fg_faint)),
        );
    let card = div()
        .id("plugin-modal")
        .track_focus(&layer.focus)
        .on_key_down(move |ev: &KeyDownEvent, _, cx: &mut App| {
            if ev.keystroke.key == "escape" {
                update_store(&key_host, cx, Store::close_modal);
            }
        })
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .w(px(w))
        .h(px(h))
        .flex()
        .flex_col()
        .rounded(px(RADIUS))
        .overflow_hidden()
        .bg(mx.bg)
        .border_1()
        .border_color(mx.border)
        .shadow(mx.card_shadow(28., 12.))
        .child(header)
        .child(div().flex_1().min_h_0().child(body));
    let backdrop = div()
        .id("plugin-modal-backdrop")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(BACKDROP))
        .occlude()
        .on_mouse_down(MouseButton::Left, move |_, _, cx: &mut App| update_store(&host, cx, Store::close_modal))
        .child(card);
    Some(full(window, backdrop.into_any_element()).with_priority(2).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_modal_card_keeps_inside_its_bounds() {
        assert_eq!(card_size(1200., 900.), (820., 792.));
        assert_eq!(card_size(300., 200.), (320., 240.));
        assert_eq!(card_size(800., 600.), (752., 528.));
    }
}
