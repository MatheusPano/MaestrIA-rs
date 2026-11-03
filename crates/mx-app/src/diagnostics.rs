use std::backtrace::Backtrace;
use std::fs::{self, File, OpenOptions};
use std::io::{IsTerminal, Write};
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};

pub const CRASH_LOG: &str = "crash.log";
pub const APP_LOG: &str = "maestria.log";
const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;

// Release builds abort on panic and an app opened from Finder has no stderr, so without these
// files a crash leaves no trace at all.
pub fn install() {
    let dir = mx_core::paths::state_dir();
    let _ = fs::create_dir_all(&dir);
    init_logger(&dir);
    let crash = dir.join(CRASH_LOG);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write_crash(&crash, info);
        previous(info);
    }));
}

fn init_logger(dir: &Path) {
    let mut builder = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    if !std::io::stderr().is_terminal()
        && let Some(file) = open_rotated(&dir.join(APP_LOG))
    {
        builder.target(env_logger::Target::Pipe(Box::new(file)));
    }
    builder.init();
}

fn open_rotated(path: &Path) -> Option<File> {
    if fs::metadata(path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
        let _ = fs::rename(path, rotated(path));
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}

fn rotated(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".old");
    PathBuf::from(name)
}

fn write_crash(path: &Path, info: &PanicHookInfo<'_>) {
    let Some(mut file) = open_rotated(path) else { return };
    let thread = std::thread::current();
    let report = format!(
        "=== {} maestria {} ===\nthread '{}' panicked at {}\n{}\n\n{}\n",
        chrono::Local::now().to_rfc3339(),
        env!("CARGO_PKG_VERSION"),
        thread.name().unwrap_or("<unnamed>"),
        info.location().map(|l| l.to_string()).unwrap_or_else(|| "<unknown>".into()),
        message(info),
        Backtrace::force_capture(),
    );
    let _ = file.write_all(report.as_bytes());
    let _ = file.sync_all();
}

fn message(info: &PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "<non-string panic payload>".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_log_over_the_limit_moves_aside_and_a_fresh_one_starts() {
        let dir = tempdir();
        let path = dir.join(APP_LOG);
        fs::write(&path, vec![b'x'; MAX_LOG_BYTES as usize + 1]).unwrap();
        let mut file = open_rotated(&path).unwrap();
        file.write_all(b"novo").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"novo");
        assert_eq!(fs::metadata(rotated(&path)).unwrap().len(), MAX_LOG_BYTES + 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_small_log_keeps_growing_in_place() {
        let dir = tempdir();
        let path = dir.join(CRASH_LOG);
        fs::write(&path, b"antes\n").unwrap();
        open_rotated(&path).unwrap().write_all(b"depois\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "antes\ndepois\n");
        assert!(!rotated(&path).exists());
        let _ = fs::remove_dir_all(dir);
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mx-diag-{}-{:?}", std::process::id(), std::thread::current().id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
