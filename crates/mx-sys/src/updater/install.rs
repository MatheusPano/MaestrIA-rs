use std::path::Path;
use std::process::Stdio;

const RELAUNCH: &str = "while kill -0 \"$1\" 2>/dev/null; do sleep 0.2; done\nsetsid \"$2\" >/dev/null 2>&1 < /dev/null &\n";

pub fn install_error(code: Option<i32>, stderr: &str, deb: &Path) -> String {
    if code == Some(126) {
        return "Instalação cancelada".to_owned();
    }
    let last = stderr.lines().rfind(|l| !l.trim().is_empty()).unwrap_or("").trim();
    format!("Não deu pra instalar ({last}). No terminal: sudo apt install {}", deb.display())
}

pub async fn install(deb: &Path) -> Result<(), String> {
    let out = tokio::process::Command::new("pkexec")
        .args(["apt-get", "install", "-y"])
        .arg(deb)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    Err(install_error(out.status.code(), &String::from_utf8_lossy(&out.stderr), deb))
}

pub fn relaunch_on_quit(pid: u32, exe: &Path) {
    let _ = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(RELAUNCH)
        .arg("maestria-update")
        .arg(pid.to_string())
        .arg(exe)
        .arg("")
        .arg("1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}
