mod search;
mod tab_row;
mod trays;
mod tree;
mod widgets;

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Render, ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use mx_core::model::sidebar::RowKey;
use mx_store::Store;

use crate::app::host::{StoreHost, update_store};
use crate::app::menus::{self, Target};
use crate::app::{chats, settings};
use crate::chrome::settings::Section;
use crate::app::icons::{Icon, icon};
use crate::keymap::{Action, keymap_of};
use crate::app::theme::{Mx, RADIUS};
use crate::app::tooltip::tip;
use tree::Ctx;

pub const MIN_WIDTH: f32 = 248.;
pub const DEFAULT_WIDTH: f32 = 352.;

pub struct Sidebar {
    host: Entity<StoreHost>,
    search_focus: FocusHandle,
    scroll: ScrollHandle,
    revealed: Option<(String, Option<String>)>,
}

pub fn no_hits_text(query: &str, filters: usize) -> String {
    let mut what = Vec::new();
    if !query.trim().is_empty() {
        what.push(format!("“{}”", query.trim()));
    }
    if filters > 0 {
        what.push(format!("{filters} {}", if filters == 1 { "filtro" } else { "filtros" }));
    }
    format!("Nenhuma sessão com {}.", what.join(" + "))
}

pub const NO_FOLDERS: &str = "Nenhuma pasta ainda.\n\nSessões do Claude já rodando aparecem aqui sozinhas — ou use o ícone de pasta no rodapé pra adicionar um repo.\n\nPra abrir um painel sem pasta nenhuma, o + na linha de \"avulsos\" logo abaixo.";

impl Sidebar {
    pub fn new(host: Entity<StoreHost>, window: &mut Window, cx: &mut Context<Self>) -> Sidebar {
        let search_focus = cx.focus_handle();
        cx.on_focus(&search_focus, window, |_, _, cx| cx.notify()).detach();
        cx.on_blur(&search_focus, window, |_, _, cx| cx.notify()).detach();
        Sidebar { host, search_focus, scroll: ScrollHandle::new(), revealed: None }
    }

    pub fn focus_search(&self, window: &mut Window) {
        window.focus(&self.search_focus);
    }

    fn header(&self, mx: &Mx, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        div()
            .h(px(42.))
            .flex_none()
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(4.))
            .border_b_1()
            .border_color(mx.border)
            .child(self.search_field(mx, window, cx))
            .child(strip_icon(
                mx,
                "sidebar-settings",
                Icon::Settings,
                with_keys("Configurações", &self.host.read(cx).store, Action::Settings),
                |window, cx| settings::open(Section::Appearance, window, cx),
            ))
    }

    fn list(&mut self, mx: &Mx, cx: &Context<Self>) -> impl IntoElement {
        let store = &self.host.read(cx).store;
        let now = store.revealed().map(|r| (r.tab_id.clone(), r.within.clone()));
        let fresh = now.is_some() && now != self.revealed;
        self.revealed = now;
        let ctx = Ctx {
            store,
            mx,
            host: &self.host,
            scroll: fresh.then(|| self.scroll.clone()),
        };
        let filtering = store.filtering();
        let mut items: Vec<AnyElement> = Vec::new();
        if let Some(g) = ctx.groups_tray() {
            items.push(g);
        }
        let has_folders = !store.folders().is_empty();
        if !has_folders && !filtering {
            items.push(
                div()
                    .p(px(20.))
                    .text_size(px(12.5))
                    .line_height(px(12.5 * 1.55))
                    .text_color(mx.fg_faint)
                    .child(NO_FOLDERS)
                    .into_any_element(),
            );
        }
        if filtering && store.hits().is_empty() {
            items.push(no_hits(mx, &self.host, store));
        }
        for row in store.sidebar_rows() {
            let el = match &row {
                RowKey::Workspace(id) => store.workspace_by_id(id).and_then(|w| ctx.workspace(w)),
                RowKey::Folder(root) => store.find_folder(root).and_then(|f| ctx.folder(f, None)),
            };
            items.extend(el);
        }
        items.extend(ctx.loose_tray(has_folders));
        div()
            .id("sidebar-list")
            .track_scroll(&self.scroll)
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .pt(px(6.))
            .pb(px(20.))
            .flex()
            .flex_col()
            .children(items)
    }

    fn footer(&self, mx: &Mx, cx: &Context<Self>) -> impl IntoElement {
        let host = self.host.clone();
        let hide = with_keys("Esconder a lateral", &self.host.read(cx).store, Action::Sidebar);
        div()
            .h(px(42.))
            .flex_none()
            .px(px(7.))
            .flex()
            .items_center()
            .gap(px(4.))
            .border_t_1()
            .border_color(mx.border)
            .child(strip_icon(mx, "footer-add", Icon::CreateNewFolder, "Adicionar pasta ou workspace", |window, cx| {
                menus::open(Target::FooterAdd, window, cx)
            }))
            .child(strip_icon(mx, "footer-history", Icon::History, "Retomar uma conversa", |_, cx| drop(chats::open(cx))))
            .child(div().flex_1())
            .child(strip_icon(mx, "footer-account", Icon::Speed, "Conta & uso", |window, cx| {
                settings::open(Section::Account, window, cx)
            }))
            .child(strip_icon(
                mx,
                "footer-hide",
                Icon::ViewSidebar,
                hide,
                move |_, cx| update_store(&host, cx, Store::toggle_sidebar),
            ))
    }
}

pub fn with_keys(label: &str, store: &Store, action: Action) -> String {
    let mut parts = vec![label.to_owned()];
    parts.extend(keymap_of(store).labels(action));
    parts.join("  ")
}

fn strip_icon(
    mx: &Mx,
    id: &'static str,
    glyph: Icon,
    tooltip: impl Into<SharedString>,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex_none()
        .size(px(36.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(mx.bg_hover))
        .tooltip(tip(tooltip.into()))
        .on_click(move |_, window, cx| on_click(window, cx))
        .child(icon(glyph, 18., mx.fg_dim))
}

fn no_hits(mx: &Mx, host: &Entity<StoreHost>, store: &Store) -> AnyElement {
    let host = host.clone();
    let s = store.search();
    div()
        .p(px(20.))
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(
            div()
                .text_size(px(12.5))
                .text_color(mx.fg_faint)
                .child(no_hits_text(&s.query, store.active_filters())),
        )
        .child(
            div()
                .id("show-all")
                .flex()
                .items_center()
                .gap(px(6.))
                .cursor_pointer()
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(mx.accent)
                .tooltip(tip("Limpar a busca e os filtros"))
                .on_click(move |_, _, cx: &mut App| update_store(&host, cx, Store::clear_search))
                .child(icon(Icon::FilterListOff, 16., mx.accent))
                .child("Mostrar tudo"),
        )
        .into_any_element()
}

impl Render for Sidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mx = Mx::get(cx).clone();
        let plugin_tab = self.host.read(cx).store.shown_plugin().and_then(|_| crate::plugin_ui::sidebar_view(cx));
        let frame = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(mx.bg_sidebar)
            .rounded(px(RADIUS))
            .border_1()
            .border_color(mx.border)
            .shadow(mx.card_shadow(12., 3.))
            .overflow_hidden();
        if let Some(view) = plugin_tab {
            return frame.child(view);
        }
        frame
            .child(self.header(&mx, window, cx))
            .child(self.list(&mx, cx))
            .child(self.footer(&mx, cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_hits_names_the_query_and_the_filters() {
        assert_eq!(no_hits_text("api", 0), "Nenhuma sessão com “api”.");
        assert_eq!(no_hits_text("", 2), "Nenhuma sessão com 2 filtros.");
        assert_eq!(no_hits_text("api", 1), "Nenhuma sessão com “api” + 1 filtro.");
    }
}
