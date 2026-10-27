mod motion;

use std::time::Instant;

use gpui::{
    AnyElement, App, Bounds, Context, Global, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, Pixels, SharedString, StatefulInteractiveElement, Styled, Window, div, px, rgba,
};
use mx_rfw::widgets::maestria::anchor_key;
use mx_store::Store;

use super::layer::{PluginLayer, full};
use super::surface::Surface;
use crate::app::host::update_store;
use crate::app::theme::{Mx, RADIUS};
use crate::app::tooltip::tip;
use crate::rfw_view::{BlocksMode, anchor_rect, named_icon, plugin_glyph};

pub use motion::card_size;
use motion::{Motion, end_rect, frame, on_screen};

pub const DOCK: &str = "abrir como painel";
pub const CLOSE_TIP: &str = "fechar (esc)";

#[derive(Default)]
struct ModalMotion {
    motion: Option<Motion>,
    shown: Option<Shown>,
}

impl Global for ModalMotion {}

fn anchor_of(plugin: &str, from: Option<&str>, viewport: (f32, f32)) -> Option<Bounds<Pixels>> {
    anchor_rect(&anchor_key(plugin, from?)).filter(|r| on_screen(*r, viewport))
}

#[derive(Clone)]
struct Shown {
    tab: String,
    title: String,
    subtitle: String,
    plugin: String,
    from: Option<String>,
    width: Option<f64>,
    height: Option<f64>,
}

fn icon_button(id: &'static str, icon: &str, tooltip: &'static str, mx: &Mx, on: impl Fn(&mut App) + 'static) -> AnyElement {
    div()
        .id(id)
        .flex_none()
        .size(px(26.))
        .rounded(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(mx.bg_hover))
        .tooltip(tip(tooltip))
        .on_click(move |_, _, cx: &mut App| on(cx))
        .child(named_icon(icon, 15., mx.fg_dim))
        .into_any_element()
}

fn sync_motion(layer: &PluginLayer, shown: Option<Shown>, viewport: (f32, f32), now: Instant, cx: &mut App) -> Option<(Motion, Shown)> {
    let global = cx.default_global::<ModalMotion>();
    let (mut m, last) = (global.motion.take(), global.shown.take());
    let store = &layer.host.read(cx).store;
    match &shown {
        Some(s) => match m.as_mut() {
            Some(mo) if mo.tab == s.tab => mo.reopen(now),
            _ => m = Some(Motion::open(&s.tab, anchor_of(&s.plugin, s.from.as_deref(), viewport), now)),
        },
        None => {
            let docked = m.as_ref().is_some_and(|mo| store.tab(&mo.tab).is_some());
            if docked || layer.modal.is_none() {
                m = None;
            } else if let Some(mo) = m.as_mut().filter(|mo| !mo.closing) {
                let from = last.as_ref().and_then(|s| anchor_of(&s.plugin, s.from.as_deref(), viewport));
                mo.close(from, now);
            }
        }
    }
    if m.as_ref().is_some_and(|mo| mo.gone(now)) {
        m = None;
    }
    let shown = if m.is_some() { shown.or(last) } else { None };
    cx.set_global(ModalMotion { motion: m.clone(), shown: shown.clone() });
    Some((m?, shown?))
}

pub fn render(layer: &mut PluginLayer, window: &mut Window, cx: &mut Context<PluginLayer>) -> Option<AnyElement> {
    let store = &layer.host.read(cx).store;
    let shown = store.modal().map(|(t, v)| Shown {
        tab: t.id.clone(),
        title: v.title.clone(),
        subtitle: v.subtitle(),
        plugin: v.plugin_id.clone(),
        from: v.modal_from.clone(),
        width: v.modal_width,
        height: v.modal_height,
    });
    let view = store.modal().map(|(_, v)| v.clone());
    let viewport = window.viewport_size();
    let viewport = (f32::from(viewport.width), f32::from(viewport.height));
    let now = Instant::now();
    let live = shown.is_some();
    let Some((motion, s)) = sync_motion(layer, shown, viewport, now, cx) else {
        layer.modal = None;
        return None;
    };
    if let (true, Some(view)) = (live, &view) {
        let fresh = layer.modal.as_ref().is_none_or(|(t, _)| *t != s.tab);
        if fresh {
            layer.modal = Some((s.tab.clone(), Surface::new(&s.tab, BlocksMode::Modal).overlaid()));
            window.focus(&layer.focus);
        }
        let env = layer.env(cx);
        if let Some((_, surface)) = layer.modal.as_mut() {
            surface.sync(view, &env, cx);
        }
    }
    let (title, subtitle) = (SharedString::from(s.title.clone()), SharedString::from(s.subtitle.clone()));
    let info = layer.host.read(cx).store.plugins().and_then(|r| r.get(&s.plugin));
    let glyph_name = info.as_ref().and_then(|p| p.manifest.as_ref().and_then(|m| m.icon.clone()));
    let dir = info.map(|p| p.dir);
    let body = layer.modal.as_ref()?.1.element();
    let mx = Mx::get(cx).clone();
    let end = end_rect(viewport, card_size(viewport, s.width, s.height));
    let t = motion.t(now);
    let f = frame(motion.from, end, t);
    if motion.moving(now) {
        window.request_animation_frame();
    }
    let closing = motion.closing;
    let host = layer.host.clone();
    let (close_host, dock_host, key_host) = (host.clone(), host.clone(), host.clone());
    let header = div()
        .h(px(44.))
        .flex_none()
        .pl(px(14.))
        .pr(px(8.))
        .flex()
        .items_center()
        .border_b_1()
        .border_color(mx.border)
        .child(plugin_glyph(glyph_name.as_deref(), dir.as_deref(), 17., mx.fg_dim))
        .child(div().w(px(9.)).flex_none())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .child(div().flex_shrink().min_w_0().truncate().text_size(px(15.)).font_weight(gpui::FontWeight::BOLD).text_color(mx.fg).child(title))
                .child(div().w(px(8.)).flex_none())
                .child(div().flex_shrink().flex_grow().min_w_0().truncate().text_size(px(12.)).text_color(mx.fg_dim).child(subtitle)),
        )
        .child(icon_button("modal-dock", "vertical_split_outlined", DOCK, &mx, move |cx| update_store(&dock_host, cx, Store::dock_modal)))
        .child(icon_button("modal-close", "close", CLOSE_TIP, &mx, move |cx| update_store(&close_host, cx, Store::close_modal)));
    let (ew, eh) = (end.size.width, end.size.height);
    let content = div()
        .absolute()
        .left((f.card.size.width - ew) / 2.)
        .top((f.card.size.height - eh) / 2.)
        .w(ew)
        .h(eh)
        .flex()
        .flex_col()
        .child(header)
        .child(div().flex_1().min_h_0().child(body));
    let card = div()
        .id("plugin-modal")
        .track_focus(&layer.focus)
        .on_key_down(move |ev: &KeyDownEvent, _, cx: &mut App| {
            if ev.keystroke.key == "escape" && !closing {
                update_store(&key_host, cx, Store::close_modal);
            }
        })
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .absolute()
        .left(f.card.origin.x)
        .top(f.card.origin.y)
        .w(f.card.size.width)
        .h(f.card.size.height)
        .opacity(f.opacity)
        .rounded(px(RADIUS + 4.))
        .overflow_hidden()
        .bg(mx.bg)
        .border_1()
        .border_color(mx.border)
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0x66 as f32 / 255.),
            offset: gpui::point(px(0.), px(16.)),
            blur_radius: px(40.),
            spread_radius: px(0.),
        }])
        .child(content);
    let alpha = (f.backdrop * 255.).round() as u32;
    let backdrop = div()
        .id("plugin-modal-backdrop")
        .absolute()
        .inset_0()
        .bg(rgba(alpha))
        .occlude()
        .on_mouse_down(MouseButton::Left, move |_, _, cx: &mut App| {
            if !closing {
                update_store(&host, cx, Store::close_modal);
            }
        })
        .child(card);
    Some(full(window, backdrop.into_any_element()).with_priority(2).into_any_element())
}
