use gpui::{App, Pixels, Point, Window, point, px};
use mx_core::model::{FeatureOrHotfixKind, Folder, Tab, TabKind};
use mx_store::Store;

use crate::app::{menu_actions, ui};
use crate::keymap::{Action, keymap_of};
use crate::chrome::icon;
use crate::chrome::menu::builders::{self, FolderMenu, WorktreeRow};
use crate::chrome::menu::panel::{self, FilterPlace, PanelKind, PanelMenu};
use crate::chrome::menu::{LauncherRef, MenuAction, MenuEntry, MenuItem, Tone};

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Workspace(String),
    Folder { root: String, within: Option<String> },
    Plus { root: String, feature: Option<String> },
    Feature(String),
    Loose,
    Group(String),
    Worktrees(String),
    Worktree { root: String, path: String },
    Panel(String),
    FooterAdd,
    Filter,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Place {
    pub root: String,
    pub cwd: Option<String>,
    pub feature: Option<String>,
    pub label: Option<String>,
}

impl Place {
    pub fn at(root: &str) -> Place {
        Place { root: root.to_owned(), ..Place::default() }
    }
}

pub fn theme_tint(t: Option<mx_core::model::Tint>) -> Option<mx_theme::Tint> {
    mx_theme::Tint::by_name(t.map(|t| t.name()))
}

pub fn core_tint(t: Option<mx_theme::Tint>) -> Option<mx_core::model::Tint> {
    mx_core::model::Tint::by_name(t.map(|t| t.name()))
}

pub fn folder_or_loose(store: &Store, root: &str) -> Option<Folder> {
    if store.loose().root == root {
        Some(store.loose().clone())
    } else {
        store.find_folder(root).cloned()
    }
}

pub fn launchers(store: &Store) -> Vec<LauncherRef> {
    store
        .launchers()
        .iter()
        .map(|l| LauncherRef { id: l.id.clone(), name: l.name.clone(), icon: l.icon })
        .collect()
}

pub fn filter_label(f: mx_core::model::Filter) -> &'static str {
    use mx_core::model::Filter as F;
    match f {
        F::Claude => panel::FILTER_KINDS[0],
        F::Shell => panel::FILTER_KINDS[1],
        F::Waiting => panel::FILTER_STATES[0],
        F::Working => panel::FILTER_STATES[1],
        F::Parked => panel::FILTER_STATES[2],
        F::Finished => panel::FILTER_STATES[3],
    }
}

pub fn filter_of(label: &str) -> Option<mx_core::model::Filter> {
    mx_core::model::Filter::ALL.into_iter().find(|f| filter_label(*f) == label)
}

fn loose_menu() -> Vec<MenuEntry<MenuAction>> {
    let mut out: Vec<MenuEntry<MenuAction>> = [FeatureOrHotfixKind::Feature, FeatureOrHotfixKind::Hotfix]
        .into_iter()
        .map(|kind| {
            MenuItem::new(format!("{}…", kind.new_label()), MenuAction::NewFeatureOrHotfix(kind))
                .icon(crate::chrome::menu::actions::kind_icon(kind), Tone::Purple)
                .into()
        })
        .collect();
    out.push(MenuItem::new("Limpar concluídos", MenuAction::CloseSettled).icon(icon::CLEAR_ALL, Tone::Dim).into());
    out
}

fn worktree_rows(store: &Store, root: &str) -> Vec<WorktreeRow> {
    let name = store.find_folder(root).map(|f| f.name.clone()).unwrap_or_default();
    store
        .worktrees_of(root)
        .iter()
        .map(|w| WorktreeRow {
            path: w.path.clone(),
            label: if w.is_main { name.clone() } else { w.short_label() },
            branch: w.branch.clone(),
            is_main: w.is_main,
            prunable: w.prunable,
            session_open: store.tab_at(&w.path).is_some(),
        })
        .collect()
}

fn panel_kind(t: &Tab) -> PanelKind {
    match t.kind {
        TabKind::Shell => PanelKind::Shell,
        TabKind::Claude => PanelKind::Claude,
        TabKind::Reader => PanelKind::Reader,
        TabKind::Setup => PanelKind::Setup,
        TabKind::Plugin => PanelKind::Plugin,
    }
}

fn panel_spec(store: &Store, t: &Tab) -> PanelMenu {
    let group = store.group_of(&t.id);
    let open = store.is_open(&t.id);
    let feature = store.feature_of(t);
    let launcher = t.launcher_id.as_deref().and_then(|id| store.launcher_by_id(id));
    let keys = keymap_of(store);
    PanelMenu {
        kind: panel_kind(t),
        relaunch: launcher.filter(|_| t.exited).map(|l| (l.id.clone(), l.name.clone())),
        hibernated: t.hibernated,
        feature_tint: feature.and_then(|f| theme_tint(f.tint).map(|tint| (tint, f.kind))),
        tint: theme_tint(t.tint),
        plans: t.hooks.plans.len(),
        has_message: t.hooks.last_message_full.as_deref().is_some_and(|m| !m.trim().is_empty()),
        follow_ups: t.follow_ups.len(),
        open,
        pinned: store.is_pinned(&t.id),
        pin_chord: keys.first_label(Action::Pin),
        can_group: store.pane_count() > 1 && open && group.is_none_or(|g| !store.showing(&g.id)),
        group: group.map(|g| (g.id.clone(), g.name.clone())),
        passive: t.is_passive(),
        done: t.done,
        plugin_commands: store
            .plugins()
            .map(|p| p.commands().into_iter().map(|c| (c.full_id(), c.title.clone())).collect())
            .unwrap_or_default(),
        can_hibernate: t.can_hibernate(),
        close_chord: keys.first_label(Action::Close),
    }
}

pub fn filter_entries(store: &Store) -> Vec<MenuEntry<MenuAction>> {
    let s = store.search();
    let place = |f: &Folder| FilterPlace {
        key: f.root.clone(),
        name: f.name.clone(),
        loose: f.is_loose,
        on: s.roots.contains(&f.root),
        features: store
            .features_of(&f.root)
            .into_iter()
            .map(|x| (x.id.clone(), x.name.clone(), x.kind, s.features.contains(&x.id)))
            .collect(),
    };
    let mut places: Vec<FilterPlace> = store.folders().iter().map(place).collect();
    places.push(place(store.loose()));
    let on = |label: &str| filter_of(label).is_some_and(|f| s.flags.contains(&f));
    panel::filter_menu(&places, &on, store.filtering())
}

pub fn entries(store: &Store, target: &Target) -> Vec<MenuEntry<MenuAction>> {
    let ls = launchers(store);
    match target {
        Target::FooterAdd => builders::footer_add(),
        Target::Filter => filter_entries(store),
        Target::Loose => loose_menu(),
        Target::Plus { feature, .. } => builders::folder_plus(&ls, feature.is_some()),
        Target::Group(id) => builders::group_menu(id),
        Target::Worktrees(root) => builders::worktrees_menu(&worktree_rows(store, root)),
        Target::Worktree { root, path } => {
            let rows = worktree_rows(store, root);
            let w = rows.iter().find(|w| &w.path == path);
            builders::worktree_menu(&ls, w.is_some_and(|w| w.is_main), w.is_some_and(|w| w.prunable))
        }
        Target::Workspace(id) => store
            .workspace_by_id(id)
            .map(|w| builders::workspace_menu(w.code_workspace_path.is_some(), theme_tint(w.tint)))
            .unwrap_or_default(),
        Target::Feature(id) => store
            .feature_by_id(id)
            .map(|f| builders::feature_menu(&ls, f.kind, f.folder_root == store.loose().root, theme_tint(f.tint)))
            .unwrap_or_default(),
        Target::Panel(id) => store.tab(id).map(|t| panel::panel_menu(&panel_spec(store, t))).unwrap_or_default(),
        Target::Folder { root, within } => {
            let Some(f) = store.find_folder(root) else { return Vec::new() };
            let trees = store.worktrees_of(root);
            builders::folder_menu(&FolderMenu {
                launchers: ls,
                worktrees: trees.len(),
                branched: trees.iter().any(|w| !w.is_main),
                ghosts: trees.iter().filter(|w| w.prunable).count(),
                tint: theme_tint(f.tint),
                workspaces: store
                    .workspaces()
                    .iter()
                    .map(|w| (w.id.clone(), w.name.clone(), w.folder_roots.contains(root)))
                    .collect(),
                within_workspace: within.is_some(),
            })
        }
    }
}

pub fn open(target: Target, window: &mut Window, cx: &mut App) {
    open_at(target, window.mouse_position(), cx);
}

pub fn open_at(target: Target, at: Point<Pixels>, cx: &mut App) {
    let entries = entries(ui::store(cx), &target);
    if target == Target::Filter {
        menu_actions::filter_menu(entries, at, cx);
        return;
    }
    ui::menu(entries, at, move |action, window, cx| menu_actions::run(&target, action, window, cx), cx);
}

pub fn nudge(at: Point<Pixels>, dx: f32, dy: f32) -> Point<Pixels> {
    point(at.x + px(dx), at.y + px(dy))
}
