use gpui::{AnyView, App, AppContext, Window, point, px};
use mx_core::model::{FeatureOrHotfixKind, LauncherIcon};
use mx_theme::Tint;

use crate::chrome::menu::builders::{self, FolderMenu};
use crate::chrome::menu::panel::{self, PanelKind, PanelMenu};
use crate::chrome::menu::{ContextMenu, LauncherRef, MenuAction, MenuEntry};

fn launchers() -> Vec<LauncherRef> {
    vec![
        LauncherRef { id: "l1".into(), name: "btop".into(), icon: LauncherIcon::Monitor },
        LauncherRef { id: "l2".into(), name: "npm run dev".into(), icon: LauncherIcon::Server },
    ]
}

fn entries(component: &str) -> Vec<MenuEntry<MenuAction>> {
    match component {
        "menu-feature" => builders::feature_menu(&launchers(), FeatureOrHotfixKind::Feature, false, Some(Tint::Violet)),
        "menu-panel" => panel::panel_menu(&PanelMenu {
            kind: PanelKind::Claude,
            plans: 2,
            has_message: true,
            open: true,
            pin_chord: Some("Ctrl+Alt+P".into()),
            can_group: true,
            close_chord: Some("Ctrl+Shift+W".into()),
            plugin_commands: vec![("p.a".into(), "Git: status".into())],
            can_hibernate: true,
            ..Default::default()
        }),
        "menu-worktrees" => builders::worktrees_menu(&[
            builders::WorktreeRow { path: "/r".into(), label: "maestria".into(), branch: "main".into(), is_main: true, prunable: false, session_open: true },
            builders::WorktreeRow { path: "/r/a".into(), label: "TASK#47730".into(), branch: "feature/TASK#47730".into(), is_main: false, prunable: false, session_open: false },
            builders::WorktreeRow { path: "/r/b".into(), label: "TASK#1".into(), branch: "feature/TASK#1".into(), is_main: false, prunable: true, session_open: false },
        ]),
        _ => builders::folder_menu(&FolderMenu {
            launchers: launchers(),
            worktrees: 3,
            branched: true,
            ghosts: 1,
            tint: Some(Tint::Cyan),
            workspaces: vec![("w1".into(), "ATRIUM".into(), true), ("w2".into(), "CEFIS".into(), false)],
            within_workspace: true,
        }),
    }
}

pub fn scene(component: &str, _window: &mut Window, cx: &mut App) -> AnyView {
    let entries = entries(component);
    let sub = std::env::var("MAESTRIA_DEMO_SUB").ok();
    let open = sub.and_then(|label| entries.iter().position(|e| e.label().is_some_and(|l| l.starts_with(&label))));
    cx.new(|_| {
        let mut menu = ContextMenu::new(
            entries,
            point(px(60.), px(40.)),
            |action, _, _| log::info!("picked {action:?}"),
            |_, _| log::info!("dismissed"),
        );
        menu.preopen(open);
        menu
    })
    .into()
}
