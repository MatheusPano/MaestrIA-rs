use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::OnceLock;

use crate::Platform;

pub use mx_core::ports::CommandOutput as ShResult;

pub fn output(code: i32, stdout: impl Into<String>, stderr: impl Into<String>) -> ShResult {
    ShResult { code, stdout: stdout.into(), stderr: stderr.into() }
}

pub fn stderr_or_stdout(r: &ShResult) -> &str {
    if r.stderr.is_empty() { &r.stdout } else { &r.stderr }
}

const SHELL_CANDIDATES: [&str; 4] = ["/bin/bash", "/usr/bin/bash", "/bin/zsh", "/usr/bin/zsh"];

pub fn resolve_shell(
    platform: Platform,
    env_shell: Option<&str>,
    exists: impl Fn(&str) -> bool,
) -> String {
    if platform == Platform::MacOs {
        return "/bin/zsh".into();
    }
    if let Some(s) = env_shell.filter(|s| !s.is_empty() && exists(s)) {
        return s.into();
    }
    SHELL_CANDIDATES
        .iter()
        .find(|c| exists(c))
        .map_or_else(|| "/bin/sh".into(), |c| (*c).into())
}

pub fn shell() -> &'static str {
    static SHELL: OnceLock<String> = OnceLock::new();
    SHELL.get_or_init(|| {
        let env_shell = std::env::var("SHELL").ok();
        resolve_shell(Platform::current(), env_shell.as_deref(), |p| Path::new(p).is_file())
    })
}

pub fn q(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

pub fn bin_dirs(home: &str, platform: Platform) -> Vec<String> {
    let mut dirs = Vec::new();
    if !home.is_empty() {
        for rel in [
            ".local/bin",
            ".claude/local",
            ".bun/bin",
            ".volta/bin",
            ".npm-global/bin",
            ".pub-cache/bin",
            ".cargo/bin",
        ] {
            dirs.push(format!("{home}/{rel}"));
        }
    }
    for d in ["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin", "/usr/local/sbin"] {
        dirs.push(d.into());
    }
    if platform == Platform::Linux {
        dirs.push("/home/linuxbrew/.linuxbrew/bin".into());
    }
    dirs
}

pub fn path_with_fallbacks<S: AsRef<str>>(
    inherited: &str,
    candidates: &[S],
    exists: impl Fn(&str) -> bool,
) -> String {
    let mut dirs: Vec<&str> = inherited.split(':').filter(|d| !d.is_empty()).collect();
    for dir in candidates {
        let dir = dir.as_ref();
        if dirs.contains(&dir) || !exists(dir) {
            continue;
        }
        dirs.push(dir);
    }
    dirs.join(":")
}

pub fn env() -> &'static HashMap<String, String> {
    static ENV: OnceLock<HashMap<String, String>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env: HashMap<String, String> = std::env::vars().collect();
        let home = env.get("HOME").cloned().unwrap_or_default();
        let inherited = env.get("PATH").cloned().unwrap_or_default();
        let path = path_with_fallbacks(&inherited, &bin_dirs(&home, Platform::current()), |d| {
            Path::new(d).is_dir()
        });
        env.insert("PATH".into(), path);
        env
    })
}

pub fn profile_file(platform: Platform) -> &'static str {
    match platform {
        Platform::MacOs => "~/.zprofile",
        Platform::Linux => "~/.profile",
    }
}

pub fn early_exit_message(title: &str, code: Option<i32>, platform: Platform) -> String {
    if code == Some(127) {
        return format!(
            "{title}: não achei o `claude` no PATH do app — instale-o, ou ponha a pasta do binário no {} (o rc do shell interativo o app não lê)",
            profile_file(platform)
        );
    }
    let code = code.map_or_else(|| "?".to_owned(), |c| c.to_string());
    format!("{title}: o Claude saiu na largada (código {code}) — abra o painel pra ver o motivo")
}

pub async fn run(command: &str, cwd: Option<&str>) -> ShResult {
    let mut cmd = tokio::process::Command::new(shell());
    cmd.arg("-lc")
        .arg(command)
        .env_clear()
        .envs(env())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    match cmd.output().await {
        Ok(out) => output(
            exit_code(&out.status),
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim(),
        ),
        Err(e) => ShResult::spawn_failure(e.to_string()),
    }
}

#[cfg(unix)]
fn exit_code(status: &std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status.code().or_else(|| status.signal().map(|s| -s)).unwrap_or(-1)
}

#[cfg(not(unix))]
fn exit_code(status: &std::process::ExitStatus) -> i32 {
    status.code().unwrap_or(-1)
}
