mod conn;

use std::path::{Path, PathBuf};

pub use conn::{ConnHandlers, Connection, LogSink, OUTBOX_LINES, RequestHandler, SHUTDOWN_GRACE};

pub fn resolve_executable(exe: &str, dir: &Path, path: &str) -> Option<PathBuf> {
    let named = mx_core::paths::expand_home(exe);
    let exts: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat", ""] } else { &[""] };
    let first_file = |base: PathBuf| {
        exts.iter().map(|e| with_suffix(&base, e)).find(|f| f.is_file())
    };
    if named.contains('/') || (cfg!(windows) && named.contains('\\')) {
        let full = if Path::new(&named).is_absolute() { PathBuf::from(&named) } else { dir.join(&named) };
        return first_file(full);
    }
    path.split(mx_core::proc::PATH_LIST_SEP)
        .filter(|d| !d.is_empty())
        .find_map(|d| first_file(Path::new(d).join(&named)))
}

fn with_suffix(base: &Path, suffix: &str) -> PathBuf {
    let mut name = base.as_os_str().to_owned();
    name.push(suffix);
    name.into()
}
