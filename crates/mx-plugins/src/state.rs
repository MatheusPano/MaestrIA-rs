use std::path::PathBuf;
use std::sync::Arc;

use chrono::Timelike;

use crate::manifest::PluginManifest;
use crate::permission::Permission;

pub const LOG_CAP: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    Invalid,
    Disabled,
    Idle,
    Starting,
    Running,
    Crashed,
}

impl PluginState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Invalid => "Com problema",
            Self::Disabled => "Desligado",
            Self::Idle => "Pronto",
            Self::Starting => "Subindo",
            Self::Running => "Rodando",
            Self::Crashed => "Parou",
        }
    }

    pub fn derive(has_manifest: bool, enabled: bool, ready: bool, starting: bool, crashed: bool) -> Self {
        if !has_manifest {
            Self::Invalid
        } else if !enabled {
            Self::Disabled
        } else if ready {
            Self::Running
        } else if starting {
            Self::Starting
        } else if crashed {
            Self::Crashed
        } else {
            Self::Idle
        }
    }
}

#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub id: String,
    pub dir: PathBuf,
    pub data_dir: PathBuf,
    pub manifest: Option<Arc<PluginManifest>>,
    pub problem: Option<String>,
    pub linked: bool,
    pub enabled: bool,
    pub crash: Option<String>,
    pub state: PluginState,
    pub palettes: Vec<String>,
}

impl PluginInfo {
    pub fn name(&self) -> &str {
        self.manifest.as_ref().map_or(&self.id, |m| &m.name)
    }

    pub fn active(&self) -> bool {
        self.manifest.is_some() && self.enabled
    }

    pub fn allows(&self, p: Permission) -> bool {
        self.manifest.as_ref().is_some_and(|m| m.allows(p))
    }

    pub fn has_process(&self) -> bool {
        self.manifest.as_ref().is_some_and(|m| m.has_process())
    }
}

pub fn log_line(text: &str) -> String {
    let now = chrono::Local::now();
    format!("{:02}:{:02}:{:02}  {text}", now.hour(), now.minute(), now.second())
}
