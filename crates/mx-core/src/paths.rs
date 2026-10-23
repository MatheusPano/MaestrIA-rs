use std::path::PathBuf;

pub const STATE_DIR_ENV: &str = "MAESTRIA_HOME";
pub const CONFIG_FILE: &str = "config.json";

pub fn home() -> Option<String> {
    std::env::var("HOME").ok().filter(|h| !h.is_empty())
}

pub fn expand_home(path: &str) -> String {
    expand_home_with(path, home().as_deref())
}

pub fn expand_home_with(path: &str, home: Option<&str>) -> String {
    if path != "~" && !path.starts_with("~/") {
        return path.to_owned();
    }
    let Some(home) = home.filter(|h| !h.is_empty()) else {
        return path.to_owned();
    };
    let rest = &path[1..];
    if rest.is_empty() {
        return home.to_owned();
    }
    format!("{}{rest}", home.strip_suffix('/').unwrap_or(home))
}

pub fn state_dir() -> PathBuf {
    state_dir_from(
        std::env::var(STATE_DIR_ENV).ok().as_deref(),
        home().as_deref(),
    )
}

pub fn state_dir_from(override_dir: Option<&str>, home: Option<&str>) -> PathBuf {
    if let Some(dir) = override_dir.map(str::trim).filter(|d| !d.is_empty()) {
        return PathBuf::from(expand_home_with(dir, home));
    }
    let home = home.filter(|h| !h.is_empty()).unwrap_or("/");
    PathBuf::from(home).join(".maestria-rs")
}

pub fn config_path(state_dir: &std::path::Path) -> PathBuf {
    state_dir.join(CONFIG_FILE)
}

pub fn loose_root(home: Option<&str>) -> String {
    home.filter(|h| !h.is_empty()).unwrap_or("/").to_owned()
}
