use mx_core::model::{ClaudeStatus, Tab, TabKind, worktree::DETACHED};
use mx_store::Store;

pub const GAP: f32 = 12.;
pub const CARD_MIN: f32 = 280.;
pub const TILE_GAP: f32 = 10.;
pub const TILE_MIN: f32 = 220.;
pub const APP_WIDTH: f32 = 196.;
pub const NOTHING_SAID: &str = "Nada dito ainda";
pub const DRAG_HINT: &str = "Arraste um cartão ou uma sessão da lateral pra cá — pela borda de um painel, ela divide a tela; pelo meio, toma o lugar dele";
pub const APPS_TITLE: &str = "O que vamos abrir?";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bucket {
    Waiting,
    Fresh,
    Working,
    Rest,
}

impl Bucket {
    pub const ALL: [Bucket; 4] = [Bucket::Waiting, Bucket::Fresh, Bucket::Working, Bucket::Rest];

    pub fn label(self) -> &'static str {
        match self {
            Bucket::Waiting => "Esperando você",
            Bucket::Fresh => "Terminaram enquanto você não via",
            Bucket::Working => "Trabalhando",
            Bucket::Rest => "Em repouso",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateTone {
    Status(ClaudeStatus),
    Faint,
}

pub fn card_tabs(store: &Store) -> Vec<&Tab> {
    let installed = |owner: &str| store.plugins().is_some_and(|r| r.get(owner).is_some());
    store
        .tabs()
        .iter()
        .filter(|t| !t.exited && !t.embedded && !t.is_plugin_view() && !t.owner.as_deref().is_some_and(installed))
        .collect()
}

pub fn classify(t: &Tab) -> Bucket {
    let status = t.status();
    if t.kind == TabKind::Claude && !t.done && status.needs_human() {
        Bucket::Waiting
    } else if t.unseen && t.rested() && !t.done {
        Bucket::Fresh
    } else if t.kind == TabKind::Claude
        && matches!(status, ClaudeStatus::Working | ClaudeStatus::Tool | ClaudeStatus::Starting)
    {
        Bucket::Working
    } else {
        Bucket::Rest
    }
}

pub fn sections<'a>(tabs: &[&'a Tab]) -> Vec<(Bucket, Vec<&'a Tab>)> {
    Bucket::ALL
        .into_iter()
        .map(|b| (b, tabs.iter().copied().filter(|t| classify(t) == b).collect::<Vec<_>>()))
        .filter(|(_, list)| !list.is_empty())
        .collect()
}

pub fn columns(width: f32, gap: f32, min: f32) -> (usize, f32) {
    let cols = (((width + gap) / (min + gap)).floor() as i64).clamp(1, 99) as usize;
    (cols, (width - gap * (cols as f32 - 1.)) / cols as f32)
}

pub fn where_line(title: &str, folder_name: &str, loose: bool, branch: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if !loose && title != folder_name {
        parts.push(folder_name);
    }
    if !branch.is_empty() && branch != DETACHED {
        parts.push(branch);
    }
    parts.join(" · ")
}

pub fn tilde(cwd: &str, home: Option<&str>) -> String {
    match home {
        Some(h) if !h.is_empty() && cwd.starts_with(h) => format!("~{}", &cwd[h.len()..]),
        _ => cwd.to_owned(),
    }
}

pub fn body(t: &Tab, subtitle: &str, home: Option<&str>) -> Option<(String, bool)> {
    if t.kind == TabKind::Shell && t.launcher_id.is_none() {
        return Some((tilde(&t.cwd, home), true));
    }
    if t.kind == TabKind::Claude && (subtitle == t.status().label() || subtitle == "Pronto") {
        return t.hooks.last_prompt.clone().map(|p| (p, false));
    }
    Some((subtitle.to_owned(), false))
}

pub fn state(t: &Tab) -> (&'static str, StateTone) {
    match t.kind {
        TabKind::Claude => (t.status().label(), StateTone::Status(t.status())),
        TabKind::Shell if t.launcher_id.is_some() => ("Programa", StateTone::Faint),
        TabKind::Shell => ("Terminal", StateTone::Faint),
        TabKind::Reader => ("Documento", StateTone::Faint),
        TabKind::Setup => ("Configuração", StateTone::Faint),
        TabKind::Plugin => ("Plugin", StateTone::Faint),
    }
}

pub fn attention(t: &Tab) -> bool {
    t.kind == TabKind::Claude && !t.done && t.status().needs_human()
}

pub fn apps_subtitle(plugins: usize) -> &'static str {
    if plugins == 0 {
        "Instale um plugin, ou abra uma sessão pela lateral"
    } else {
        "Escolha um plugin, ou abra uma sessão pela lateral"
    }
}

pub fn tile_opens(sidebar_hidden: bool, shown: Option<&str>, plugin: &str) -> bool {
    sidebar_hidden || shown != Some(plugin)
}

pub fn open_card(store: &mut Store, id: &str) {
    match store.group_of(id).map(|g| g.id.clone()) {
        Some(g) => store.open_group(&g),
        None => store.select(id),
    }
}

pub fn open_tile(store: &mut Store, plugin: &str) {
    let shown = store.shown_plugin();
    if tile_opens(store.prefs().sidebar_hidden, shown.as_deref(), plugin) {
        store.show_sidebar_view(Some(plugin));
    }
}

#[cfg(test)]
#[path = "cards_model_tests.rs"]
mod tests;
