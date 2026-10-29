use gpui::{
    App, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use mx_core::model::{ClaudeStatus, Tab, TabKind};
use mx_store::Store;

use crate::app::avatar::claude_avatar;
use crate::app::host::{StoreHost, update_store};
use crate::app::theme::Mx;
use crate::chrome::anim::slots;
use crate::status_bar::{HEIGHT, INSET};

pub struct SessionsView {
    host: Entity<StoreHost>,
}

impl SessionsView {
    pub fn new(host: Entity<StoreHost>) -> SessionsView {
        SessionsView { host }
    }
}

impl Render for SessionsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        slots::forget(cx.entity_id(), cx);
        let mx = Mx::get(cx).clone();
        let store = &self.host.read(cx).store;
        let top = store.prefs().status_bar_top;
        let list = SessionList::of(store);
        let close_host = self.host.clone();
        let pos = div().absolute().left(px(INSET));
        let pos = if top { pos.top(px(8.)) } else { pos.bottom(px(8.)) };
        div().size_full().relative().child(
            pos.on_mouse_down_out(move |ev, window, cx: &mut App| {
                let y = f32::from(ev.position.y);
                let h = f32::from(window.viewport_size().height);
                let on_bar = if top { y < HEIGHT } else { y > h - HEIGHT };
                if !on_bar {
                    update_store(&close_host, cx, Store::close_sessions)
                }
            })
            .child(list.render(&mx, &self.host)),
        )
    }
}

struct Row {
    id: String,
    title: String,
    meta: String,
    status: ClaudeStatus,
    focused: bool,
    hibernated: bool,
}

#[derive(Default)]
pub struct SessionList {
    blocks: Vec<(&'static str, Vec<Row>)>,
}

fn block_of(t: &Tab) -> usize {
    let status = t.status();
    if !t.done && status.needs_human() {
        0
    } else if matches!(status, ClaudeStatus::Working | ClaudeStatus::Tool) {
        1
    } else {
        2
    }
}

impl SessionList {
    pub fn of(store: &Store) -> SessionList {
        let labels = ["Esperando você", "Trabalhando", "Abertas"];
        let mut blocks: Vec<(&'static str, Vec<Row>)> =
            labels.iter().map(|l| (*l, Vec::new())).collect();
        let focused = store.focused_tab_id();
        for t in store
            .tabs()
            .iter()
            .filter(|t| t.kind == TabKind::Claude && !t.exited)
        {
            let folder = store.folder_of(t);
            let sub = store.subtitle(t);
            let meta = if folder.is_loose {
                sub
            } else {
                format!("{} · {sub}", folder.name)
            };
            blocks[block_of(t)].1.push(Row {
                id: t.id.clone(),
                title: store.title(t),
                meta,
                status: t.status(),
                focused: focused.as_deref() == Some(t.id.as_str()),
                hibernated: t.hibernated,
            });
        }
        blocks.retain(|(_, rows)| !rows.is_empty());
        SessionList { blocks }
    }

    pub fn render(self, mx: &Mx, host: &Entity<StoreHost>) -> impl IntoElement {
        let card = div()
            .id("session-list")
            .w(px(420.))
            .max_h(px(520.))
            .overflow_y_scroll()
            .bg(mx.bg_sidebar)
            .rounded(px(8.))
            .border_1()
            .border_color(mx.border)
            .shadow(mx.card_shadow(18., 6.))
            .pb(px(6.));
        if self.blocks.is_empty() {
            return card.child(
                div()
                    .p(px(16.))
                    .text_size(px(12.))
                    .text_color(mx.fg_faint)
                    .child("Nenhuma sessão do Claude aberta"),
            );
        }
        let mut card = card;
        for (label, rows) in self.blocks {
            card = card.child(
                div()
                    .pt(px(10.))
                    .px(px(12.))
                    .pb(px(6.))
                    .text_size(px(10.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(mx.fg_faint)
                    .child(format!("{label}  {}", rows.len())),
            );
            for row in rows {
                card = card.child(session_row(mx, host, row));
            }
        }
        card
    }
}

fn session_row(mx: &Mx, host: &Entity<StoreHost>, row: Row) -> impl IntoElement {
    let host = host.clone();
    let id = row.id.clone();
    let color = mx.status(row.status);
    div()
        .id(SharedString::from(format!("session:{}", row.id)))
        .mx(px(6.))
        .my(px(1.))
        .rounded(px(6.))
        .pl(px(10.))
        .pr(px(12.))
        .py(px(9.))
        .flex()
        .items_center()
        .cursor_pointer()
        .when(row.focused, |d| d.bg(mx.bg_active))
        .when(!row.focused, |d| d.hover(|s| s.bg(mx.bg_hover)))
        .on_click(move |_, _, cx: &mut App| update_store(&host, cx, |s| s.open_session(&id)))
        .child(claude_avatar(mx, row.status, 20., false, row.hibernated))
        .child(
            div()
                .ml(px(12.))
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(mx.fg)
                        .truncate()
                        .child(row.title),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(mx.fg_faint)
                        .truncate()
                        .child(row.meta),
                ),
        )
        .child(
            div()
                .ml(px(10.))
                .text_size(px(11.5))
                .text_color(color)
                .child(row.status.label()),
        )
}
