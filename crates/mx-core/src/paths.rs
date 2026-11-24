use std::path::PathBuf;

pub const SEPARATORS: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };

pub fn is_within(path: &str, root: &str) -> bool {
    let Some(head) = path.get(..root.len()) else { return false };
    let same = if cfg!(windows) { head.eq_ignore_ascii_case(root) } else { head == root };
    same && (path.len() == root.len() || path[root.len()..].starts_with(SEPARATORS))
}

pub fn native(path: &str) -> String {
    if cfg!(windows) { path.replace('/', "\\") } else { path.to_owned() }
}

pub fn plain(path: PathBuf) -> PathBuf {
    let Some(text) = path.to_str().filter(|_| cfg!(windows)) else { return path };
    if let Some(share) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{share}"));
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path,
    }
}

pub const STATE_DIR_ENV: &str = "MAESTRIA_HOME";
pub const CONFIG_FILE: &str = "config.json";

pub fn home() -> Option<String> {
    let var = |k| std::env::var(k).ok().filter(|h: &String| !h.is_empty());
    if cfg!(windows) { var("USERPROFILE").or_else(|| var("HOME")) } else { var("HOME") }
}

pub fn expand_home(path: &str) -> String {
    expand_home_with(path, home().as_deref())
}

pub fn expand_home_with(path: &str, home: Option<&str>) -> String {
    let tilde_dir = path.strip_prefix('~').is_some_and(|r| r.starts_with(SEPARATORS));
    if path != "~" && !tilde_dir {
        return path.to_owned();
    }
    let Some(home) = home.filter(|h| !h.is_empty()) else {
        return path.to_owned();
    };
    let rest = &path[1..];
    if rest.is_empty() {
        return home.to_owned();
    }
    let home = home.trim_end_matches(SEPARATORS);
    if cfg!(windows) { format!("{home}{}", rest.replace('/', "\\")) } else { format!("{home}{rest}") }
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
