use crate::manifest::PluginCommand;
use crate::registry::MAX_SIDEBAR_COMMANDS;
use crate::state::PluginInfo;

pub fn earns_rail_spot(p: &PluginInfo, has_windows: bool) -> bool {
    let declared = p.manifest.as_ref().is_some_and(|m| {
        p.enabled && (m.sidebar || m.screen || (m.icon.is_some() && !m.commands.is_empty()))
    });
    declared || has_windows
}

pub fn rail_plugins(
    plugins: &[PluginInfo],
    rail_order: &[String],
    has_windows: impl Fn(&str) -> bool,
) -> Vec<String> {
    let shown: Vec<&str> = plugins
        .iter()
        .filter(|p| earns_rail_spot(p, has_windows(&p.id)))
        .map(|p| p.id.as_str())
        .collect();
    let mut out: Vec<String> =
        rail_order.iter().filter(|id| shown.contains(&id.as_str())).cloned().collect();
    out.extend(shown.iter().filter(|id| !rail_order.iter().any(|o| o == *id)).map(|id| (*id).to_owned()));
    out
}

pub fn move_rail_plugin(rail: &[String], rail_order: &[String], id: &str, to: usize) -> Vec<String> {
    let mut ids: Vec<String> = rail.iter().filter(|r| *r != id).cloned().collect();
    ids.insert(to.min(ids.len()), id.to_owned());
    let rest: Vec<String> = rail_order.iter().filter(|o| !ids.contains(o)).cloned().collect();
    ids.extend(rest);
    ids
}

pub fn footer_commands(commands: &[PluginCommand], rail: &[String]) -> Vec<PluginCommand> {
    commands
        .iter()
        .filter(|c| c.sidebar && !rail.contains(&c.plugin_id))
        .take(MAX_SIDEBAR_COMMANDS)
        .cloned()
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RailBadge {
    Busy,
    Said(String),
    Windows(usize),
}

pub fn rail_badge(busy: bool, said: Option<&str>, open: usize, current: bool) -> Option<RailBadge> {
    if busy {
        Some(RailBadge::Busy)
    } else if let Some(s) = said {
        Some(RailBadge::Said(s.to_owned()))
    } else if open > 0 && !current {
        Some(RailBadge::Windows(open))
    } else {
        None
    }
}

pub fn rail_tooltip(name: &str, open: usize) -> String {
    match open {
        0 => name.to_owned(),
        1 => format!("{name} — 1 janela aberta"),
        n => format!("{name} — {n} janelas abertas"),
    }
}

pub fn sidebar_placeholder(name: &str, crash: Option<&str>) -> String {
    match crash {
        Some(c) => format!("{name} caiu: {c}"),
        None => format!("Abrindo {name}…"),
    }
}

pub const NO_WINDOWS: &str = "Nenhuma janela aberta";
pub const INSTALL_PLUGIN: &str = "Instalar plugin";

pub fn empty_screen_text(name: &str) -> String {
    format!("nada aberto do {name}")
}
