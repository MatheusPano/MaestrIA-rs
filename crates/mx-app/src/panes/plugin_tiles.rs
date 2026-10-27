use gpui::{
    AnyElement, App, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px,
};
use mx_plugins::rail::RailBadge;
use mx_store::Store;

use super::cards_model::{self as model, APP_WIDTH, TILE_GAP, TILE_MIN};
use super::session_cards::section_header;
use crate::app::host::{StoreHost, update_store};
use crate::app::icons::{Icon, icon};
use crate::app::theme::{Mx, RADIUS};
use crate::rfw_view::plugin_glyph;

const INSTALL: &str = "Instalar plugin";

struct Tile {
    id: String,
    name: String,
    about: String,
    icon: Option<String>,
    dir: Option<std::path::PathBuf>,
    shown: bool,
    badge: Option<RailBadge>,
}

fn tiles(store: &Store) -> Vec<Tile> {
    let Some(r) = store.plugins() else { return Vec::new() };
    let shown = store.shown_plugin().filter(|_| !store.prefs().sidebar_hidden);
    store
        .rail_plugins()
        .into_iter()
        .filter_map(|id| {
            let p = r.get(&id)?;
            let manifest = p.manifest.as_ref();
            Some(Tile {
                name: p.name().to_owned(),
                about: manifest.map(|m| m.description.clone()).unwrap_or_default(),
                icon: manifest.and_then(|m| m.icon.clone()),
                dir: Some(p.dir.clone()),
                shown: shown.as_deref() == Some(id.as_str()),
                badge: store.rail_badge(&id).filter(|b| !matches!(b, RailBadge::Windows(_))),
                id,
            })
        })
        .collect()
}

pub fn count(store: &Store) -> usize {
    store.rail_plugins().len()
}

fn badge(mx: &Mx, b: &RailBadge) -> AnyElement {
    match b {
        RailBadge::Said(text) => div()
            .px(px(6.))
            .py(px(2.))
            .rounded(px(8.))
            .bg(mx.bg_active)
            .text_size(px(10.5))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(mx.fg)
            .child(text.clone())
            .into_any_element(),
        _ => div().size(px(8.)).rounded(px(4.)).bg(mx.accent).into_any_element(),
    }
}

fn tile(mx: &Mx, host: &Entity<StoreHost>, t: Tile, width: f32, large: bool) -> AnyElement {
    let border = if t.shown { mx.accent.opacity(0.35) } else { mx.border };
    let glyph_color = if t.shown { mx.fg } else { mx.fg_dim };
    let (accent, hover) = (mx.accent, mx.bg_hover);
    let shadow = mx.card_shadow(20., 8.);
    let host = host.clone();
    let id = t.id.clone();
    let base = div()
        .id(SharedString::from(format!("tile:{}", t.id)))
        .relative()
        .w(px(width))
        .bg(mx.bg_sidebar)
        .border_1()
        .border_color(border)
        .cursor_pointer()
        .on_click(move |_, _, cx: &mut App| update_store(&host, cx, |s| model::open_tile(s, &id)));
    let badge = t.badge.as_ref().map(|b| badge(mx, b));
    if large {
        return base
            .h(px(184.))
            .rounded(px(16.))
            .hover(move |s| s.bg(hover).border_color(accent.opacity(0.6)).shadow(shadow.clone()))
            .child(
                div()
                    .pl(px(16.))
                    .pr(px(16.))
                    .pt(px(22.))
                    .pb(px(14.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        div()
                            .size(px(64.))
                            .rounded(px(16.))
                            .bg(if t.shown { mx.accent.opacity(0.16) } else { mx.bg_active })
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(plugin_glyph(t.icon.as_deref(), t.dir.as_deref(), 30., glyph_color)),
                    )
                    .child(div().h(px(14.)))
                    .child(div().w_full().truncate().text_center().text_size(px(14.5)).font_weight(FontWeight::SEMIBOLD).text_color(mx.fg).child(t.name))
                    .when(!t.about.is_empty(), |d| {
                        d.child(div().h(px(4.))).child(
                            div()
                                .w_full()
                                .text_center()
                                .line_clamp(2)
                                .text_ellipsis()
                                .text_size(px(11.5))
                                .line_height(px(15.5))
                                .text_color(mx.fg_faint)
                                .child(t.about),
                        )
                    }),
            )
            .when_some(badge, |d, b| d.child(div().absolute().top(px(12.)).right(px(12.)).child(b)))
            .into_any_element();
    }
    base.h(px(58.))
        .px(px(12.))
        .rounded(px(RADIUS))
        .flex()
        .items_center()
        .hover(move |s| s.bg(hover).border_color(accent.opacity(0.6)))
        .child(
            div()
                .flex_none()
                .size(px(32.))
                .rounded(px(8.))
                .bg(mx.bg_active)
                .flex()
                .items_center()
                .justify_center()
                .child(plugin_glyph(t.icon.as_deref(), t.dir.as_deref(), 18., glyph_color)),
        )
        .child(div().w(px(10.)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().truncate().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(mx.fg).child(t.name))
                .when(!t.about.is_empty(), |d| d.child(div().truncate().text_size(px(11.)).text_color(mx.fg_faint).child(t.about))),
        )
        .when_some(badge, |d, b| d.child(div().ml(px(8.)).child(b)))
        .into_any_element()
}

fn add_tile(mx: &Mx, width: f32, large: bool) -> AnyElement {
    let (faint, dim, hover) = (mx.fg_faint, mx.fg_dim, mx.bg_hover);
    let base = div()
        .id(if large { "tile-install-large" } else { "tile-install" })
        .group("tile-install")
        .w(px(width))
        .h(px(if large { 184. } else { 58. }))
        .rounded(px(if large { 16. } else { RADIUS }))
        .border_1()
        .border_color(mx.border)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .flex()
        .items_center()
        .justify_center()
        .on_click(|_, window, cx: &mut App| crate::plugin_ui::open_catalog(window, cx));
    let label = |size: f32| div().text_size(px(size)).text_color(faint).group_hover("tile-install", move |s| s.text_color(dim)).child(INSTALL);
    if large {
        return base
            .flex_col()
            .child(
                div()
                    .size(px(64.))
                    .rounded(px(16.))
                    .border_1()
                    .border_color(mx.border)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(Icon::Add, 26., faint)),
            )
            .child(div().h(px(14.)))
            .child(label(13.))
            .child(div().h(px(4.)))
            .child(div().text_size(px(11.5)).text_color(faint).child("Do catálogo"))
            .into_any_element();
    }
    base.child(icon(Icon::Add, 16., faint)).child(div().w(px(6.))).child(label(12.)).into_any_element()
}

pub fn plugin_tiles(mx: &Mx, host: &Entity<StoreHost>, store: &Store, width: f32, large: bool) -> AnyElement {
    let list = tiles(store);
    if large {
        let w = APP_WIDTH.min(width.max(0.));
        let mut items: Vec<AnyElement> = list.into_iter().map(|t| tile(mx, host, t, w, true)).collect();
        items.push(add_tile(mx, w, true));
        return div().w(px(width)).flex().flex_wrap().justify_center().gap(px(16.)).children(items).into_any_element();
    }
    let (_, w) = model::columns(width, TILE_GAP, TILE_MIN);
    let n = list.len();
    let mut items: Vec<AnyElement> = list.into_iter().map(|t| tile(mx, host, t, w, false)).collect();
    items.push(add_tile(mx, w, false));
    div()
        .flex()
        .flex_col()
        .child(section_header(mx, "Plugins", None, n))
        .child(div().h(px(8.)))
        .child(div().flex().flex_wrap().gap(px(TILE_GAP)).children(items))
        .into_any_element()
}
