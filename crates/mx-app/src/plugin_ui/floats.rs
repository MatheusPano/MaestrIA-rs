use gpui::{
    AnyElement, AppContext, Context, DragMoveEvent, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Point, Render, StatefulInteractiveElement, Styled, Window, anchored, deferred, div, point, px,
};
use mx_plugins::floats::FloatSpot;

use super::layer::PluginLayer;
use super::surface::Surface;
use crate::app::host::update_store;
use crate::app::theme::{GAP, Mx, RADIUS};
use crate::rfw_view::BlocksMode;

#[derive(Clone)]
pub struct FloatDrag(pub String);

pub struct Grab {
    key: String,
    offset: Point<f32>,
    at: Option<(f32, f32)>,
    size: (f32, f32),
}

struct Ghost;

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

pub struct Area {
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

pub fn area(window: &Window, status_top: bool) -> Area {
    let size = window.viewport_size();
    let bar = crate::status_bar::HEIGHT;
    Area {
        top: if status_top { bar } else { 0. },
        width: f32::from(size.width),
        height: f32::from(size.height) - bar,
    }
}

fn commit(layer: &mut PluginLayer, a: &Area, cx: &mut Context<PluginLayer>) {
    let Some(grab) = layer.grab.take() else { return };
    let Some((x, y)) = grab.at else { return };
    let (w, h) = grab.size;
    let spot = FloatSpot::dropped(x as f64, (y - a.top) as f64, w as f64, h as f64, a.width as f64, a.height as f64, GAP as f64);
    let host = layer.host.clone();
    cx.defer(move |cx| update_store(&host, cx, |s| s.move_float(&grab.key, spot)));
}

pub fn render(layer: &mut PluginLayer, window: &mut Window, cx: &mut Context<PluginLayer>) -> Vec<AnyElement> {
    let store = &layer.host.read(cx).store;
    let a = area(window, store.prefs().status_bar_top);
    if layer.grab.as_ref().is_some_and(|g| g.at.is_some()) && !cx.has_active_drag() {
        commit(layer, &a, cx);
    }
    let store = &layer.host.read(cx).store;
    let registry = store.plugins().cloned();
    let ui = store.plugin_ui();
    let floats: Vec<_> = ui
        .floats
        .shown()
        .iter()
        .filter(|f| registry.as_ref().and_then(|r| r.get(&f.view.plugin_id)).is_none_or(|p| p.crash.is_none()))
        .map(|f| (f.key(), f.tab_id.clone(), f.view.clone(), f.width as f32, f.height as f32, ui.floats.spot_of(&f.key()), f.start))
        .collect();
    let env = layer.env(cx);
    layer.floats.retain(|tab, _| floats.iter().any(|f| &f.1 == tab));
    let mx = Mx::get(cx).clone();
    let mut out = Vec::new();
    for (key, tab, view, w, h, spot, start) in floats {
        let surface = layer.floats.entry(tab.clone()).or_insert_with(|| Surface::new(&tab, BlocksMode::Bare).overlaid());
        surface.sync(&view, &env, cx);
        let spot = spot.unwrap_or(FloatSpot { corner: start, dx: GAP as f64, dy: GAP as f64 });
        let (ox, oy) = spot.origin(w as f64, h as f64, a.width as f64, a.height as f64, GAP as f64);
        let dragging = layer.grab.as_ref().filter(|g| g.key == key);
        let (x, y) = dragging.and_then(|g| g.at).unwrap_or((ox as f32, oy as f32 + a.top));
        let lifted = dragging.is_some_and(|g| g.at.is_some());
        let body = surface.element();
        let (grab_key, move_key) = (key.clone(), key.clone());
        let card = div()
            .id(gpui::ElementId::Name(format!("float:{key}").into()))
            .w(px(w))
            .h(px(h))
            .rounded(px(RADIUS))
            .overflow_hidden()
            .bg(mx.bg)
            .border_1()
            .border_color(if lifted { mx.accent } else { mx.border })
            .shadow(mx.card_shadow(18., 6.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |l, ev: &gpui::MouseDownEvent, _, _| {
                    let offset = point(f32::from(ev.position.x) - x, f32::from(ev.position.y) - y);
                    l.grab = Some(Grab { key: grab_key.clone(), offset, at: None, size: (w, h) });
                }),
            )
            .on_drag(FloatDrag(key.clone()), |_, _, _, cx| cx.new(|_| Ghost))
            .on_drag_move(cx.listener(move |l, ev: &DragMoveEvent<FloatDrag>, _, cx| {
                if ev.drag(cx).0 != move_key {
                    return;
                }
                let Some(g) = l.grab.as_mut().filter(|g| g.key == move_key) else { return };
                let p = ev.event.position;
                g.at = Some((f32::from(p.x) - g.offset.x, f32::from(p.y) - g.offset.y));
                cx.notify();
            }))
            .child(body);
        out.push(deferred(anchored().position(point(px(x), px(y))).child(card)).with_priority(1).into_any_element());
    }
    out
}
