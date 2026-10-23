use mx_core::model::{FeatureOrHotfixKind, LauncherIcon};
use mx_theme::Tint;

use crate::chrome::menu::builders::{self, FolderMenu};
use crate::chrome::menu::geometry::{self, Rect};
use crate::chrome::menu::model::{content_height, find, labels};
use crate::chrome::menu::panel::{self, PanelKind, PanelMenu};
use crate::chrome::menu::{LauncherRef, MenuAction, MenuEntry};

fn btop() -> Vec<LauncherRef> {
    vec![LauncherRef { id: "l1".into(), name: "btop".into(), icon: LauncherIcon::Monitor }]
}

fn pos(entries: &[String], label: &str) -> usize {
    entries.iter().position(|l| l == label).unwrap_or_else(|| panic!("missing {label} in {entries:?}"))
}

#[test]
fn feature_menu_orders_claude_terminal_flow_before_rename_without_resume() {
    let l = labels(&builders::feature_menu(&[], FeatureOrHotfixKind::Feature, false, None));
    assert!(pos(&l, "Sessão do Claude") < pos(&l, "Terminal"));
    assert!(pos(&l, "Terminal") < pos(&l, "Montar um fluxo…"));
    assert!(pos(&l, "Montar um fluxo…") < pos(&l, "Renomear"));
    assert!(!l.iter().any(|x| x.to_lowercase().contains("retomar conversa")));
    assert!(l.contains(&"Nova task nessa feature…".to_owned()));
    assert!(l.contains(&"Dissolver feature".to_owned()));
}

#[test]
fn folder_plus_menu_picks_claude_in_folder_root() {
    let entries = builders::folder_plus(&[], false);
    let MenuEntry::Item(first) = &entries[0] else { panic!() };
    assert_eq!(first.value, Some(MenuAction::OpenClaude));
    let l = labels(&entries);
    assert!(l.contains(&"Feature…".to_owned()) && l.contains(&"Hotfix…".to_owned()));
    assert!(!labels(&builders::folder_plus(&[], true)).contains(&"Feature…".to_owned()));
}

#[test]
fn folder_menu_with_launcher_and_worktrees_shows_programs_worktrees_and_remove() {
    let l = labels(&builders::folder_menu(&FolderMenu { launchers: btop(), worktrees: 2, branched: true, ..Default::default() }));
    for want in ["Sessão do Claude", "Terminal", "Meus programas", "Worktrees", "Remover pasta"] {
        pos(&l, want);
    }
}

#[test]
fn panel_menu_of_a_claude_tab_has_rename_and_plan_but_no_markdown() {
    let l = labels(&panel::panel_menu(&PanelMenu { kind: PanelKind::Claude, open: true, ..Default::default() }));
    assert!(!l.contains(&"Abrir um markdown…".to_owned()));
    pos(&l, "Renomear…");
    pos(&l, "Ver o plano");
}

#[test]
fn tint_row_reads_cor_then_cor_magenta_and_sem_cor_clears() {
    let MenuEntry::Submenu(row, items) = builders::tint_item(None) else { panic!() };
    assert_eq!(row.label, "Cor");
    assert_eq!(labels(&items).len(), 15);
    let magenta = find(&items, "Magenta").unwrap();
    let MenuEntry::Item(m) = magenta else { panic!() };
    assert_eq!(m.value, Some(MenuAction::Tint(Some(Tint::Magenta))));
    let MenuEntry::Submenu(row, items) = builders::tint_item(Some(Tint::Magenta)) else { panic!() };
    assert_eq!(row.label, "Cor: Magenta");
    let MenuEntry::Item(none) = find(&items, "Sem cor").unwrap() else { panic!() };
    assert_eq!(none.value, Some(MenuAction::Tint(None)));
    let names: Vec<String> = labels(&items).into_iter().skip(2).collect();
    assert_eq!(names, Tint::ALL.iter().map(|t| t.label().to_owned()).collect::<Vec<_>>());
}

#[test]
fn tint_submenu_in_an_800_by_240_window_stays_inside_8_to_232_and_scrolls() {
    let MenuEntry::Submenu(_, items) = builders::tint_item(None) else { panic!() };
    let h = content_height(&items);
    assert!(h > 224.);
    let f = geometry::submenu_frame(Rect::new(60., 8., 240., 224.), 200., h, 800., 240.);
    assert!(f.y >= 8. && f.bottom() <= 232.);
    assert!(f.h < h);
}

#[test]
fn submenu_flips_left_when_there_is_no_room_on_the_right() {
    let f = geometry::submenu_frame(Rect::new(600., 40., 200., 300.), 100., 200., 800., 600.);
    assert_eq!(f.x, 600. - 216. + 4.);
}

#[test]
fn tab_of_a_red_feature_shows_disabled_feature_colour() {
    let entries = panel::panel_menu(&PanelMenu { feature_tint: Some((Tint::Red, FeatureOrHotfixKind::Feature)), ..Default::default() });
    let MenuEntry::Item(i) = find(&entries, "Cor: Vermelho — da feature").unwrap() else { panic!() };
    assert!(i.disabled);
}

#[test]
fn launcher_submenu_lists_launchers_then_another_program() {
    let entries = builders::open_here(&btop());
    let MenuEntry::Submenu(_, items) = find(&entries, "Meus programas").unwrap() else { panic!() };
    assert_eq!(labels(items), vec!["btop", "—", "Outro programa…"]);
    let MenuEntry::Item(b) = &items[0] else { panic!() };
    assert_eq!(b.value, Some(MenuAction::OpenLauncher("l1".into())));
    assert!(find(&builders::open_here(&[]), "Criar um programa…").is_some());
}

#[test]
fn footer_add_offers_folder_import_and_workspace() {
    assert_eq!(labels(&builders::footer_add()), vec!["Adicionar pasta…", "Importar .code-workspace…", "—", "Novo workspace…"]);
}

#[test]
fn workspace_menu_without_file_offers_link_and_with_file_offers_vscode_and_unlink() {
    let l = labels(&builders::workspace_menu(false, None));
    pos(&l, "Associar .code-workspace…");
    assert!(!l.contains(&"Abrir no vscode".to_owned()));
    pos(&l, "Desfazer workspace");
    let l = labels(&builders::workspace_menu(true, None));
    pos(&l, "Abrir no vscode");
    pos(&l, "Desassociar .code-workspace");
}

#[test]
fn folder_menu_workspace_submenu_ticks_membership_and_offers_leave_inside_one() {
    let m = FolderMenu { workspaces: vec![("w".into(), "ATRIUM".into(), false)], within_workspace: true, ..Default::default() };
    let entries = builders::folder_menu(&m);
    let MenuEntry::Submenu(_, items) = find(&entries, "Adicionar a workspace").unwrap() else { panic!() };
    let MenuEntry::Item(a) = &items[0] else { panic!() };
    assert_eq!(a.value, Some(MenuAction::ToggleWorkspace("w".into())));
    pos(&labels(&entries), "Tirar deste workspace");
}

#[test]
fn grouping_offers_group_or_ungroup_by_state() {
    let grouped = labels(&panel::panel_menu(&PanelMenu { open: true, group: Some(("g".into(), "3 terminais".into())), ..Default::default() }));
    pos(&grouped, "Desagrupar \"3 terminais\"");
    assert!(!grouped.contains(&"Agrupar painéis".to_owned()));
    let both = labels(&panel::panel_menu(&PanelMenu { open: true, can_group: true, group: Some(("g".into(), "3 terminais".into())), ..Default::default() }));
    pos(&both, "Agrupar painéis");
    pos(&both, "Desagrupar \"3 terminais\"");
    let single = labels(&panel::panel_menu(&PanelMenu { open: true, ..Default::default() }));
    assert!(!single.contains(&"Agrupar painéis".to_owned()));
}

#[test]
fn worktree_menu_copies_path_and_ghost_offers_clear_record() {
    let l = labels(&builders::worktree_menu(&[], false, false));
    pos(&l, "Copiar caminho");
    pos(&l, "Excluir worktree…");
    let ghost = labels(&builders::worktree_menu(&[], false, true));
    assert_eq!(ghost, vec!["Copiar caminho", "Limpar registro…"]);
    assert!(!labels(&builders::worktree_menu(&[], true, false)).contains(&"Excluir worktree…".to_owned()));
}
