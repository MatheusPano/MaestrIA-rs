use std::sync::Arc;

use mx_core::model::LauncherIcon;
use mx_theme::{Face, Theme, Typography};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Appearance,
    Shortcuts,
    Programs,
    Plugins,
    Sessions,
    Updates,
    Account,
}

impl Section {
    pub const ALL: [Section; 7] = [
        Section::Appearance,
        Section::Shortcuts,
        Section::Programs,
        Section::Plugins,
        Section::Sessions,
        Section::Updates,
        Section::Account,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Section::Appearance => "Aparência",
            Section::Shortcuts => "Atalhos",
            Section::Programs => "Programas",
            Section::Plugins => "Plugins",
            Section::Sessions => "Sessões",
            Section::Updates => "Atualizações",
            Section::Account => "Conta & uso",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyPress {
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChordCheck {
    Ignore,
    Rejected(String),
    Accepted { id: String, label: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShortcutRow {
    pub action: String,
    pub label: String,
    pub hint: Option<String>,
    pub group: String,
    pub chords: Vec<(String, String)>,
    pub changed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LauncherRow {
    pub id: String,
    pub name: String,
    pub command: String,
    pub icon: LauncherIcon,
    pub open: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginState {
    Broken,
    Disabled,
    Ready,
    Starting,
    Running,
    Stopped,
}

impl PluginState {
    pub fn label(self) -> &'static str {
        match self {
            PluginState::Broken => "Com problema",
            PluginState::Disabled => "Desligado",
            PluginState::Ready => "Pronto",
            PluginState::Starting => "Subindo",
            PluginState::Running => "Rodando",
            PluginState::Stopped => "Parou",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PluginRow {
    pub id: String,
    pub name: String,
    pub version: String,
    pub state: PluginState,
    pub dev: bool,
    pub description: String,
    pub details: Vec<String>,
    pub commands: Vec<String>,
    pub themes: Vec<String>,
    pub enabled: bool,
    pub conflicts: usize,
    pub kinds: Vec<mx_plugins::catalog::PluginKind>,
    pub busy: bool,
    pub update: Option<RowUpdate>,
    pub offer: Option<RowUpdate>,
    pub has_origin: bool,
    pub manual: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RowUpdate {
    pub from: String,
    pub to: String,
    pub asks_more: bool,
    pub newer: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PluginAuto {
    pub on: bool,
    pub checking: bool,
    pub checked_at: Option<String>,
    pub error: Option<String>,
}

pub fn checked_label(a: &PluginAuto) -> Option<String> {
    if a.checking {
        return Some("Procurando…".into());
    }
    a.checked_at.as_ref().map(|at| format!("Procurado às {at}"))
}

pub fn update_status(p: &PluginRow) -> Option<(String, crate::chrome::menu::Tone)> {
    use crate::chrome::menu::Tone;
    if p.busy {
        return Some(("Atualizando…".into(), Tone::Purple));
    }
    let u = p.update.as_ref()?;
    Some(if u.asks_more { (format!("{} pede permissão nova", u.to), Tone::Yellow) } else { (format!("{} disponível", u.to), Tone::Purple) })
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppUpdates {
    pub current: String,
    pub enabled: bool,
    pub active: bool,
    pub installs_on_quit: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UsageWindow {
    pub label: String,
    pub percent: f64,
    pub resets: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Account {
    Loading,
    Error(String),
    Ready {
        org: String,
        plan: String,
        email: String,
        windows: Vec<UsageWindow>,
        credits: Option<String>,
        read_at: String,
    },
}

#[derive(Clone)]
pub struct SettingsData {
    pub themes: Vec<Arc<Theme>>,
    pub plugin_themes: Vec<Arc<Theme>>,
    pub theme_id: String,
    pub typography: Typography,
    pub faces: Vec<Face>,
    pub zoom_keys: (String, String),
    pub status_bar_top: bool,
    pub shortcuts: Vec<ShortcutRow>,
    pub fixed: Vec<(String, String)>,
    pub launchers: Vec<LauncherRow>,
    pub plugins: Vec<PluginRow>,
    pub plugin_auto: PluginAuto,
    pub hibernate_minutes: u32,
    pub app_updates: AppUpdates,
    pub account: Account,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PluginMenuAction {
    Restart,
    Configure,
    Log,
    Reveal,
    Remove,
    Update,
    Adopt,
    ToggleAutoUpdate,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingsAction {
    SetTheme(String),
    SetTypography(Typography),
    SetStatusBarTop(bool),
    ResetShortcuts(Option<String>),
    Bind { action: String, chord: String, replacing: Option<String> },
    Unbind { action: String, chord: String },
    NewLauncher,
    EditLauncher(String),
    RemoveLauncher(String),
    InstallPlugin,
    OpenCatalog,
    SetPluginAutoUpdate(bool),
    CheckPluginUpdates,
    OpenPluginsFolder,
    RescanPlugins,
    Plugin(String, PluginMenuAction),
    SetPluginEnabled(String, bool),
    LoadDevPlugin,
    SetHibernateMinutes(u32),
    SetAutoUpdate(bool),
    ReloadUsage,
    Close,
}

pub fn size_label(t: &Typography) -> String {
    format!("{}px", t.size().round() as i64)
}

pub fn line_label(t: &Typography) -> String {
    format!("{:.2}", t.line())
}

pub const MIN_HIBERNATE: u32 = 5;
pub const MAX_HIBERNATE: u32 = 24 * 60;

pub fn hibernate_label(m: u32) -> String {
    if m >= 60 && m.is_multiple_of(60) { format!("{} h", m / 60) } else { format!("{m} min") }
}

pub fn hibernate_less(m: u32) -> u32 {
    let next = if m <= 5 {
        5
    } else if m <= 30 {
        m - 5
    } else if m <= 120 {
        m - 15
    } else {
        m - 60
    };
    next.clamp(MIN_HIBERNATE, MAX_HIBERNATE)
}

pub fn hibernate_more(m: u32) -> u32 {
    let next = if m < 30 {
        m + 5
    } else if m < 120 {
        m + 15
    } else {
        m + 60
    };
    next.clamp(MIN_HIBERNATE, MAX_HIBERNATE)
}

pub fn usage_tone(percent: f64) -> crate::chrome::menu::Tone {
    use crate::chrome::menu::Tone;
    if percent >= 90. {
        Tone::Red
    } else if percent >= 75. {
        Tone::Yellow
    } else {
        Tone::Accent
    }
}

pub fn open_panels(n: usize) -> String {
    if n == 1 { "1 painel aberto".into() } else { format!("{n} painéis abertos") }
}

pub fn remove_launcher_tip(n: usize) -> String {
    if n == 0 { "Apagar".into() } else { format!("Apagar — os {n} painéis continuam abertos, como terminais") }
}

pub fn conflicts_label(n: usize) -> String {
    if n == 1 { "1 atalho em conflito".into() } else { format!("{n} atalhos em conflito") }
}
