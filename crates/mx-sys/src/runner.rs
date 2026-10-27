use std::sync::Mutex;

pub use mx_core::ports::{BoxFuture, CommandOutput, CommandRunner};

use crate::shell::{self, ShResult};

#[derive(Debug, Default, Clone, Copy)]
pub struct LoginShell;

impl CommandRunner for LoginShell {
    fn run(&self, command: String, cwd: Option<String>) -> BoxFuture<'_, CommandOutput> {
        Box::pin(async move { shell::run(&command, cwd.as_deref()).await })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub command: String,
    pub cwd: Option<String>,
}

struct Rule {
    prefix: String,
    result: ShResult,
    once: bool,
}

pub struct FakeRunner {
    rules: Mutex<Vec<Rule>>,
    calls: Mutex<Vec<Call>>,
    fallback: ShResult,
}

impl Default for FakeRunner {
    fn default() -> Self {
        Self {
            rules: Mutex::new(Vec::new()),
            calls: Mutex::new(Vec::new()),
            fallback: shell::output(1, "", ""),
        }
    }
}

impl FakeRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_fallback(mut self, result: ShResult) -> Self {
        self.fallback = result;
        self
    }

    pub fn respond(&self, prefix: &str, result: ShResult) -> &Self {
        self.push(prefix, result, false)
    }

    pub fn respond_once(&self, prefix: &str, result: ShResult) -> &Self {
        self.push(prefix, result, true)
    }

    fn push(&self, prefix: &str, result: ShResult, once: bool) -> &Self {
        self.rules.lock().unwrap().push(Rule { prefix: prefix.into(), result, once });
        self
    }

    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    pub fn commands(&self) -> Vec<String> {
        self.calls().into_iter().map(|c| c.command).collect()
    }

    fn answer(&self, command: &str) -> ShResult {
        let mut rules = self.rules.lock().unwrap();
        let hit = rules
            .iter()
            .position(|r| r.once && command.starts_with(&r.prefix))
            .or_else(|| rules.iter().position(|r| command.starts_with(&r.prefix)));
        match hit {
            Some(i) if rules[i].once => rules.remove(i).result,
            Some(i) => rules[i].result.clone(),
            None => self.fallback.clone(),
        }
    }
}

impl CommandRunner for FakeRunner {
    fn run(&self, command: String, cwd: Option<String>) -> BoxFuture<'_, CommandOutput> {
        let result = self.answer(&command);
        self.calls.lock().unwrap().push(Call { command, cwd });
        Box::pin(async move { result })
    }
}
