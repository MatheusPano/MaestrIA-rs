use gpui::{
    AnyElement, App, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, MouseButton,
    ParentElement, ScrollHandle, SharedString, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px,
};
use mx_core::model::{FeatureOrHotfix, FeatureOrHotfixKind, Folder, Tab, Workspace};
use mx_store::Store;

use crate::app::host::{StoreHost, update_store};
use crate::app::icons::{Icon, icon};
use crate::app::theme::Mx;
use crate::app::tooltip::tip;
use crate::sidebar::tab_row::TabRow;
use crate::app::menus::Target;
use crate::sidebar::widgets::{badge, chevron, count_text, menu_on, nest, right_click, row_button};

pub struct Ctx<'a> {
    pub store: &'a Store,
    pub mx: &'a Mx,
    pub host: &'a Entity<StoreHost>,
    pub scroll: Option<ScrollHandle>,
}

impl Ctx<'_> {
    fn visible(&self, t: &Tab) -> bool {
        !t.embedded && (!self.store.filtering() || self.store.matches(t))
    }

    fn rows(&self, tabs: &[&Tab], within: Option<&str>) -> Vec<AnyElement> {
        tabs.iter()
            .filter(|t| self.visible(t))
            .map(|t| TabRow::of(self.store, self.mx, t, within).render(self.mx, self.host, self.scroll.as_ref()))
            .collect()
    }

    fn folder_tint(&self, f: &Folder, ws: Option<&Workspace>) -> Hsla {
        f.tint
            .or_else(|| ws.and_then(|w| w.tint))
            .map(|t| self.mx.tint(t))
            .unwrap_or(self.mx.fg_dim)
    }

    pub fn workspace(&self, w: &Workspace) -> Option<AnyElement> {
        let mx = self.mx;
        let filtering = self.store.filtering();
        if filtering && !self.store.has_hits_in_workspace(&w.id) {
            return None;
        }
        let folders = self.store.folders_of(&w.id);
        let needs: usize = folders.iter().map(|f| self.store.needing_human(&f.root)).sum();
        let collapsed = w.collapsed && !filtering;
        let color = w.tint.map(|t| mx.tint(t)).unwrap_or(mx.accent);
        let host = self.host.clone();
        let ws_id = w.id.clone();
        let noun = if folders.len() == 1 { "pasta" } else { "pastas" };
        let header = div()
            .id(SharedString::from(format!("ws:{}", w.id)))
            .mt(px(6.))
            .pl(px(10.))
            .pr(px(8.))
            .py(px(8.))
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .rounded(px(8.))
            .hover(|s| s.bg(mx.bg_hover))
            .on_click(move |_, _, cx: &mut App| {
                update_store(&host, cx, |s| s.toggle_workspace_collapsed(&ws_id))
            })
            .on_mouse_down(MouseButton::Right, right_click(Target::Workspace(w.id.clone())))
            .child(chevron(mx, collapsed, 20.))
            .child(icon(Icon::Hexagon, 15., color))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(13.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(mx.fg)
                    .child(w.name.clone()),
            )
            .when(collapsed && needs > 0, |d| d.child(badge(mx, needs, None)))
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(mx.fg_faint)
                    .child(format!("{} {noun}", folders.len())),
            )
            .child(row_button(
                mx,
                SharedString::from(format!("ws-more:{}", w.id)),
                Icon::MoreHoriz,
                "O que fazer com esse workspace",
                menu_on(Target::Workspace(w.id.clone())),
            ));
        let mut col = div().flex().flex_col().child(header);
        if !collapsed {
            let mut kids: Vec<AnyElement> = folders
                .iter()
                .filter_map(|f| self.folder(f, Some(w)))
                .collect();
            if folders.is_empty() {
                kids.push(
                    div()
                        .px(px(12.))
                        .py(px(8.))
                        .text_size(px(11.5))
                        .text_color(mx.fg_faint)
                        .child("Arraste uma pasta pra cá")
                        .into_any_element(),
                );
            }
            let line = w.tint.map(|t| mx.tint(t)).unwrap_or(mx.border);
            col = col.child(nest(20., line, kids));
        }
        Some(col.into_any_element())
    }

    pub fn folder(&self, f: &Folder, ws: Option<&Workspace>) -> Option<AnyElement> {
        let mx = self.mx;
        let filtering = self.store.filtering();
        if filtering && !self.store.has_hits(&f.root) {
            return None;
        }
        let within = ws.map(|w| w.id.as_str());
        let collapsed = self.store.is_folder_collapsed(&f.root, within) && !filtering;
        let tabs = self.store.tabs_of(&f.root);
        let needs = self.store.needing_human(&f.root);
        let color = self.folder_tint(f, ws);
        let host = self.host.clone();
        let root = f.root.clone();
        let within_owned = within.map(str::to_owned);
        let key = format!("{}@{}", f.root, within.unwrap_or(""));
        let glyph = div()
            .relative()
            .flex_none()
            .size(px(18.))
            .child(icon(
                if f.is_repo {
                    Icon::Folder
                } else {
                    Icon::FolderOutlined
                },
                18.,
                color,
            ))
            .when(f.is_repo, |d| {
                d.child(
                    div()
                        .absolute()
                        .right(px(-3.))
                        .bottom(px(-3.))
                        .size(px(11.))
                        .rounded_full()
                        .bg(mx.bg_sidebar)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(Icon::CallSplit, 8., mx.purple)),
                )
            });
        let mut name = div().flex_1().min_w_0().flex().flex_col().child(
            div()
                .truncate()
                .text_size(px(13.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(mx.fg)
                .child(f.name.clone()),
        );
        if !f.branch.is_empty() {
            name = name.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(3.))
                    .child(icon(Icon::CallSplit, 10., mx.fg_faint))
                    .child(
                        div()
                            .truncate()
                            .font_family("Hack")
                            .text_size(px(10.5))
                            .text_color(mx.fg_faint)
                            .child(f.branch.clone()),
                    ),
            );
        }
        let ghosts = self.store.worktrees_of(&f.root).iter().filter(|w| w.prunable).count();
        let target = Target::Folder { root: f.root.clone(), within: within.map(str::to_owned) };
        let plus = Target::Plus { root: f.root.clone(), feature: None };
        let header = div()
            .id(SharedString::from(format!("folder:{key}")))
            .group("folder-row")
            .mt(px(6.))
            .pl(px(10.))
            .pr(px(8.))
            .py(px(8.))
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .rounded(px(8.))
            .hover(|s| s.bg(mx.bg_hover))
            .on_click(move |_, _, cx: &mut App| {
                update_store(&host, cx, |s| {
                    s.toggle_folder_collapsed(&root, within_owned.as_deref())
                })
            })
            .on_mouse_down(MouseButton::Right, right_click(target.clone()))
            .child(chevron(mx, collapsed, 20.))
            .child(glyph)
            .child(div().w(px(3.)))
            .child(name)
            .when(ghosts > 0, |d| d.child(ghost_chip(mx, ghosts)))
            .when(needs > 0, |d| d.child(badge(mx, needs, None)))
            .child(count_text(mx, tabs.len()))
            .child(
                div()
                    .when(!tabs.is_empty(), |d| {
                        d.invisible().group_hover("folder-row", |s| s.visible())
                    })
                    .child(row_button(
                        mx,
                        SharedString::from(format!("folder-add:{key}")),
                        Icon::Add,
                        "Abrir aqui",
                        menu_on(plus),
                    )),
            )
            .child(row_button(
                mx,
                SharedString::from(format!("folder-more:{key}")),
                Icon::MoreHoriz,
                "O que fazer com essa pasta",
                menu_on(target),
            ));
        let mut col = div().flex().flex_col().child(header);
        if !collapsed {
            col = col.child(nest(20., color.opacity(0.35), self.folder_body(f, &tabs, within)));
        }
        Some(col.into_any_element())
    }

    pub fn folder_body(&self, f: &Folder, tabs: &[&Tab], within: Option<&str>) -> Vec<AnyElement> {
        let mut kids: Vec<AnyElement> = Vec::new();
        for feat in self.store.features_of(&f.root) {
            let inside: Vec<&Tab> = tabs
                .iter()
                .copied()
                .filter(|t| t.feature_or_hotfix_id.as_deref() == Some(feat.id.as_str()))
                .collect();
            if let Some(el) = self.feature(feat, &inside, within) {
                kids.push(el);
            }
        }
        let loose: Vec<&Tab> = tabs
            .iter()
            .copied()
            .filter(|t| {
                t.feature_or_hotfix_id
                    .as_deref()
                    .is_none_or(|id| self.store.feature_by_id(id).is_none())
            })
            .collect();
        kids.extend(self.rows(&loose, within));
        kids.push(div().h(px(8.)).into_any_element());
        kids
    }

    fn feature(&self, f: &FeatureOrHotfix, tabs: &[&Tab], within: Option<&str>) -> Option<AnyElement> {
        let mx = self.mx;
        let filtering = self.store.filtering();
        if filtering && !tabs.iter().any(|t| self.visible(t)) {
            return None;
        }
        let collapsed = f.collapsed && !filtering;
        let color = f.tint.map(|t| mx.tint(t)).unwrap_or(mx.purple);
        let glyph = match f.kind {
            FeatureOrHotfixKind::Feature => Icon::TrackChanges,
            FeatureOrHotfixKind::Hotfix => Icon::Bolt,
        };
        let needs = self.store.needing_human_in(&f.id);
        let host = self.host.clone();
        let id = f.id.clone();
        let drop_host = self.host.clone();
        let drop_id = f.id.clone();
        let root = f.folder_root.clone();
        let accent = mx.accent;
        let hover = mx.bg_hover;
        let header = div()
            .id(SharedString::from(format!("feature:{}:{}", f.id, within.unwrap_or(""))))
            .mx(px(7.))
            .pl(px(7.))
            .pr(px(8.))
            .pt(px(8.))
            .pb(px(4.))
            .rounded(px(8.))
            .border_1()
            .border_color(gpui::transparent_black())
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .hover(|s| s.bg(hover))
            .on_click(move |_, _, cx: &mut App| {
                update_store(&host, cx, |s| s.toggle_feature_collapsed(&id))
            })
            .on_mouse_down(MouseButton::Right, right_click(Target::Feature(f.id.clone())))
            .drag_over::<crate::app::drag::DraggedTab>(move |s, d, _, _| {
                if d.folder_root == root {
                    s.bg(hover).border_color(accent)
                } else {
                    s
                }
            })
            .on_drop(move |d: &crate::app::drag::DraggedTab, _, cx: &mut App| {
                update_store(&drop_host, cx, |s| s.assign(&d.id, Some(&drop_id)))
            })
            .child(chevron(mx, collapsed, 16.))
            .child(icon(glyph, 15., color))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(12.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(mx.fg)
                    .child(f.name.clone()),
            )
            .when(f.kind == FeatureOrHotfixKind::Hotfix, |d| {
                d.child(div().text_size(px(10.5)).text_color(mx.fg_faint).child("hotfix"))
            })
            .when(!f.brief.trim().is_empty(), |d| {
                d.child(
                    div()
                        .id(SharedString::from(format!("brief:{}", f.id)))
                        .tooltip(tip(f.brief.clone()))
                        .child(icon(Icon::StickyNote, 13., mx.fg_faint)),
                )
            })
            .when(needs > 0, |d| d.child(badge(mx, needs, None)))
            .child(count_text(mx, tabs.len()))
            .child(row_button(
                mx,
                SharedString::from(format!("feature-add:{}:{}", f.id, within.unwrap_or(""))),
                Icon::Add,
                "Abrir aqui",
                menu_on(Target::Plus { root: f.folder_root.clone(), feature: Some(f.id.clone()) }),
            ))
            .child(row_button(
                mx,
                SharedString::from(format!("feature-more:{}:{}", f.id, within.unwrap_or(""))),
                Icon::MoreHoriz,
                format!("O que fazer com {}", f.kind.this_one()),
                menu_on(Target::Feature(f.id.clone())),
            ));
        let mut col = div().flex().flex_col().child(header);
        if !collapsed {
            col = col.child(nest(15., color.opacity(0.35), self.rows(tabs, within)));
        }
        Some(col.into_any_element())
    }
}

fn ghost_chip(mx: &Mx, n: usize) -> impl IntoElement {
    let text = if n == 1 {
        "Uma sem pasta no disco".to_owned()
    } else {
        format!("{n} sem pasta no disco")
    };
    div()
        .id("ghosts")
        .flex()
        .items_center()
        .gap(px(2.))
        .px(px(4.))
        .rounded(px(5.))
        .bg(mx.yellow.opacity(0.14))
        .tooltip(tip(text))
        .child(icon(Icon::LinkOff, 11., mx.yellow))
        .child(
            div()
                .text_size(px(10.))
                .font_weight(FontWeight::BOLD)
                .text_color(mx.yellow)
                .child(n.to_string()),
        )
}
