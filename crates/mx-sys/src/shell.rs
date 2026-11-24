use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use mx_core::proc::PATH_LIST_SEP;

use crate::Platform;

pub use mx_core::ports::CommandOutput as ShResult;

pub fn output(code: i32, stdout: impl Into<String>, stderr: impl Into<String>) -> ShResult {
    ShResult { code, stdout: stdout.into(), stderr: stderr.into() }
}

pub fn stderr_or_stdout(r: &ShResult) -> &str {
    if r.stderr.is_empty() { &r.stdout } else { &r.stderr }
}

const SHELL_CANDIDATES: [&str; 4] = ["/bin/bash", "/usr/bin/bash", "/bin/zsh", "/usr/bin/zsh"];

pub const GIT_BASH_ENV: &str = "CLAUDE_CODE_GIT_BASH_PATH";
const GIT_BASH_DEFAULT: &str = r"C:\Program Files\Git\bin\bash.exe";

pub fn git_bash_from_git(git_exe: &Path) -> Option<std::path::PathBuf> {
    let root = git_exe.parent()?.parent()?;
    Some(root.join("bin").join("bash.exe"))
}

pub fn resolve_git_bash(
    env_override: Option<&str>,
    git_on_path: Option<&Path>,
    exists: impl Fn(&str) -> bool,
) -> String {
    if let Some(s) = env_override.filter(|s| !s.is_empty() && exists(s)) {
        return s.into();
    }
    git_on_path
        .and_then(git_bash_from_git)
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|p| exists(p))
        .unwrap_or_else(|| GIT_BASH_DEFAULT.into())
}

pub fn which(program: &str, path: &str) -> Option<std::path::PathBuf> {
    let exts: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat", ""] } else { &[""] };
    path.split(PATH_LIST_SEP)
        .filter(|d| !d.is_empty())
        .flat_map(|d| exts.iter().map(move |e| with_suffix(Path::new(d).join(program), e)))
        .find(|f| f.is_file())
}

pub fn with_suffix(path: std::path::PathBuf, suffix: &str) -> std::path::PathBuf {
    let mut name = path.into_os_string();
    name.push(suffix);
    name.into()
}

pub fn resolve_shell(
    platform: Platform,
    env_shell: Option<&str>,
    exists: impl Fn(&str) -> bool,
) -> String {
    if platform == Platform::Windows {
        let path = std::env::var("PATH").unwrap_or_default();
        let git = which("git", &path);
        return resolve_git_bash(std::env::var(GIT_BASH_ENV).ok().as_deref(), git.as_deref(), exists);
    }
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

pub fn interactive_shell() -> &'static str {
    static SHELL: OnceLock<String> = OnceLock::new();
    SHELL.get_or_init(|| {
        if Platform::current() != Platform::Windows {
            return shell().to_owned();
        }
        let path = std::env::var("PATH").unwrap_or_default();
        which("pwsh", &path)
            .or_else(|| which("powershell", &path))
            .map_or_else(|| "powershell.exe".to_owned(), |p| p.to_string_lossy().into_owned())
    })
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

pub fn q_operand(value: &str) -> String {
    if value.starts_with('-') { q(&format!("./{value}")) } else { q(value) }
}

pub fn bin_dirs(home: &str, platform: Platform) -> Vec<String> {
    let mut dirs = Vec::new();
    if platform == Platform::Windows {
        if !home.is_empty() {
            dirs.push(format!("{home}\\.local\\bin"));
            dirs.push(format!("{home}\\AppData\\Roaming\\npm"));
            dirs.push(format!("{home}\\.bun\\bin"));
            dirs.push(format!("{home}\\.cargo\\bin"));
        }
        return dirs;
    }
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
    let mut dirs: Vec<&str> = inherited.split(PATH_LIST_SEP).filter(|d| !d.is_empty()).collect();
    for dir in candidates {
        let dir = dir.as_ref();
        if dirs.contains(&dir) || !exists(dir) {
            continue;
        }
        dirs.push(dir);
    }
    dirs.join(&PATH_LIST_SEP.to_string())
}

pub fn env() -> &'static HashMap<String, String> {
    static ENV: OnceLock<HashMap<String, String>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env: HashMap<String, String> = std::env::vars().collect();
        let home = mx_core::paths::home().unwrap_or_default();
        let path_key = if cfg!(windows) {
            env.keys().find(|k| k.eq_ignore_ascii_case("PATH")).cloned().unwrap_or_else(|| "PATH".into())
        } else {
            "PATH".into()
        };
        let inherited = env.remove(&path_key).unwrap_or_default();
        let path = path_with_fallbacks(&inherited, &bin_dirs(&home, Platform::current()), |d| {
            Path::new(d).is_dir()
        });
        env.insert("PATH".into(), path);
        restore_input_method(&mut env);
        env
    })
}

pub const SAVED_XMODIFIERS: &str = "MAESTRIA_SAVED_XMODIFIERS";

pub fn restore_input_method(env: &mut HashMap<String, String>) {
    match env.remove(SAVED_XMODIFIERS) {
        Some(saved) if !saved.is_empty() => {
            env.insert("XMODIFIERS".into(), saved);
        }
        Some(_) => {
            env.remove("XMODIFIERS");
        }
        None => {}
    }
}

pub fn profile_file(platform: Platform) -> &'static str {
    match platform {
        Platform::MacOs => "~/.zprofile",
        Platform::Linux => "~/.profile",
        Platform::Windows => "PATH das variáveis de ambiente do usuário",
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

pub const TIMED_OUT: i32 = 124;

pub async fn run(command: &str, cwd: Option<&str>) -> ShResult {
    run_with_timeout(command, cwd, None).await
}

pub async fn run_with_timeout(command: &str, cwd: Option<&str>, limit: Option<Duration>) -> ShResult {
    let mut cmd = tokio::process::Command::new(shell());
    cmd.arg("-lc")
        .arg(command)
        .env_clear()
        .envs(env())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    mx_core::proc::isolate(cmd.as_std_mut());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => return ShResult::spawn_failure(e.to_string()),
    };
    let pid = child.id();
    let waited = child.wait_with_output();
    tokio::pin!(waited);
    let mut group = GroupKill(pid);
    let done = match limit {
        None => (&mut waited).await,
        Some(limit) => match tokio::time::timeout(limit, &mut waited).await {
            Ok(done) => done,
            Err(_) => {
                return output(TIMED_OUT, "", format!("timed out after {} ms", limit.as_millis()));
            }
        },
    };
    group.0 = None;
    match done {
        Ok(out) => output(
            mx_core::proc::exit_code(&out.status),
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim(),
        ),
        Err(e) => ShResult::spawn_failure(e.to_string()),
    }
}

struct GroupKill(Option<u32>);

impl Drop for GroupKill {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            mx_core::proc::kill_group_now(pid);
        }
    }
}

pub fn ps_quote(value: &str) -> String {
    let doubled: String = value.chars().flat_map(|c| if PS_QUOTES.contains(&c) { vec![c, c] } else { vec![c] }).collect();
    format!("'{doubled}'")
}

const PS_QUOTES: [char; 5] = ['\'', '\u{2018}', '\u{2019}', '\u{201A}', '\u{201B}'];
