use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use super::QuitPlan;

pub const INSTALL_ENV: &str = "MAESTRIA_UPDATE_INSTALL";
pub const RELAUNCH_ENV: &str = "MAESTRIA_UPDATE_RELAUNCH";
pub const LAUNCHER: &str = "/usr/bin/maestria-rs";

#[cfg(unix)]
const RELAUNCH: &str = "while kill -0 \"$1\" 2>/dev/null; do sleep 0.2; done\nsetsid \"$2\" >/dev/null 2>&1 < /dev/null &\n";

#[cfg(unix)]
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

#[cfg(unix)]
fn detached(script: &str, pid: u32, target: &Path, staged: Option<&Path>, relaunch: bool) {
    let mut cmd = std::process::Command::new("/bin/sh");
    mx_core::proc::isolate(&mut cmd);
    let _ = cmd
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
        .spawn();
}

#[cfg(unix)]
pub fn on_quit(pid: u32, plan: &QuitPlan) {
    match plan {
        QuitPlan::Swap { bundle, staged, relaunch } => detached(SWAP, pid, bundle, Some(staged), *relaunch),
        QuitPlan::Relaunch => detached(RELAUNCH, pid, &relaunch_target(), None, true),
    }
}

fn ps_quote(value: &Path) -> String {
    crate::shell::ps_quote(&value.to_string_lossy())
}

pub fn windows_quit_script(pid: u32, exe: &Path, msi: Option<&Path>, relaunch: bool) -> String {
    let mut script = format!("Wait-Process -Id {pid} -ErrorAction SilentlyContinue\n");
    if let Some(msi) = msi {
        script.push_str(&format!(
            "Start-Process msiexec.exe -Wait -ArgumentList @('/i', {}, '/passive', '/norestart')\n",
            ps_quote(Path::new(&format!("\"{}\"", msi.display())))
        ));
        script.push_str(&format!("Remove-Item -Recurse -Force -ErrorAction SilentlyContinue {}\n", ps_quote(msi.parent().unwrap_or(msi))));
    }
    if relaunch {
        script.push_str(&format!("Start-Process {}\n", ps_quote(exe)));
    }
    script
}

#[cfg(windows)]
pub fn on_quit(pid: u32, plan: &QuitPlan) {
    use base64::Engine;
    let script = match plan {
        QuitPlan::Swap { bundle, staged, relaunch } => windows_quit_script(pid, bundle, Some(staged), *relaunch),
        QuitPlan::Relaunch => {
            let exe = std::env::current_exe().unwrap_or_else(|_| relaunch_target());
            windows_quit_script(pid, &exe, None, true)
        }
    };
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut cmd = std::process::Command::new("powershell.exe");
    mx_core::proc::isolate(&mut cmd);
    let _ = cmd
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-EncodedCommand"])
        .arg(base64::engine::general_purpose::STANDARD.encode(utf16))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}
