use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use super::QuitPlan;

pub const INSTALL_ENV: &str = "MAESTRIA_UPDATE_INSTALL";
pub const RELAUNCH_ENV: &str = "MAESTRIA_UPDATE_RELAUNCH";
pub const LAUNCHER: &str = "/usr/bin/maestria-rs";

const RELAUNCH: &str = "while kill -0 \"$1\" 2>/dev/null; do sleep 0.2; done\nsetsid \"$2\" >/dev/null 2>&1 < /dev/null &\n";

const SWAP: &str = r#"while kill -0 "$1" 2>/dev/null; do sleep 0.2; done
old="$2.old"
rm -rf "$old"
if mv "$2" "$old"; then
  if ditto "$3" "$2"; then
    rm -rf "$old" "$(dirname "$3")"
  else
    rm -rf "$2"; mv "$old" "$2"
  fi
fi
if [ -n "$4" ]; then open "$2"; fi
"#;

fn env_path(name: &str) -> Option<OsString> {
    std::env::var_os(name).filter(|v| !v.is_empty())
}

pub fn install_command(deb: &Path) -> (OsString, Vec<OsString>) {
    match env_path(INSTALL_ENV) {
        Some(program) => (program, vec![deb.into()]),
        None => ("pkexec".into(), vec!["apt-get".into(), "install".into(), "-y".into(), deb.into()]),
    }
}

pub fn relaunch_target() -> PathBuf {
    env_path(RELAUNCH_ENV).map_or_else(|| PathBuf::from(LAUNCHER), PathBuf::from)
}

pub fn install_error(code: Option<i32>, stderr: &str, deb: &Path) -> String {
    if code == Some(126) {
        return "Instalação cancelada".to_owned();
    }
    let last = stderr.lines().rfind(|l| !l.trim().is_empty()).unwrap_or("").trim();
    format!("Não deu pra instalar ({last}). No terminal: sudo apt install {}", deb.display())
}

pub async fn install(deb: &Path) -> Result<(), String> {
    let (program, args) = install_command(deb);
    let out = tokio::process::Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    Err(install_error(out.status.code(), &String::from_utf8_lossy(&out.stderr), deb))
}

fn detached(script: &str, pid: u32, target: &Path, staged: Option<&Path>, relaunch: bool) {
    let _ = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .arg("maestria-update")
        .arg(pid.to_string())
        .arg(target)
        .arg(staged.map_or_else(OsString::new, |p| p.as_os_str().to_owned()))
        .arg(if relaunch { "1" } else { "" })
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();
}

pub fn on_quit(pid: u32, plan: &QuitPlan) {
    match plan {
        QuitPlan::Swap { bundle, staged, relaunch } => detached(SWAP, pid, bundle, Some(staged), *relaunch),
        QuitPlan::Relaunch => detached(RELAUNCH, pid, &relaunch_target(), None, true),
    }
}
