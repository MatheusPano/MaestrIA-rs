mod release;
mod sums;

#[cfg(feature = "self-update")]
pub mod fetch;
#[cfg(feature = "self-update")]
pub mod install;

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::Duration;

use mx_core::version::compare_versions;

pub use release::Release;
pub use sums::expected_sum;

pub const BUILT_IN: bool = cfg!(feature = "self-update");
pub const PACKAGE_PREFIX: &str = "maestria-rs_";
pub const ASSET_SUFFIX: &str = "_amd64.deb";
pub const INSTALL_DIR: &str = "/opt/maestria-rs/";
pub const FIRST_DELAY: Duration = Duration::from_secs(30);
pub const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Idle,
    Fetching,
    Ready,
    Installing,
    Installed,
}

#[derive(Debug, Clone)]
pub struct Updater {
    current: String,
    enabled: bool,
    active: bool,
    stage: Stage,
    release: Option<Release>,
    staged: Option<PathBuf>,
    etag: Option<String>,
    relaunch: bool,
    dir: PathBuf,
}

pub fn installed_here(exe: &Path) -> bool {
    cfg!(all(target_os = "linux", target_arch = "x86_64")) && exe.starts_with(INSTALL_DIR)
}

impl Updater {
    pub fn new(current: &str, state_dir: &Path) -> Updater {
        let exe = std::env::current_exe().ok().and_then(|e| e.canonicalize().ok());
        let enabled = BUILT_IN && !current.is_empty() && exe.as_deref().is_some_and(installed_here);
        Updater::with_enabled(current, state_dir, enabled)
    }

    pub fn with_enabled(current: &str, state_dir: &Path, enabled: bool) -> Updater {
        Updater {
            current: current.to_owned(),
            enabled: enabled && !current.is_empty(),
            active: false,
            stage: Stage::Idle,
            release: None,
            staged: None,
            etag: None,
            relaunch: false,
            dir: state_dir.join("update"),
        }
    }

    pub fn current(&self) -> &str {
        &self.current
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn release(&self) -> Option<&Release> {
        self.release.as_ref()
    }

    pub fn staged(&self) -> Option<&Path> {
        self.staged.as_deref()
    }

    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    pub fn set_etag(&mut self, etag: Option<String>) {
        self.etag = etag;
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn relaunch_requested(&self) -> bool {
        self.relaunch
    }

    pub fn load_active(&mut self, on: bool) {
        self.active = on;
    }

    pub fn start(&mut self) -> Option<Duration> {
        if !self.enabled {
            return None;
        }
        let _ = std::fs::remove_dir_all(&self.dir);
        self.active.then_some(FIRST_DELAY)
    }

    pub fn set_active(&mut self, on: bool) -> Option<Duration> {
        if on == self.active {
            return None;
        }
        self.active = on;
        if !self.enabled {
            return None;
        }
        if on {
            return Some(Duration::ZERO);
        }
        if self.stage == Stage::Ready {
            self.discard();
        }
        None
    }

    pub fn can_check(&self) -> bool {
        self.enabled && self.active && self.stage == Stage::Idle
    }

    pub fn begin_fetch(&mut self, latest: Option<&Release>) -> bool {
        let newer = latest.is_some_and(|r| compare_versions(&r.version, &self.current) == Ordering::Greater);
        if !self.can_check() || !newer {
            return false;
        }
        self.stage = Stage::Fetching;
        true
    }

    pub fn fetched(&mut self, release: Release, staged: Result<PathBuf, String>) -> Result<(), String> {
        match staged {
            Ok(path) if self.active => {
                self.staged = Some(path);
                self.release = Some(release);
                self.stage = Stage::Ready;
                Ok(())
            }
            Ok(_) => {
                self.discard();
                Ok(())
            }
            Err(e) => {
                self.staged = None;
                if self.stage == Stage::Fetching {
                    self.stage = Stage::Idle;
                }
                Err(e)
            }
        }
    }

    pub fn discard(&mut self) {
        self.staged = None;
        self.release = None;
        self.relaunch = false;
        self.etag = None;
        self.stage = Stage::Idle;
        let _ = std::fs::remove_dir_all(&self.dir);
    }

    pub fn begin_install(&mut self) -> Option<PathBuf> {
        if self.stage != Stage::Ready {
            return None;
        }
        let staged = self.staged.clone()?;
        self.stage = Stage::Installing;
        Some(staged)
    }

    pub fn installed(&mut self, outcome: Result<(), String>) -> Result<(), String> {
        self.stage = if outcome.is_ok() { Stage::Installed } else { Stage::Ready };
        outcome
    }

    pub fn relaunch_after_quit(&mut self) {
        self.relaunch = true;
    }

    pub fn shows_chip(&self) -> bool {
        self.release.is_some() && matches!(self.stage, Stage::Ready | Stage::Installing)
    }

    pub fn relaunches_on_quit(&self) -> bool {
        self.stage == Stage::Installed && self.relaunch
    }
}
