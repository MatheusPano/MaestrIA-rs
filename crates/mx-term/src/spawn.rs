use std::collections::HashMap;
use std::path::PathBuf;

pub const TERM: &str = "xterm-256color";
pub const DEFAULT_LANG: &str = "en_US.UTF-8";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Launch {
    Shell,
    Command(String),
    Program { program: String, args: Vec<String> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnPlan {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
}

pub fn argv(shell: &str, launch: &Launch) -> (String, Vec<String>) {
    match launch {
        Launch::Shell => (shell.to_owned(), vec!["-l".into()]),
        Launch::Command(cmd) => (shell.to_owned(), vec!["-lc".into(), format!("exec {cmd}")]),
        Launch::Program { program, args } => (program.clone(), args.clone()),
    }
}

pub fn child_env(
    base: &HashMap<String, String>,
    caller: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut env = base.clone();
    env.insert("TERM".into(), TERM.into());
    env.insert("COLORTERM".into(), "truecolor".into());
    env.entry("LANG".into()).or_insert_with(|| DEFAULT_LANG.into());
    env.insert("FORCE_HYPERLINK".into(), "1".into());
    env.extend(caller.iter().map(|(k, v)| (k.clone(), v.clone())));
    env
}

pub fn plan(
    shell: &str,
    base_env: &HashMap<String, String>,
    launch: &Launch,
    cwd: Option<PathBuf>,
    caller_env: &HashMap<String, String>,
) -> SpawnPlan {
    let (program, args) = argv(shell, launch);
    SpawnPlan {
        program,
        args,
        cwd,
        env: child_env(base_env, caller_env),
    }
}

pub fn note_line(display: &str) -> String {
    format!("\x1b[2m$ {display}\x1b[0m\r\n")
}

pub fn remark_line(text: &str) -> String {
    format!("\x1b[2m[{text}]\x1b[0m\r\n")
}

pub fn missing_cwd_line(cwd: &str) -> String {
    format!("\x1b[31ma pasta {cwd} não existe — nada foi iniciado.\x1b[0m\r\n")
}

pub fn exit_line(code: Option<i32>) -> String {
    let code = code.map_or_else(|| "null".to_owned(), |c| c.to_string());
    format!("\r\n\x1b[2m[processo saiu com {code}]\x1b[0m\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| ((*k).into(), (*v).into())).collect()
    }

    #[test]
    fn shell_is_a_login_shell() {
        let (p, a) = argv("/bin/zsh", &Launch::Shell);
        assert_eq!((p.as_str(), a), ("/bin/zsh", vec!["-l".to_owned()]));
    }

    #[test]
    fn command_is_exec_inside_login_shell() {
        let (_, a) = argv("/bin/bash", &Launch::Command("btop --x".into()));
        assert_eq!(a, ["-lc", "exec btop --x"]);
    }

    #[test]
    fn env_forces_term_and_hyperlinks_and_keeps_lang() {
        let base = env(&[("TERM", "xterm-kitty"), ("LANG", "pt_BR.UTF-8"), ("PATH", "/x")]);
        let e = child_env(&base, &HashMap::new());
        assert_eq!(e["TERM"], "xterm-256color");
        assert_eq!(e["COLORTERM"], "truecolor");
        assert_eq!(e["FORCE_HYPERLINK"], "1");
        assert_eq!(e["LANG"], "pt_BR.UTF-8");
        assert_eq!(e["PATH"], "/x");
    }

    #[test]
    fn env_sets_lang_only_when_absent_and_caller_wins() {
        let e = child_env(&HashMap::new(), &env(&[("FORCE_HYPERLINK", "0"), ("X", "1")]));
        assert_eq!(e["LANG"], DEFAULT_LANG);
        assert_eq!(e["FORCE_HYPERLINK"], "0");
        assert_eq!(e["X"], "1");
    }

    #[test]
    fn buffer_lines_are_byte_exact() {
        assert_eq!(note_line("ls"), "\x1b[2m$ ls\x1b[0m\r\n");
        assert_eq!(remark_line("oi"), "\x1b[2m[oi]\x1b[0m\r\n");
        assert_eq!(exit_line(Some(3)), "\r\n\x1b[2m[processo saiu com 3]\x1b[0m\r\n");
        assert_eq!(exit_line(None), "\r\n\x1b[2m[processo saiu com null]\x1b[0m\r\n");
        assert_eq!(
            missing_cwd_line("/nao"),
            "\x1b[31ma pasta /nao não existe — nada foi iniciado.\x1b[0m\r\n"
        );
    }
}
