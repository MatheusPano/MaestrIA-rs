use gpui::{
    AnyElement, App, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px,
};
use mx_core::model::Tab;

use crate::app::host::update_store;
use crate::app::icons::{Icon, icon};
use crate::app::confirms;
use crate::chrome::widgets::HOVER_RADIUS;
use crate::app::menus::Target;
use crate::sidebar::tree::Ctx;
use crate::sidebar::widgets::{badge, count_chip, count_text, menu_on, right_click, row_button};

pub fn settled_label(n: usize) -> String {
    if n == 1 {
        "Limpar 1 painel concluído".into()
    } else {
        format!("Limpar {n} painéis concluídos")
    }
}

pub fn group_subtitle(name: &str, summary: &str, where_: &str) -> String {
    if name.to_lowercase() == summary.to_lowercase() {
        where_.to_owned()
    } else {
        summary.to_owned()
    }
}

impl Ctx<'_> {
    pub fn groups_tray(&self) -> Option<AnyElement> {
        let groups = self.store.groups();
        if groups.is_empty() || self.store.filtering() {
            return None;
        }
        let mx = self.mx;
        let collapsed = self.store.prefs().groups_collapsed;
        let host = self.host.clone();
        let ruler = div()
            .id("groups-ruler")
            .group("groups-ruler")
            .ml(px(2.))
            .mr(px(7.))
            .pl(px(12.))
            .pr(px(7.))
            .mt(px(6.))
            .py(px(4.))
            .rounded(px(HOVER_RADIUS))
            .hover(|s| s.bg(mx.bg_hover))
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .on_click(move |_, _, cx: &mut App| {
                update_store(&host, cx, |s| s.toggle_groups_collapsed())
            })
            .child(icon(
                if collapsed {
                    Icon::ChevronRight
                } else {
                    Icon::ExpandMore
                },
                14.,
                mx.fg_faint,
            ))
            .child(div().text_size(px(10.5)).text_color(mx.fg_faint).child("Grupos"))
            .child(div().flex_1().h(px(1.)).bg(mx.border))
            .child(count_text(mx, groups.len()))
            .child(
                div()
                    .invisible()
                    .group_hover("groups-ruler", |s| s.visible())
                    .child(row_button(mx, "groups-clear", Icon::CloseRounded, "Esquecer todos os grupos", |_, cx| confirms::clear_groups(cx))),
            );
        let mut col = div().flex().flex_col().child(ruler);
        if !collapsed {
            for g in groups {
                let color = self.mx.tab_tint(mx_store::TabTint::Group(g.tint_index(mx_store::GROUP_TINTS)));
                let selected = self.store.showing(&g.id);
                let host = self.host.clone();
                let id = g.id.clone();
                col = col.child(
                    div()
                        .id(SharedString::from(format!("group:{}", g.id)))
                        .mx(px(7.))
                        .my(px(3.))
                        .pl(px(11.))
                        .pr(px(9.))
                        .py(px(9.))
                        .rounded(px(9.))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .cursor_pointer()
                        .when(selected, |d| d.bg(mx.bg_active))
                        .when(!selected, |d| d.hover(|s| s.bg(mx.bg_hover)))
                        .on_click(move |_, _, cx: &mut App| update_store(&host, cx, |s| s.open_group(&id)))
                        .on_mouse_down(MouseButton::Right, right_click(Target::Group(g.id.clone())))
                        .child(icon(Icon::GridView, 17., color))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(14.5))
                                        .text_color(mx.fg)
                                        .when(selected, |d| d.font_weight(FontWeight::SEMIBOLD))
                                        .child(g.name.clone()),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(12.))
                                        .text_color(mx.fg_faint)
                                        .child(group_subtitle(&g.name, &g.summary(), &g.where_())),
                                ),
                        )
                        .child(count_chip(mx, g.count()))
                        .child(row_button(
                            mx,
                            SharedString::from(format!("group-more:{}", g.id)),
                            Icon::MoreHoriz,
                            "O que fazer com esse grupo",
                            menu_on(Target::Group(g.id.clone())),
                        )),
                );
            }
        }
        Some(col.into_any_element())
    }

    pub fn loose_tray(&self, has_folders: bool) -> Option<AnyElement> {
        let mx = self.mx;
        let loose = self.store.loose();
        let tabs: Vec<&Tab> = self.store.tabs_of(&loose.root);
        let filtering = self.store.filtering();
        if filtering && !self.store.has_hits(&loose.root) {
            return None;
        }
        let needs = self.store.needing_human(&loose.root);
        let settled = self.store.settled_in(&loose.root);
        let clear_host = self.host.clone();
        let clear_root = loose.root.clone();
        let plus = Target::Plus { root: loose.root.clone(), feature: None };
        let ruler = div()
            .id("loose-ruler")
            .group("loose-ruler")
            .mx(px(14.))
            .mt(px(if has_folders { 22. } else { 4. }))
            .mb(px(2.))
            .flex()
            .items_center()
            .gap(px(6.))
            .child(div().text_size(px(10.5)).text_color(mx.fg_faint).child(loose.name.clone()))
            .child(div().flex_1().h(px(1.)).bg(mx.border))
            .when(needs > 0, |d| d.child(badge(mx, needs, None)))
            .child(count_text(mx, tabs.len()))
            .when(settled > 0, |d| {
                d.child(
                    div()
                        .invisible()
                        .group_hover("loose-ruler", |s| s.visible())
                        .child(row_button(mx, "loose-clear", Icon::TaskAlt, settled_label(settled), move |_, cx| {
                            update_store(&clear_host, cx, |s| {
                                s.close_settled(&clear_root);
                            })
                        })),
                )
            })
            .child(row_button(mx, "loose-add", Icon::Add, "Abrir aqui", menu_on(plus)))
            .child(row_button(mx, "loose-more", Icon::MoreHoriz, "O que fazer com os avulsos", menu_on(Target::Loose)));
        let body = self.folder_body(loose, &tabs, None);
        Some(div().flex().flex_col().child(ruler).children(body).into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settled_label_reads_singular_and_plural() {
        assert_eq!(settled_label(1), "Limpar 1 painel concluído");
        assert_eq!(settled_label(3), "Limpar 3 painéis concluídos");
    }

    #[test]
    fn group_subtitle_shows_where_when_the_name_is_the_summary() {
        assert_eq!(group_subtitle("Api + Web", "api + web", "repo"), "repo");
        assert_eq!(group_subtitle("Deploy", "api + web", "repo"), "api + web");
    }
}
