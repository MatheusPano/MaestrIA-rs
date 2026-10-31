use std::rc::Rc;

use gpui::{
    InteractiveElement, MouseButton, ParentElement, StatefulInteractiveElement, Styled, px,
};
use mx_rfw::widgets::Local;
use mx_rfw::widgets::maestria::{IconButton, MenuItem, Pressable, menu_extra};
use mx_rfw::{Event, Map};
use mx_theme::Palette;

use super::drag::{draggable, drop_target};
use super::icons::{glyph, named, plugin_icon, svg_element};
use super::layout::{Ctx, El, Fit, error_text, fire, owned, pass};
use super::menu::{self, MenuRow};
use super::style::{alpha, argb, hsla, set_padding, set_radius};
use super::tooltip;
use super::widgets_core::{child, optional};
use super::zoom_view::scaled;

pub fn render(cx: &mut Ctx, local: Local, fit: Fit, turns: f64) -> El {
    match local {
        Local::Svg(s) => {
            let Some(svg) = s.svg else { return cx.el() };
            let image = svg_element(&svg, s.width.map(|v| scaled(v as f32)), s.height.map(|v| scaled(v as f32)));
            cx.el().flex_none().child(image)
        }
        Local::Glyph(g) => {
            let color = g.color.map_or(hsla(cx.palette.fg_dim), argb);
            let icon = glyph(g.icon.as_deref(), cx.dir, scaled(g.size as f32), color, turns);
            cx.el().flex_none().child(icon)
        }
        Local::IconButton(b) => icon_button(cx, &b),
        Local::Tooltip(t) => {
            let inner = child(cx, t.child, fit);
            let el = pass(cx.el(), Some(inner));
            if t.message.is_empty() { el } else { el.tooltip(tooltip::builder(t.message.clone(), cx.palette)) }
        }
        Local::Pressable(p) => pressable(cx, &p, fit),
        Local::MenuButton(p) => menu_button(cx, &p),
        Local::Draggable(d) => draggable(cx, &d, fit, cx.palette.clone()),
        Local::DropTarget(d) => drop_target(cx, &d, fit),
        Local::CodeEditor(e) => super::code_editor::render(cx, &e),
        Local::Sprite(s) => super::sprite::render(cx, &s),
        Local::Picture(p) => super::picture::render(cx, &p),
        Local::ZoomView(z) => super::zoom_view::render(cx, &z, fit),
        Local::FittedBox(f) => super::zoom_view::fitted(cx, &f, fit),
        Local::TextInput(t) => super::text_input_widget::render(cx, &t),
        Local::Anchor(a) => super::anchor::render(cx, &a, fit),
        Local::Selectable(s) | Local::SelectionLines(s) => super::selectable::render(cx, &s, fit),
        Local::Markdown(m) => super::markdown_widget::render(cx, &m),
        _ => error_text(cx, "widget sem desenho", hsla(cx.palette.yellow)),
    }
}

fn round_button(cx: &mut Ctx, extent: f32, icon: gpui::AnyElement, hover: bool) -> El {
    let fg = cx.palette.fg;
    let el = cx.el().flex_none().size(px(extent)).flex().items_center().justify_center().rounded_full().child(icon);
    if hover { el.cursor_pointer().hover(move |s| s.bg(alpha(fg, 0.08))) } else { el }
}

fn icon_button(cx: &mut Ctx, b: &IconButton) -> El {
    let base = b.color.map_or(hsla(cx.palette.fg_dim), argb);
    let enabled = b.enabled() && cx.interactive;
    let mut color = base;
    if !b.enabled() {
        color.a *= 0.4;
    }
    let name = plugin_icon(b.icon.as_deref()).unwrap_or("circle_outlined");
    let icon = named(name, scaled(b.size as f32), color);
    let mut el = round_button(cx, scaled(b.extent as f32), icon, enabled);
    if let Some(t) = &b.tooltip {
        el = el.tooltip(tooltip::builder(t.clone(), cx.palette));
    }
    if enabled {
        let events = owned(&b.on_pressed);
        let sink = cx.sink.clone();
        el = el.on_click(move |_, window, app| {
            app.stop_propagation();
            fire(&sink, &events, &Map::new(), window, app);
        });
    }
    el
}

fn rows(items: &[MenuItem], p: &Palette) -> Vec<MenuRow> {
    items
        .iter()
        .map(|m| {
            if m.is_divider() {
                MenuRow::Divider
            } else if !m.children.is_empty() {
                MenuRow::Sub {
                    label: m.label.clone(),
                    icon: plugin_icon(m.icon.as_deref()),
                    children: m
                        .children
                        .iter()
                        .map(|c| {
                            let dot = (c.color != 0).then(|| argb(mx_rfw::decoders::Color(c.color as u32)));
                            (c.action.clone(), c.label.clone(), dot)
                        })
                        .collect(),
                }
            } else {
                MenuRow::Item {
                    action: m.action.clone().unwrap_or_default(),
                    label: m.shown_label().to_owned(),
                    icon: plugin_icon(m.icon.as_deref()),
                    color: m.red.then(|| hsla(p.red)),
                    disabled: m.disabled,
                }
            }
        })
        .collect()
}

fn opener(cx: &Ctx, p: &Pressable) -> impl Fn(gpui::Point<gpui::Pixels>, &mut gpui::App) + 'static {
    let rows = rows(&p.menu, cx.palette);
    let events: Vec<Event> = owned(&p.on_menu);
    let sink = cx.sink.clone();
    let view = cx.view.clone();
    move |at, app| {
        let (events, sink) = (events.clone(), sink.clone());
        let pick: menu::Pick = Rc::new(move |choice, window, app| {
            fire(&sink, &events, &menu_extra(&choice), window, app);
        });
        let rows = rows.clone();
        view.update(app, |v, cx| {
            menu::open(v, at, rows, pick);
            cx.notify();
        })
        .ok();
    }
}

fn pressable(cx: &mut Ctx, p: &Pressable, fit: Fit) -> El {
    let inner = optional(cx, p.child, fit);
    let mut el = pass(cx.el(), inner);
    if let Some(pad) = &p.padding {
        set_padding(el.style(), pad);
    }
    if p.radius > 0.0 {
        set_radius(el.style(), p.radius as f32);
    }
    if let Some(c) = p.color {
        el = el.bg(argb(c));
    }
    if !cx.interactive {
        return el;
    }
    if let Some(h) = p.hover_color {
        el = el.hover(move |s| s.bg(argb(h)));
    }
    let taps = owned(&p.on_tap);
    let doubles = owned(&p.on_double_tap);
    if !taps.is_empty() {
        el = el.cursor_pointer();
    }
    if !taps.is_empty() || !doubles.is_empty() {
        let sink = cx.sink.clone();
        el = el.on_click(move |e, window, app| {
            app.stop_propagation();
            let events = if e.click_count() >= 2 && !doubles.is_empty() { &doubles } else { &taps };
            fire(&sink, events, &Map::new(), window, app);
        });
    }
    if !p.menu.is_empty() {
        let open = opener(cx, p);
        el = el.on_mouse_down(MouseButton::Right, move |e, _, app| {
            app.stop_propagation();
            open(e.position, app);
        });
    }
    el
}

fn menu_button(cx: &mut Ctx, p: &Pressable) -> El {
    let color = p.color.map_or(hsla(cx.palette.fg_dim), argb);
    let name = plugin_icon(p.icon.as_deref()).unwrap_or("more_horiz_rounded");
    let icon = named(name, scaled(p.size as f32), color);
    let mut el = round_button(cx, scaled(26.0), icon, cx.interactive);
    if let Some(t) = &p.tooltip {
        el = el.tooltip(tooltip::builder(t.clone(), cx.palette));
    }
    if cx.interactive {
        let open = opener(cx, p);
        el = el.on_click(move |e, _, app| {
            app.stop_propagation();
            open(menu::below(e.position()), app);
        });
    }
    el
}
