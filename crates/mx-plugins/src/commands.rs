use std::sync::LazyLock;

use mx_core::json::Obj;
use regex::{Captures, Regex};
use serde_json::Value;

use crate::manifest::{CommandTarget, PluginCommand};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandValues {
    pub cwd: String,
    pub folder: String,
    pub session_id: String,
    pub title: String,
    pub plugin_dir: String,
}

impl CommandValues {
    pub fn get(&self, name: &str) -> Option<&str> {
        Some(match name {
            "cwd" => &self.cwd,
            "folder" => &self.folder,
            "sessionId" => &self.session_id,
            "title" => &self.title,
            "pluginDir" => &self.plugin_dir,
            _ => return None,
        })
    }

    pub fn context(&self, tab_id: Option<&str>) -> Obj {
        let mut m = Obj::new();
        for key in ["cwd", "folder", "sessionId", "title", "pluginDir"] {
            m.insert(key.into(), Value::from(self.get(key).unwrap_or_default()));
        }
        if let Some(t) = tab_id {
            m.insert("tabId".into(), Value::from(t));
        }
        m
    }
}

pub fn sh_quote(v: &str) -> String {
    format!("'{}'", v.replace('\'', "'\\''"))
}

static VAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$\{([A-Za-z0-9_]+)\}").unwrap());

pub fn expand(template: &str, lookup: impl Fn(&str) -> Option<String>, quote: bool) -> String {
    if quote {
        return expand_for_shell(template, lookup);
    }
    VAR.replace_all(template, |c: &Captures| lookup(&c[1]).unwrap_or_else(|| c[0].to_owned())).into_owned()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Quoting {
    Bare,
    Single,
    Double,
    Backtick,
}

fn expand_for_shell(template: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut stack = vec![Quoting::Bare];
    let mut rest = template;
    while let Some(c) = rest.chars().next() {
        let top = *stack.last().unwrap_or(&Quoting::Bare);
        if let Some(found) = VAR.find(rest).filter(|m| m.start() == 0) {
            match lookup(&rest[2..found.end() - 1]) {
                None => out.push_str(found.as_str()),
                Some(v) => match top {
                    Quoting::Single => out.push_str(&format!("'{}'", sh_quote(&v))),
                    Quoting::Double => out.push_str(&format!("\"{}\"", sh_quote(&v))),
                    Quoting::Bare | Quoting::Backtick => out.push_str(&sh_quote(&v)),
                },
            }
            rest = &rest[found.end()..];
            continue;
        }
        let mut step = c.len_utf8();
        match (top, c) {
            (Quoting::Single, '\'') => {
                stack.pop();
            }
            (Quoting::Single, _) => {}
            (_, '\\') => step += rest[1..].chars().next().map_or(0, char::len_utf8),
            (Quoting::Double, '"') => {
                stack.pop();
            }
            (Quoting::Bare | Quoting::Backtick, '\'') => stack.push(Quoting::Single),
            (Quoting::Bare | Quoting::Backtick, '"') => stack.push(Quoting::Double),
            (Quoting::Backtick, '`') => {
                stack.pop();
            }
            (_, '`') => stack.push(Quoting::Backtick),
            (_, '$') if rest[1..].starts_with('(') => {
                stack.push(Quoting::Bare);
                step += 1;
            }
            (Quoting::Bare, ')') if stack.len() > 1 => {
                stack.pop();
            }
            _ => {}
        }
        out.push_str(&rest[..step]);
        rest = &rest[step..];
    }
    out
}

pub fn expand_values(template: &str, values: &CommandValues, quote: bool) -> String {
    expand(template, |k| values.get(k).map(str::to_owned), quote)
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandRun {
    Terminal { command: String },
    Background { command: String },
    Session { text: String },
    Plugin { context: Obj },
}

pub fn plan(command: &PluginCommand, values: &CommandValues, tab_id: Option<&str>) -> CommandRun {
    let run = || expand_values(command.run.as_deref().unwrap_or_default(), values, true);
    match command.target {
        CommandTarget::Terminal => CommandRun::Terminal { command: run() },
        CommandTarget::Background => CommandRun::Background { command: run() },
        CommandTarget::Session => {
            CommandRun::Session { text: expand_values(command.send.as_deref().unwrap_or_default(), values, false) }
        }
        CommandTarget::Plugin => CommandRun::Plugin { context: values.context(tab_id) },
    }
}

fn last_line(text: &str) -> Option<&str> {
    text.split('\n').rfind(|l| !l.trim().is_empty())
}

pub fn background_banner(title: &str, code: i32, stdout: &str, stderr: &str) -> (String, bool) {
    if code == 0 {
        return (format!("{title}: {}", last_line(stdout).unwrap_or("feito")), false);
    }
    let said = last_line(if stderr.is_empty() { stdout } else { stderr });
    let tail = said.map_or_else(String::new, |s| format!(": {s}"));
    (format!("{title} falhou (código {code}){tail}"), true)
}

pub fn no_session_banner(title: &str) -> String {
    format!("{title}: precisa de uma sessão do Claude rodando em foco")
}

pub fn failure_banner(title: &str, message: &str) -> String {
    format!("{title}: {message}")
}

#[cfg(test)]
mod tests {
    use super::expand;

    fn values(name: &str) -> Option<String> {
        match name {
            "title" => Some(r#"x" $(touch /tmp/pwned) '; rm -rf ~; '"#.to_owned()),
            "cwd" => Some("/repo/it's".to_owned()),
            _ => None,
        }
    }

    fn words(command: &str) -> Vec<String> {
        let script = format!("for w in {command}; do printf '%s\\n' \"$w\"; done");
        let out = std::process::Command::new("sh").arg("-c").arg(script).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap().lines().map(str::to_owned).collect()
    }

    #[test]
    fn bare_placeholders_keep_the_documented_single_quoting() {
        assert_eq!(
            expand("cd ${cwd} && echo ${title} ${nada}", values, true),
            r#"cd '/repo/it'\''s' && echo 'x" $(touch /tmp/pwned) '\''; rm -rf ~; '\''' ${nada}"#
        );
        assert_eq!(expand("revise ${cwd}", values, false), "revise /repo/it's");
    }

    #[test]
    fn placeholders_inside_quotes_cannot_break_out() {
        let title = values("title").unwrap();
        let cwd = values("cwd").unwrap();
        assert_eq!(words(&expand(r#""${title}""#, values, true)), [title.as_str()]);
        assert_eq!(words(&expand("'${cwd}'", values, true)), [cwd.as_str()]);
        assert_eq!(words(&expand(r#""[${title}] em ${cwd}""#, values, true)), [format!("[{title}] em {cwd}")]);
        assert_eq!(words(&expand("'a ${title} b'", values, true)), [format!("a {title} b")]);
        assert_eq!(words(&expand(r#""$(printf %s ${cwd})""#, values, true)), [cwd.as_str()]);
        assert_eq!(words(&expand(r#"\"${cwd}"#, values, true)), [format!("\"{cwd}")]);
    }
}
