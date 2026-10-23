use mx_core::model::{FeatureOrHotfixKind, LauncherIcon};
use mx_theme::Tint;

use crate::chrome::icon::{self, Icon};

#[derive(Clone, Debug, PartialEq)]
pub enum MenuAction {
    OpenClaude,
    OpenShell,
    NewFlow,
    OpenMarkdown,
    NewLauncher,
    OpenLauncher(String),
    AddFolder,
    ImportWorkspace,
    NewWorkspace,
    NewFeatureOrHotfix(FeatureOrHotfixKind),
    Setup,
    Worktrees,
    Rename,
    Tint(Option<Tint>),
    ToggleWorkspace(String),
    LeaveWorkspace,
    CloseSettled,
    RemoveFolder,
    Worktree(String),
    PruneWorktrees,
    OpenInEditor,
    CopyPath,
    RemoveWorktree,
    LinkCodeWorkspace,
    UnlinkCodeWorkspace,
    DissolveWorkspace,
    CloseWorkspace,
    NewTask,
    Brief,
    SwitchKind,
    Complete,
    Dissolve,
    SaveGroup,
    ForgetGroup,
    Relaunch,
    Wake,
    ShowPlan,
    ShowMessage,
    Flow,
    TogglePin,
    GroupPanes,
    Ungroup,
    MoveTo,
    SetDone(bool),
    PluginCommand(String),
    Hibernate,
    Close,
    Copy,
    Paste,
    FilterFolder(String),
    FilterFeature(String),
    Filter(String),
    ClearFilters,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LauncherRef {
    pub id: String,
    pub name: String,
    pub icon: LauncherIcon,
}

pub fn launcher_icon(icon: LauncherIcon) -> Icon {
    match icon {
        LauncherIcon::Terminal => icon::TERMINAL,
        LauncherIcon::Monitor => icon::MONITOR_HEART_OUTLINED,
        LauncherIcon::Chart => icon::BAR_CHART,
        LauncherIcon::Git => icon::CALL_SPLIT,
        LauncherIcon::Server => icon::DNS_OUTLINED,
        LauncherIcon::Database => icon::STORAGE_OUTLINED,
        LauncherIcon::Rocket => icon::ROCKET_LAUNCH_OUTLINED,
        LauncherIcon::Bug => icon::BUG_REPORT_OUTLINED,
        LauncherIcon::Edit => icon::EDIT_OUTLINED,
        LauncherIcon::Box => icon::INVENTORY_2_OUTLINED,
    }
}

pub fn kind_icon(kind: FeatureOrHotfixKind) -> Icon {
    match kind {
        FeatureOrHotfixKind::Feature => icon::TRACK_CHANGES,
        FeatureOrHotfixKind::Hotfix => icon::BOLT,
    }
}

pub fn group_index(id: &str) -> usize {
    id.encode_utf16().map(usize::from).sum::<usize>() % 4
}
