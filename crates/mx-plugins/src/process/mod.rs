mod conn;

use std::path::{Path, PathBuf};

pub use conn::{ConnHandlers, Connection, LogSink, RequestHandler, SHUTDOWN_GRACE};

pub fn resolve_executable(exe: &str, dir: &Path, path: &str) -> Option<PathBuf> {
    let named = mx_core::paths::expand_home(exe);
    if named.contains('/') {
        let full = if named.starts_with('/') { PathBuf::from(&named) } else { dir.join(&named) };
        return full.is_file().then_some(full);
    }
    path.split(':')
        .filter(|d| !d.is_empty())
        .map(|d| Path::new(d).join(&named))
        .find(|f| f.is_file())
}
