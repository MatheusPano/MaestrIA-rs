use std::rc::Rc;

use gpui::{
    AnyElement, App, BoxShadow, Context, ElementId, Hsla, InteractiveElement, ParentElement, Pixels, Point,
    StatefulInteractiveElement, Styled, Window, anchored, deferred, div, point, prelude::*, px,
};
use mx_theme::Palette;

use super::icons::named;
use super::style::{alpha, hsla};

pub const MENU_WIDTH: f32 = 216.0;
const MENU_MAX_WIDTH: f32 = 340.0;
const ROW: f32 = 36.0;
const PAD: f32 = 14.0;
const GLYPH: f32 = 16.0;

#[derive(Debug, Clone, PartialEq)]
pub enum MenuRow {
    Divider,
    Item { action: String, label: String, icon: Option<&'static str>, color: Option<Hsla>, disabled: bool },
    Sub { label: String, icon: Option<&'static str>, children: Vec<(String, String, Option<Hsla>)> },
}

pub type Pick = Rc<dyn Fn(String, &mut Window, &mut App)>;

pub struct OpenMenu {
    pub at: Point<Pixels>,
    pub rows: Vec<MenuRow>,
    pub pick: Pick,
    pub sub: Option<usize>,
}

pub trait HasMenu: 'static + Sized {
    fn menu_slot(&mut self) -> &mut Option<OpenMenu>;
}

pub fn open<V: HasMenu>(view: &mut V, at: Point<Pixels>, rows: Vec<MenuRow>, pick: Pick) {
    if !rows.is_empty() {
        *view.menu_slot() = Some(OpenMenu { at, rows, pick, sub: None });
    }
}

pub fn below(at: Point<Pixels>) -> Point<Pixels> {
    point(at.x - px(MENU_WIDTH - 13.0), at.y + px(17.0))
}

pub fn color_dot(color: Option<Hsla>, p: &Palette) -> gpui::Div {
    let d = div().size(px(11.0)).rounded_full().flex_none();
    match color {
        Some(c) => d.bg(c),
        None => d.border_color(hsla(p.fg_faint)).border(px(1.4)),
    }
}

fn panel(p: &Palette) -> gpui::Div {
    div()
        .occlude()
        .min_w(px(MENU_WIDTH))
        .max_w(px(MENU_MAX_WIDTH))
        .py(px(5.0))
        .flex()
        .flex_col()
        .bg(hsla(p.bg_active))
        .border_1()
        .border_color(hsla(p.border))
        .rounded(px(10.0))
        .shadow(vec![BoxShadow {
            color: alpha(p.shadow(), 1.0),
            offset: point(px(0.0), px(6.0)),
            blur_radius: px(18.0),
            spread_radius: px(0.0),
        }])
        .text_size(px(12.0))
        .line_height(gpui::relative(1.2))
}

fn row_base(id: ElementId, p: &Palette, icon: Option<gpui::AnyElement>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(ROW))
        .px(px(PAD))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(9.0))
        .hover(|s| s.bg(hsla(p.bg_hover)))
        .child(div().w(px(GLYPH)).flex_none().flex().justify_center().children(icon))
}

pub fn render<V: HasMenu>(p: &Palette, cx: &mut Context<V>, menu: &OpenMenu) -> AnyElement {
    deferred(floating(p, cx, menu)).with_priority(2).into_any_element()
}

pub fn floating<V: HasMenu>(p: &Palette, cx: &mut Context<V>, menu: &OpenMenu) -> gpui::Anchored {
    let close = cx.listener(|v: &mut V, _: &gpui::MouseDownEvent, _, cx| {
        *v.menu_slot() = None;
        cx.notify();
    });
    let mut list = panel(p).on_mouse_down_out(close);
    for (i, row) in menu.rows.iter().enumerate() {
        list = list.child(match row {
            MenuRow::Divider => div().h(px(9.0)).px(px(PAD)).flex().items_center().child(div().h(px(1.0)).w_full().bg(hsla(p.border))).into_any_element(),
            MenuRow::Item { action, label, icon, color, disabled } => {
                let fg = if *disabled { hsla(p.fg_faint) } else { color.unwrap_or(hsla(p.fg)) };
                let glyph = icon.map(|i| named(i, 14.0, color.unwrap_or(hsla(p.fg_dim))));
                let selector = format!("menu-item:{label}");
                let mut r = row_base(ElementId::NamedInteger("menu".into(), i as u64), p, glyph)
                    .debug_selector(move || selector.clone())
                    .text_color(fg)
                    .child(div().flex_1().truncate().child(label.clone()))
                    .on_hover(cx.listener(move |v: &mut V, on: &bool, _, cx| {
                        if *on && let Some(m) = v.menu_slot() {
                            m.sub = None;
                            cx.notify();
                        }
                    }));
                if !*disabled {
                    let pick = menu.pick.clone();
                    let action = action.clone();
                    r = r.cursor_pointer().on_click(cx.listener(move |v: &mut V, _, window, cx| {
                        *v.menu_slot() = None;
                        cx.notify();
                        let (pick, action) = (pick.clone(), action.clone());
                        window.defer(cx, move |window, app| pick(action, window, app));
                    }));
                }
                r.into_any_element()
            }
            MenuRow::Sub { label, icon, children } => {
                let glyph = icon.map(|i| named(i, 14.0, hsla(p.fg_dim)));
                let mut r = row_base(ElementId::NamedInteger("menu".into(), i as u64), p, glyph)
                    .relative()
                    .text_color(hsla(p.fg))
                    .child(div().flex_1().truncate().child(label.clone()))
                    .child(named("chevron_right", 16.0, hsla(p.fg_dim)))
                    .on_hover(cx.listener(move |v: &mut V, on: &bool, _, cx| {
                        if *on && let Some(m) = v.menu_slot() {
                            m.sub = Some(i);
                            cx.notify();
                        }
                    }));
                if menu.sub == Some(i) {
                    let mut sub = panel(p).absolute().left(gpui::relative(1.0)).ml(px(-4.0)).top(px(-5.0));
                    for (j, (action, label, dot)) in children.iter().enumerate() {
                        let pick = menu.pick.clone();
                        let action = action.clone();
                        sub = sub.child(
                            row_base(ElementId::NamedInteger("sub".into(), j as u64), p, Some(color_dot(*dot, p).into_any_element()))
                                .cursor_pointer()
                                .text_color(hsla(p.fg))
                                .child(div().flex_1().truncate().child(label.clone()))
                                .on_click(cx.listener(move |v: &mut V, _, window, cx| {
                                    *v.menu_slot() = None;
                                    cx.notify();
                                    let (pick, action) = (pick.clone(), action.clone());
                                    window.defer(cx, move |window, app| pick(action, window, app));
                                })),
                        );
                    }
                    r = r.child(sub);
                }
                r.into_any_element()
            }
        });
    }
    anchored().position(menu.at).snap_to_window_with_margin(px(8.0)).child(list)
}
