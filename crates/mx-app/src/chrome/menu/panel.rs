use mx_core::model::FeatureOrHotfixKind;
use mx_theme::Tint;

use super::actions::{MenuAction as A, group_index, kind_icon};
use super::builders::tint_item;
use super::model::{MenuEntry, MenuItem, Tone};
use crate::chrome::icon;

type Entries = Vec<MenuEntry<A>>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanelKind {
    #[default]
    Shell,
    Claude,
    Reader,
    Setup,
    Plugin,
}

#[derive(Clone, Debug, Default)]
pub struct PanelMenu {
    pub kind: PanelKind,
    pub relaunch: Option<(String, String)>,
    pub hibernated: bool,
    pub feature_tint: Option<(Tint, FeatureOrHotfixKind)>,
    pub tint: Option<Tint>,
    pub plans: usize,
    pub has_message: bool,
    pub follow_ups: usize,
    pub open: bool,
    pub pinned: bool,
    pub pin_chord: Option<String>,
    pub can_group: bool,
    pub group: Option<(String, String)>,
    pub passive: bool,
    pub done: bool,
    pub plugin_commands: Vec<(String, String)>,
    pub can_hibernate: bool,
    pub close_chord: Option<String>,
}

fn item(label: impl Into<gpui::SharedString>, action: A, glyph: icon::Icon, tone: Tone) -> MenuItem<A> {
    MenuItem::new(label, action).icon(glyph, tone)
}

pub fn panel_menu(m: &PanelMenu) -> Entries {
    let mut out = Entries::new();
    if let Some((launcher_id, name)) = &m.relaunch {
        out.push(item(format!("Rodar {name} de novo"), A::Relaunch, icon::REFRESH, Tone::Group(group_index(launcher_id))).into());
        out.push(MenuEntry::Divider);
    }
    if m.hibernated {
        out.push(item("Retomar a conversa", A::Wake, icon::PLAY_ARROW, Tone::Accent).into());
        out.push(MenuEntry::Divider);
    }
    out.push(item("Renomear…", A::Rename, icon::DRIVE_FILE_RENAME_OUTLINE, Tone::Dim).into());
    match m.feature_tint {
        Some((t, kind)) => out.push(
            MenuItem::new(format!("Cor: {} — {}", t.label(), kind.of_the()), A::Tint(Some(t)))
                .dot(icon::CIRCLE, Tone::Tint(t), 12.)
                .disabled(true)
                .into(),
        ),
        None => out.push(tint_item(m.tint)),
    }
    if m.kind == PanelKind::Claude {
        let plan = if m.plans > 1 { format!("Ver o plano ({})", m.plans) } else { "Ver o plano".into() };
        out.push(item(plan, A::ShowPlan, icon::CHECKLIST_RTL, Tone::Dim).disabled(m.plans == 0).into());
        out.push(item("Ver o último recado", A::ShowMessage, icon::CHAT_BUBBLE_OUTLINE, Tone::Dim).disabled(!m.has_message).into());
        let flow = if m.follow_ups == 0 {
            "Quando terminar…".to_owned()
        } else {
            format!("Quando terminar… ({})", m.follow_ups)
        };
        out.push(item(flow, A::Flow, icon::ACCOUNT_TREE_OUTLINED, Tone::Dim).into());
    }
    out.push(MenuEntry::Divider);
    if m.open {
        let (label, g, tone) = if m.pinned {
            ("Soltar o painel", icon::PUSH_PIN, Tone::Accent)
        } else {
            ("Prender o painel", icon::PUSH_PIN_OUTLINED, Tone::Dim)
        };
        out.push(item(label, A::TogglePin, g, tone).chord(m.pin_chord.clone()).into());
    }
    if m.can_group {
        out.push(item("Agrupar painéis", A::GroupPanes, icon::GRID_VIEW_ROUNDED, Tone::Dim).into());
    }
    if let Some((id, name)) = &m.group {
        out.push(item(format!("Desagrupar \"{name}\""), A::Ungroup, icon::GRID_OFF, Tone::Group(group_index(id))).into());
    }
    if !m.passive {
        out.push(item("Mover pra feature/hotfix…", A::MoveTo, kind_icon(FeatureOrHotfixKind::Feature), Tone::Purple).into());
        out.push(if m.done {
            item("Reabrir: não está concluída", A::SetDone(false), icon::TASK_ALT, Tone::Green).into()
        } else {
            item("Marcar como concluída", A::SetDone(true), icon::CHECK_CIRCLE_OUTLINE, Tone::Dim).into()
        });
    }
    if !m.plugin_commands.is_empty() {
        out.push(MenuEntry::Divider);
        let cmds = m
            .plugin_commands
            .iter()
            .map(|(id, title)| MenuItem::new(title.clone(), A::PluginCommand(id.clone())).into())
            .collect();
        out.push(MenuEntry::Submenu(item("Plugins", A::Close, icon::EXTENSION_OUTLINED, Tone::Dim), cmds));
    }
    out.push(MenuEntry::Divider);
    if m.can_hibernate {
        out.push(item("Hibernar (libera a memória)", A::Hibernate, icon::BEDTIME_OUTLINED, Tone::Dim).into());
    }
    let close = match m.kind {
        PanelKind::Reader => "Fechar o leitor",
        PanelKind::Setup => "Fechar a configuração",
        PanelKind::Plugin => "Fechar a janela do plugin",
        _ => "Fechar painel",
    };
    out.push(item(close, A::Close, icon::CLOSE, Tone::Red).danger().chord(m.close_chord.clone()).into());
    out
}

pub fn terminal_menu(copy_chord: &str, paste_chord: &str, has_selection: bool) -> Entries {
    vec![
        MenuItem::new("Copiar", A::Copy)
            .dot(icon::CONTENT_COPY, Tone::Dim, 13.)
            .chord(Some(copy_chord.to_owned()))
            .disabled(!has_selection)
            .into(),
        MenuItem::new("Colar", A::Paste)
            .dot(icon::CONTENT_PASTE, Tone::Dim, 13.)
            .chord(Some(paste_chord.to_owned()))
            .into(),
    ]
}

#[derive(Clone, Debug)]
pub struct FilterPlace {
    pub key: String,
    pub name: String,
    pub loose: bool,
    pub on: bool,
    pub features: Vec<(String, String, FeatureOrHotfixKind, bool)>,
}

pub const FILTER_KINDS: [&str; 2] = ["Sessões do Claude", "Terminais"];
pub const FILTER_STATES: [&str; 4] = ["Esperando você", "Rodando", "Paradas", "Concluídas"];

fn check(label: impl Into<gpui::SharedString>, action: A, on: bool) -> MenuItem<A> {
    let (g, tone) = if on {
        (icon::CHECK_BOX_ROUNDED, Tone::Accent)
    } else {
        (icon::CHECK_BOX_OUTLINE_BLANK_ROUNDED, Tone::Faint)
    };
    MenuItem::new(label, action).dot(g, tone, 15.).keep_open()
}

pub fn filter_menu(places: &[FilterPlace], on: &dyn Fn(&str) -> bool, filtering: bool) -> Entries {
    let mut out: Entries = vec![MenuEntry::Caption("Onde".into())];
    for p in places {
        let g = if p.loose { icon::INBOX_OUTLINED } else { icon::FOLDER_OUTLINED };
        let mut row = check(p.name.clone(), A::FilterFolder(p.key.clone()), p.on);
        row.trailing.clear();
        out.push(row.trailing(super::model::Trailing::Icon(g)).into());
        for (key, name, kind, f_on) in &p.features {
            out.push(
                check(name.clone(), A::FilterFeature(key.clone()), *f_on)
                    .indent(14.)
                    .trailing(super::model::Trailing::Kind(kind_icon(*kind)))
                    .into(),
            );
        }
    }
    out.push(MenuEntry::Caption("Tipo".into()));
    for k in FILTER_KINDS {
        out.push(check(k, A::Filter(k.into()), on(k)).into());
    }
    out.push(MenuEntry::Caption("Estado".into()));
    for s in FILTER_STATES {
        out.push(check(s, A::Filter(s.into()), on(s)).into());
    }
    if filtering {
        out.push(MenuEntry::Divider);
        out.push(MenuItem::new("Mostrar tudo de novo", A::ClearFilters).icon(icon::FILTER_LIST_OFF_ROUNDED, Tone::Dim).into());
    }
    out
}
