use mx_core::model::FeatureOrHotfixKind;
use mx_theme::Tint;

use super::actions::{LauncherRef, MenuAction as A, group_index, kind_icon, launcher_icon};
use super::model::{Glyph, MenuEntry, MenuItem, Tone, Trailing};
use crate::chrome::icon;

type Entries = Vec<MenuEntry<A>>;

fn item(label: impl Into<gpui::SharedString>, action: A, glyph: icon::Icon, tone: Tone) -> MenuItem<A> {
    MenuItem::new(label, action).icon(glyph, tone)
}

pub fn open_here(launchers: &[LauncherRef]) -> Entries {
    let mut out: Entries = vec![
        MenuItem::new("Sessão do Claude", A::OpenClaude).glyph(Glyph::Claude).into(),
        item("Terminal", A::OpenShell, icon::TERMINAL, Tone::Dim).into(),
        item("Montar um fluxo…", A::NewFlow, icon::ACCOUNT_TREE_OUTLINED, Tone::Purple).into(),
        item("Abrir um markdown…", A::OpenMarkdown, icon::DESCRIPTION_OUTLINED, Tone::Dim).into(),
    ];
    if launchers.is_empty() {
        out.push(item("Criar um programa…", A::NewLauncher, icon::ADD_CIRCLE_OUTLINE, Tone::Faint).into());
        return out;
    }
    let mut programs: Entries = launchers
        .iter()
        .map(|l| {
            item(
                l.name.clone(),
                A::OpenLauncher(l.id.clone()),
                launcher_icon(l.icon),
                Tone::Group(group_index(&l.id)),
            )
            .into()
        })
        .collect();
    programs.push(MenuEntry::Divider);
    programs.push(item("Outro programa…", A::NewLauncher, icon::ADD_CIRCLE_OUTLINE, Tone::Faint).into());
    out.push(MenuEntry::Submenu(
        item("Meus programas", A::NewLauncher, icon::APPS_OUTLINED, Tone::Dim),
        programs,
    ));
    out
}

pub fn tint_item(current: Option<Tint>) -> MenuEntry<A> {
    let row = match current {
        None => MenuItem::new("Cor", A::Tint(None)).dot(icon::CIRCLE_OUTLINED, Tone::Dim, 12.),
        Some(t) => MenuItem::new(format!("Cor: {}", t.label()), A::Tint(Some(t)))
            .dot(icon::CIRCLE, Tone::Tint(t), 12.),
    };
    let none_glyph = if current.is_none() { icon::CHECK_CIRCLE_OUTLINE } else { icon::CIRCLE_OUTLINED };
    let mut items: Entries = vec![
        MenuItem::new("Sem cor", A::Tint(None)).dot(none_glyph, Tone::Faint, 13.).into(),
        MenuEntry::Divider,
    ];
    for t in Tint::ALL {
        let g = if current == Some(t) { icon::CHECK_CIRCLE } else { icon::CIRCLE };
        items.push(MenuItem::new(t.label(), A::Tint(Some(t))).dot(g, Tone::Tint(t), 13.).into());
    }
    MenuEntry::Submenu(row, items)
}

pub fn footer_add() -> Entries {
    vec![
        item("Adicionar pasta…", A::AddFolder, icon::CREATE_NEW_FOLDER_OUTLINED, Tone::Dim).into(),
        item("Importar .code-workspace…", A::ImportWorkspace, icon::FILE_OPEN_OUTLINED, Tone::Dim).into(),
        MenuEntry::Divider,
        item("Novo workspace…", A::NewWorkspace, icon::HEXAGON_OUTLINED, Tone::Accent).into(),
    ]
}

pub fn folder_plus(launchers: &[LauncherRef], in_feature: bool) -> Entries {
    let mut out = open_here(launchers);
    if !in_feature {
        out.push(MenuEntry::Divider);
        for kind in [FeatureOrHotfixKind::Feature, FeatureOrHotfixKind::Hotfix] {
            let label = capitalize(kind.label());
            out.push(item(format!("{label}…"), A::NewFeatureOrHotfix(kind), kind_icon(kind), Tone::Purple).into());
        }
    }
    out
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

#[derive(Clone, Debug, Default)]
pub struct FolderMenu {
    pub launchers: Vec<LauncherRef>,
    pub worktrees: usize,
    pub branched: bool,
    pub ghosts: usize,
    pub tint: Option<Tint>,
    pub workspaces: Vec<(String, String, bool)>,
    pub within_workspace: bool,
}

pub fn folder_menu(m: &FolderMenu) -> Entries {
    let mut out = open_here(&m.launchers);
    out.push(MenuEntry::Divider);
    out.push(item("Configuração do Claude", A::Setup, icon::TUNE, Tone::Dim).into());
    if m.branched {
        let mut w = item("Worktrees", A::Worktrees, icon::CALL_SPLIT, Tone::Dim)
            .trailing(Trailing::Chip(m.worktrees.to_string().into()));
        if m.ghosts > 0 {
            w = w.trailing(Trailing::Ghost(m.ghosts));
        }
        out.push(MenuEntry::Item(w.trailing(Trailing::Chevron)));
    }
    out.push(MenuEntry::Divider);
    for kind in [FeatureOrHotfixKind::Feature, FeatureOrHotfixKind::Hotfix] {
        out.push(item(format!("{}…", kind.new_label()), A::NewFeatureOrHotfix(kind), kind_icon(kind), Tone::Purple).into());
    }
    out.push(item("Renomear", A::Rename, icon::DRIVE_FILE_RENAME_OUTLINE, Tone::Dim).into());
    out.push(tint_item(m.tint));
    let mut ws: Entries = m
        .workspaces
        .iter()
        .map(|(id, name, member)| {
            let g = if *member { icon::CHECK_BOX } else { icon::CHECK_BOX_OUTLINE_BLANK };
            MenuItem::new(name.clone(), A::ToggleWorkspace(id.clone())).dot(g, Tone::Dim, 13.).into()
        })
        .collect();
    if !ws.is_empty() {
        ws.push(MenuEntry::Divider);
    }
    ws.push(MenuItem::new("Novo workspace…", A::NewWorkspace).dot(icon::ADD, Tone::Dim, 13.).into());
    out.push(MenuEntry::Submenu(
        item("Adicionar a workspace", A::NewWorkspace, icon::HEXAGON_OUTLINED, Tone::Dim),
        ws,
    ));
    if m.within_workspace {
        out.push(item("Tirar deste workspace", A::LeaveWorkspace, icon::LOGOUT, Tone::Dim).into());
    }
    out.push(MenuEntry::Divider);
    out.push(item("Limpar concluídos", A::CloseSettled, icon::CLEAR_ALL, Tone::Dim).into());
    out.push(item("Remover pasta", A::RemoveFolder, icon::FOLDER_OFF_OUTLINED, Tone::Red).danger().into());
    out
}

#[derive(Clone, Debug)]
pub struct WorktreeRow {
    pub path: String,
    pub label: String,
    pub branch: String,
    pub is_main: bool,
    pub prunable: bool,
    pub session_open: bool,
}

pub fn worktrees_menu(rows: &[WorktreeRow]) -> Entries {
    let mut out: Entries = rows
        .iter()
        .map(|w| {
            let (g, tone) = if w.prunable {
                (icon::LINK_OFF, Tone::Yellow)
            } else if w.is_main {
                (icon::HOME_OUTLINED, Tone::Faint)
            } else {
                (icon::CALL_SPLIT, Tone::Faint)
            };
            let (sub, sub_tone) = if w.prunable {
                ("Sem pasta no disco".to_owned(), Tone::Yellow)
            } else if w.session_open {
                (format!("{} · sessão aberta", w.branch), Tone::Faint)
            } else {
                (w.branch.clone(), Tone::Faint)
            };
            item(w.label.clone(), A::Worktree(w.path.clone()), g, tone).subtitle(sub, sub_tone).into()
        })
        .collect();
    if rows.iter().any(|w| w.prunable) {
        out.push(MenuEntry::Divider);
        out.push(
            item("Limpar worktrees fantasmas", A::PruneWorktrees, icon::CLEANING_SERVICES_OUTLINED, Tone::Yellow)
                .tone(Tone::Yellow)
                .into(),
        );
    }
    out
}

pub fn worktree_menu(launchers: &[LauncherRef], is_main: bool, ghost: bool) -> Entries {
    let mut out = Entries::new();
    if !ghost {
        out = open_here(launchers);
        out.push(MenuEntry::Divider);
        out.push(item("Abrir no vscode", A::OpenInEditor, icon::CODE, Tone::Dim).into());
        out.push(item("Configuração do Claude", A::Setup, icon::TUNE, Tone::Dim).into());
    }
    out.push(item("Copiar caminho", A::CopyPath, icon::LINK, Tone::Dim).into());
    if !is_main {
        let (label, g) = if ghost {
            ("Limpar registro…", icon::LINK_OFF)
        } else {
            ("Excluir worktree…", icon::DELETE_OUTLINE)
        };
        out.push(item(label, A::RemoveWorktree, g, Tone::Red).danger().into());
    }
    out
}

pub fn workspace_menu(has_file: bool, tint: Option<Tint>) -> Entries {
    let mut out: Entries = vec![
        item("Renomear…", A::Rename, icon::DRIVE_FILE_RENAME_OUTLINE, Tone::Dim).into(),
        tint_item(tint),
        MenuEntry::Divider,
    ];
    if has_file {
        out.push(item("Abrir no vscode", A::OpenInEditor, icon::OPEN_IN_NEW, Tone::Dim).into());
        out.push(item("Desassociar .code-workspace", A::UnlinkCodeWorkspace, icon::LINK_OFF, Tone::Dim).into());
    } else {
        out.push(item("Associar .code-workspace…", A::LinkCodeWorkspace, icon::LINK, Tone::Dim).into());
    }
    out.push(MenuEntry::Divider);
    out.push(item("Desfazer workspace", A::DissolveWorkspace, icon::HEXAGON_OUTLINED, Tone::Dim).into());
    out.push(item("Fechar workspace…", A::CloseWorkspace, icon::FOLDER_OFF_OUTLINED, Tone::Red).danger().into());
    out
}

pub fn feature_menu(launchers: &[LauncherRef], kind: FeatureOrHotfixKind, loose: bool, tint: Option<Tint>) -> Entries {
    let mut out = open_here(launchers);
    out.push(MenuEntry::Divider);
    if !loose {
        out.push(item("Configuração do Claude", A::Setup, icon::TUNE, Tone::Dim).into());
        out.push(item(format!("Nova task {}…", kind.in_this()), A::NewTask, icon::CALL_SPLIT, Tone::Dim).into());
    }
    out.push(item("Briefing…", A::Brief, icon::ASSIGNMENT_OUTLINED, Tone::Dim).into());
    out.push(item("Renomear", A::Rename, icon::DRIVE_FILE_RENAME_OUTLINE, Tone::Dim).into());
    out.push(tint_item(tint));
    out.push(item(format!("Virar {}", kind.other().label()), A::SwitchKind, kind_icon(kind.other()), Tone::Dim).into());
    out.push(MenuEntry::Divider);
    out.push(item(format!("Concluir {}", kind.label()), A::Complete, icon::TASK_ALT, Tone::Green).into());
    out.push(item(format!("Dissolver {}", kind.label()), A::Dissolve, icon::TRACK_CHANGES, Tone::Red).danger().into());
    out
}

pub fn group_menu(group_id: &str) -> Entries {
    vec![
        item("Guardar a tela de agora aqui", A::SaveGroup, icon::GRID_VIEW_ROUNDED, Tone::Group(group_index(group_id))).into(),
        item("Renomear", A::Rename, icon::DRIVE_FILE_RENAME_OUTLINE, Tone::Dim).into(),
        item("Esquecer o grupo", A::ForgetGroup, icon::GRID_OFF, Tone::Red).danger().into(),
    ]
}
