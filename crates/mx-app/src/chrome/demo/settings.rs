use std::rc::Rc;

use gpui::{AnyView, App, AppContext, Window};
use mx_core::model::LauncherIcon;
use mx_plugins::catalog::PluginKind;
use mx_theme::{HACK, Typography};

use crate::chrome::settings::{
    Account, ChordCheck, KeyPress, LauncherRow, PluginAuto, PluginRow, PluginState, RowUpdate, Section, SettingsData,
    SettingsDialog, ShortcutRow, UsageWindow, fixed_rows,
};

fn chord(label: &str) -> (String, String) {
    (label.to_lowercase(), label.to_owned())
}

fn shortcuts() -> Vec<ShortcutRow> {
    let row = |action: &str, label: &str, hint: Option<&str>, group: &str, chords: &[&str], changed: bool| ShortcutRow {
        action: action.into(),
        label: label.into(),
        hint: hint.map(Into::into),
        group: group.into(),
        chords: chords.iter().map(|c| chord(c)).collect(),
        changed,
    };
    vec![
        row("claude", "Nova sessão Claude", Some("Na pasta e na feature/hotfix do painel em foco"), "Sessões", &["Ctrl+Shift+T"], false),
        row("shell", "Novo shell", Some("Um terminal comum, no mesmo lugar"), "Sessões", &["Ctrl+Shift+Enter"], false),
        row("rename", "Renomear o painel em foco", Some("O título dele; em branco, volta a ser o da branch ou da pasta"), "Sessões", &["F2"], true),
        row("next-pane", "Próximo painel", Some("Na ordem em que estão na tela"), "Navegação", &["Ctrl+Tab", "Ctrl+PgDn"], false),
        row("find", "Buscar no painel", Some("No que o terminal em foco já escreveu; Enter sobe, Shift+Enter desce, Esc fecha"), "Navegação", &["Ctrl+Shift+F"], false),
        row("search", "Buscar na lateral", Some("Foca o campo de busca; Esc limpa e devolve o teclado"), "Navegação", &["Ctrl+Alt+F"], false),
        row("zoom-in", "Aumentar o corpo do painel", Some("Só o painel em foco; a base de todos está em aparência"), "Exibição", &["Ctrl+=", "Ctrl+Shift+="], false),
        row("settings", "Configurações", Some("Esta tela"), "Janela", &["Ctrl+,", "Ctrl+Shift+P"], false),
    ]
}

pub fn data() -> SettingsData {
    SettingsData {
        themes: mx_theme::built_ins().to_vec(),
        plugin_themes: Vec::new(),
        theme_id: std::env::var("MAESTRIA_DEMO_THEME").unwrap_or_else(|_| "maestria".into()),
        typography: Typography::standard(),
        faces: vec![HACK, mx_theme::FACES.iter().copied().find(|f| f.family == "DejaVu Sans Mono").unwrap_or(HACK)],
        zoom_keys: ("Ctrl+=".into(), "Ctrl+−".into()),
        status_bar_top: false,
        shortcuts: shortcuts(),
        fixed: fixed_rows(true),
        launchers: vec![
            LauncherRow { id: "l1".into(), name: "btop".into(), command: "btop".into(), icon: LauncherIcon::Monitor, open: 1 },
            LauncherRow { id: "l2".into(), name: "npm".into(), command: "npm run dev".into(), icon: LauncherIcon::Server, open: 0 },
        ],
        plugins: vec![
            PluginRow { id: "git".into(), name: "Git".into(), version: "0.4.1".into(), state: PluginState::Running, dev: true, description: "Mudanças, branches e commits na lateral".into(), details: vec!["Hooks: SessionEnd".into()], commands: vec!["status".into(), "commit".into()], themes: vec![], enabled: true, conflicts: 1, kinds: vec![PluginKind::Feature], busy: false, update: Some(RowUpdate { from: "0.4.1".into(), to: "0.5.0".into(), asks_more: false, newer: true }), offer: None, has_origin: true, manual: false },
            PluginRow { id: "cat".into(), name: "Catppuccin".into(), version: "1.0.0".into(), state: PluginState::Ready, dev: false, description: "Quatro sabores de tema".into(), details: vec![], commands: vec![], themes: vec!["Mocha".into(), "Latte".into()], enabled: true, conflicts: 0, kinds: vec![PluginKind::Theme], busy: false, update: None, offer: None, has_origin: false, manual: false },
        ],
        plugin_auto: PluginAuto { on: true, checking: false, checked_at: Some("14:32".into()), error: None },
        hibernate_minutes: 30,
        account: Account::Ready {
            org: "Marrow".into(),
            plan: "Max".into(),
            email: "dev@example.com".into(),
            windows: vec![
                UsageWindow { label: "Sessão de 5 horas".into(), percent: 42., resets: "2h 10min".into() },
                UsageWindow { label: "Semana".into(), percent: 81., resets: "3d".into() },
            ],
            credits: None,
            read_at: "14:32".into(),
        },
    }
}

pub fn demo_validate(p: &KeyPress) -> ChordCheck {
    if matches!(p.key.as_str(), "shift" | "control" | "alt" | "platform" | "function") {
        return ChordCheck::Ignore;
    }
    let mut label = String::new();
    for (on, name) in [(p.ctrl, "Ctrl+"), (p.shift, "Shift+"), (p.alt, "Alt+"), (p.meta, "Super+")] {
        if on {
            label.push_str(name);
        }
    }
    label.push_str(&p.key.to_uppercase());
    if p.ctrl && p.shift && matches!(p.key.as_str(), "c" | "v") {
        return ChordCheck::Rejected(format!("o painel {} com {label}", if p.key == "c" { "copia a seleção" } else { "cola" }));
    }
    if !(p.ctrl || p.alt || p.meta) {
        return ChordCheck::Rejected("precisa de Ctrl ou Alt — sem modificador a tecla sumiria do terminal".into());
    }
    ChordCheck::Accepted { id: label.to_lowercase(), label }
}

pub fn scene(_component: &str, _window: &mut Window, cx: &mut App) -> AnyView {
    let section = match std::env::var("MAESTRIA_DEMO_SECTION").as_deref() {
        Ok("atalhos") => Section::Shortcuts,
        Ok("programas") => Section::Programs,
        Ok("plugins") => Section::Plugins,
        Ok("sessoes") => Section::Sessions,
        Ok("conta") => Section::Account,
        _ => Section::Appearance,
    };
    let recording = std::env::var("MAESTRIA_DEMO_RECORD").ok();
    cx.new(|cx| {
        let mut d = SettingsDialog::new(data(), section, Rc::new(demo_validate), |a, _, _| log::info!("settings {a:?}"), cx);
        if let Some(why) = recording {
            d.preset_recording("rename", why);
        }
        d
    })
    .into()
}
