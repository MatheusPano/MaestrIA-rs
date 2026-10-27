use gpui::{
    AnyElement, App, AppContext, Entity, FontWeight, Hsla, InteractiveElement, IntoElement,
    Bounds, MouseButton, ParentElement, Pixels, ScrollHandle, SharedString, StatefulInteractiveElement, Styled, div,
    canvas, point, prelude::FluentBuilder, px,
};
use mx_core::model::{ClaudeStatus, Tab, TabKind};
use mx_store::{Store, TabTint};

use crate::app::avatar::dot;
use crate::app::drag::{DraggedTab, TabGhost};
use crate::app::host::{StoreHost, update_store};
use crate::app::icons::{Icon, icon};
use crate::app::theme::{Mx, blend};
use crate::app::tooltip::tip;
use crate::app::panel_actions::open_panel_menu;
use crate::chrome::anim::reveal::{reveal_offset, reveal_overlay};
use crate::sidebar::glyph::tab_glyph;
use crate::sidebar::widgets::{badge, mini_button};

pub struct TabRow {
    id: String,
    folder_root: String,
    title: String,
    subtitle: String,
    stamp: Option<(String, bool)>,
    kind: TabKind,
    status: ClaudeStatus,
    selected: bool,
    focused: bool,
    dim: bool,
    done: bool,
    hibernated: bool,
    needs: Option<Hsla>,
    hotkey: Option<String>,
    shell_dot: Option<Hsla>,
    tint: Option<TabTint>,
    group: Option<String>,
    revealed: bool,
}

impl TabRow {
    pub fn of(store: &Store, mx: &Mx, t: &Tab, within: Option<&str>) -> TabRow {
        let now = chrono::Local::now();
        let status = t.status();
        let dim = t.done || (t.rested() && !t.unseen) || t.hibernated;
        let shell_dot = (t.kind == TabKind::Shell).then(|| {
            if !t.exited {
                mx.green
            } else if t.exit_code.unwrap_or(0) != 0 {
                mx.red
            } else {
                mx.fg_faint
            }
        });
        let revealed = store
            .revealed()
            .is_some_and(|r| r.tab_id == t.id && r.within.as_deref() == within);
        TabRow {
            id: t.id.clone(),
            folder_root: t.folder_root.clone(),
            title: store.title(t),
            subtitle: store.subtitle(t),
            stamp: t.rested_ago(&now).map(|a| (a, t.unseen)),
            kind: t.kind,
            status,
            selected: store.is_open(&t.id),
            focused: store.focused_tab_id().as_deref() == Some(t.id.as_str()),
            dim,
            done: t.done,
            hibernated: t.hibernated,
            needs: (!t.done && status.needs_human()).then(|| mx.status(status)),
            hotkey: t.hotkey.as_deref().map(mx_store::hotkey_label),
            shell_dot,
            tint: store.tint_of(&t.id),
            group: store.group_of(&t.id).map(|g| g.id.clone()),
            revealed,
        }
    }

    fn background(&self, mx: &Mx) -> Option<Hsla> {
        let grey = match (self.selected, self.focused) {
            (true, true) => Some(mx.bg_active),
            (true, false) => Some(mx.bg_hover),
            _ => None,
        };
        let Some(tint) = self.tint else { return grey };
        let a = match (tint, self.selected) {
            (TabTint::Chosen(_), true) => 0.40,
            (TabTint::Chosen(_), false) => 0.26,
            (TabTint::Group(_), true) => 0.20,
            (TabTint::Group(_), false) => 0.11,
        };
        Some(blend(mx.tab_tint(tint), a, grey.unwrap_or(mx.bg_sidebar)))
    }

    fn leading(&self, mx: &Mx) -> AnyElement {
        tab_glyph(mx, self.kind, self.status, self.done, 34.)
    }

    fn trailing(&self, mx: &Mx) -> impl IntoElement {
        let mut row = div().flex_none().flex().items_center().gap(px(6.));
        if self.done {
            row = row.child(
                div()
                    .id("done")
                    .tooltip(tip("Concluída"))
                    .child(icon(Icon::TaskAlt, 14., mx.green)),
            );
        }
        if self.hibernated {
            row = row.child(
                div()
                    .id("asleep")
                    .tooltip(tip(
                        "Hibernada — sem processo; clique pra retomar a conversa",
                    ))
                    .child(icon(Icon::Bedtime, 13., mx.fg_faint)),
            );
        }
        if let Some(color) = self.needs {
            row = row.child(badge(mx, 1, Some(color)));
        }
        if let Some(key) = &self.hotkey {
            row = row.child(
                div()
                    .pr(px(6.))
                    .font_family("Hack")
                    .text_size(px(10.5))
                    .text_color(mx.fg_faint)
                    .child(key.clone()),
            );
        }
        if let Some(color) = self.shell_dot {
            row = row.child(dot(color, 9.));
        }
        row
    }

    pub fn render(self, mx: &Mx, host: &Entity<StoreHost>, scroll: Option<&ScrollHandle>) -> AnyElement {
        let bg = self.background(mx);
        let title_color = if self.dim { mx.fg_dim } else { mx.fg };
        let weight = if self.selected {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        };
        let mut sub = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .min_w_0()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(12.))
                    .text_color(mx.fg_faint)
                    .truncate()
                    .child(self.subtitle.clone()),
            );
        if let Some((ago, unseen)) = &self.stamp {
            sub = sub.child(
                div()
                    .flex_none()
                    .text_size(px(11.))
                    .text_color(if *unseen { mx.green } else { mx.fg_faint })
                    .when(*unseen, |d| d.font_weight(FontWeight::SEMIBOLD))
                    .child(ago.clone()),
            );
        }
        let close_host = host.clone();
        let close_id = self.id.clone();
        let close = div()
            .absolute()
            .right(px(9.))
            .top_0()
            .bottom_0()
            .flex()
            .items_center()
            .invisible()
            .group_hover("tab-row", |s| s.visible())
            .child(mini_button(
                mx,
                SharedString::from(format!("close:{}", self.id)),
                Icon::CloseRounded,
                mx.fg_dim,
                "Fechar painel",
                move |_, cx| {
                    update_store(&close_host, cx, |s| {
                        s.close_tab(&close_id);
                    })
                },
            ));
        let host_click = host.clone();
        let id = self.id.clone();
        let group = self.group.clone();
        let drag = DraggedTab {
            id: self.id.clone(),
            title: self.title.clone(),
            folder_root: self.folder_root.clone(),
        };
        let host_drop = host.clone();
        let target = self.id.clone();
        let root = self.folder_root.clone();
        let accent = mx.accent;
        div()
            .id(SharedString::from(format!("tab:{}", self.id)))
            .group("tab-row")
            .relative()
            .mx(px(7.))
            .my(px(3.))
            .pl(px(11.))
            .pr(px(9.))
            .py(px(11.))
            .rounded(px(9.))
            .flex()
            .items_center()
            .cursor_pointer()
            .border_1()
            .border_color(gpui::transparent_black())
            .when_some(bg, |d, bg| d.bg(bg))
            .when(bg.is_none(), |d| d.hover(|s| s.bg(mx.bg_hover)))
            .when(self.revealed, |d| d.child(reveal_overlay(&mx.theme)))
            .when_some(scroll.filter(|_| self.revealed).cloned(), |d, handle| {
                d.child(canvas(move |row, window, _| scroll_into_view(&handle, row, window), |_, _, _, _| {}).absolute().size_full())
            })
            .on_mouse_down(MouseButton::Right, {
                let id = self.id.clone();
                move |ev, _, cx| {
                    cx.stop_propagation();
                    open_panel_menu(&id, ev.position, cx)
                }
            })
            .on_click(move |_, _, cx: &mut App| {
                update_store(&host_click, cx, |s| match &group {
                    Some(g) => s.open_group(g),
                    None => s.select(&id),
                })
            })
            .on_drag(drag, |d: &DraggedTab, at, _, cx| {
                cx.new(|_| TabGhost {
                    title: d.title.clone(),
                    at,
                })
            })
            .drag_over::<DraggedTab>(move |s, d, _, _| {
                if d.folder_root == root {
                    s.border_t(px(2.5)).border_color(accent)
                } else {
                    s
                }
            })
            .on_drop(move |d: &DraggedTab, _, cx: &mut App| {
                if d.id != target {
                    update_store(&host_drop, cx, |s| s.move_tab(&d.id, &target));
                }
            })
            .child(self.leading(mx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(14.5))
                            .font_weight(weight)
                            .text_color(title_color)
                            .truncate()
                            .child(self.title.clone()),
                    )
                    .child(sub),
            )
            .child(
                div()
                    .ml(px(8.))
                    .group_hover("tab-row", |s| s.invisible())
                    .child(self.trailing(mx)),
            )
            .child(close)
            .into_any_element()
    }
}

fn scroll_into_view(handle: &ScrollHandle, row: Bounds<Pixels>, window: &mut gpui::Window) {
    let list = handle.bounds();
    let offset = f32::from(handle.offset().y);
    let viewport = f32::from(list.size.height);
    let top = f32::from(row.origin.y - list.origin.y) - offset;
    let content = viewport + f32::from(handle.max_offset().height);
    let target = reveal_offset(top, f32::from(row.size.height), viewport, content);
    if (target + offset).abs() > 0.5 {
        handle.set_offset(point(px(0.), px(-target)));
        window.request_animation_frame();
    }
}
