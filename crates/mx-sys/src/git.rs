use std::sync::Arc;
use std::time::Duration;

pub use mx_core::model::worktree::{DETACHED, WorktreeInfo};
use mx_core::text::last_segment as basename;

use crate::runner::CommandRunner;
use crate::shell::{ShResult, q, stderr_or_stdout};

pub const FETCH_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOutcome {
    pub ok: bool,
    pub path: String,
    pub message: String,
}

impl GitOutcome {
    fn new(ok: bool, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self { ok, path: path.into(), message: message.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeSafety {
    pub dirty: Option<usize>,
    pub unmerged: u64,
    pub base: Option<String>,
}

impl WorktreeSafety {
    pub fn risky(&self) -> bool {
        self.dirty != Some(0) || self.unmerged > 0
    }

    pub fn force_branch(&self) -> bool {
        self.base.is_some() && self.risky()
    }
}

pub fn not_a_repo(r: &ShResult) -> bool {
    let e = &r.stderr;
    e.contains("not a git repository")
        || e.contains("not a work tree")
        || e.contains("no such file or directory")
}

pub fn parse_worktrees(porcelain: &str) -> Vec<WorktreeInfo> {
    let mut out = Vec::new();
    let mut path: Option<String> = None;
    let mut branch = DETACHED.to_owned();
    let mut prunable = false;
    let mut flush = |path: &mut Option<String>, branch: &mut String, prunable: &mut bool| {
        if let Some(p) = path.take() {
            let is_main = out.is_empty();
            out.push(WorktreeInfo {
                path: p,
                branch: std::mem::replace(branch, DETACHED.to_owned()),
                is_main,
                prunable: std::mem::take(prunable),
            });
        }
    };
    for line in porcelain.split('\n') {
        if let Some(rest) = line.strip_prefix("worktree ") {
            flush(&mut path, &mut branch, &mut prunable);
            path = Some(rest.trim().to_owned());
        } else if let Some(rest) = line.strip_prefix("branch ") {
            branch = rest.trim().replacen("refs/heads/", "", 1);
        } else if line.starts_with("prunable") {
            prunable = true;
        }
    }
    flush(&mut path, &mut branch, &mut prunable);
    out
}

pub fn count_lines(text: &str) -> usize {
    text.split('\n').filter(|l| !l.trim().is_empty()).count()
}

#[derive(Clone)]
pub struct Git {
    runner: Arc<dyn CommandRunner>,
}

impl Git {
    pub fn new(runner: Arc<dyn CommandRunner>) -> Self {
        Self { runner }
    }

    async fn sh(&self, command: &str, cwd: Option<&str>) -> ShResult {
        self.runner.run(command.to_owned(), cwd.map(str::to_owned)).await
    }

    pub async fn main_root(&self, path: &str) -> Option<String> {
        let r = self.sh("git worktree list --porcelain", Some(path)).await;
        if !r.ok() {
            return None;
        }
        r.stdout
            .split('\n')
            .find_map(|l| l.strip_prefix("worktree ").map(|p| p.trim().to_owned()))
    }

    pub async fn worktrees(&self, root: &str) -> Option<Vec<WorktreeInfo>> {
        let r = self.sh("git worktree list --porcelain", Some(root)).await;
        if !r.ok() {
            return not_a_repo(&r).then(Vec::new);
        }
        Some(parse_worktrees(&r.stdout))
    }

    pub async fn branch_of(&self, path: &str) -> String {
        let r = self.sh("git rev-parse --abbrev-ref HEAD", Some(path)).await;
        if r.ok() { r.stdout } else { String::new() }
    }

    pub async fn dirty_count(&self, path: &str) -> Option<usize> {
        let r = self.sh("git status --porcelain", Some(path)).await;
        if !r.ok() {
            return not_a_repo(&r).then_some(0);
        }
        Some(count_lines(&r.stdout))
    }

    pub async fn default_remote_ref(&self, root: &str) -> Option<String> {
        let r = self
            .sh("git symbolic-ref --quiet refs/remotes/origin/HEAD || echo", Some(root))
            .await;
        if r.ok() && !r.stdout.is_empty() {
            let named = r.stdout.replacen("refs/remotes/", "", 1);
            let v = self.sh(&format!("git rev-parse --verify --quiet {}", q(&named)), Some(root)).await;
            if v.ok() {
                return Some(named);
            }
        }
        for candidate in ["origin/master", "origin/main"] {
            let v = self.sh(&format!("git rev-parse --verify {}", q(candidate)), Some(root)).await;
            if v.ok() {
                return Some(candidate.to_owned());
            }
        }
        None
    }

    pub async fn add_worktree(
        &self,
        root: &str,
        dir_name: &str,
        branch: &str,
        base_ref: &str,
    ) -> GitOutcome {
        let path = format!("{root}/.claude/worktrees/{dir_name}");
        if self.sh(&format!("test -d {}", q(&path)), None).await.ok() {
            return GitOutcome::new(false, &path, format!("Worktree já existe em {path}"));
        }
        let _ = tokio::time::timeout(FETCH_TIMEOUT, self.sh("git fetch origin --quiet", Some(root))).await;
        let cmd = format!("git worktree add {} -b {} {}", q(&path), q(branch), q(base_ref));
        let r = self.sh(&cmd, Some(root)).await;
        if !r.ok() {
            return GitOutcome::new(false, &path, stderr_or_stdout(&r));
        }
        let message = format!("Worktree criada em {path} (branch {branch})");
        GitOutcome::new(true, path, message)
    }

    pub async fn safety_of(&self, w: &WorktreeInfo, root: &str) -> WorktreeSafety {
        if w.prunable {
            return WorktreeSafety { dirty: Some(0), unmerged: 0, base: None };
        }
        let dirty = self.dirty_count(&w.path).await;
        let mut base = self.default_remote_ref(root).await;
        let mut unmerged = 0;
        if let Some(b) = &base {
            let r = self.sh(&format!("git rev-list --count {}..HEAD", q(b)), Some(&w.path)).await;
            match r.ok().then(|| r.stdout.parse::<u64>().ok()).flatten() {
                Some(n) => unmerged = n,
                None => base = None,
            }
        }
        WorktreeSafety { dirty, unmerged, base }
    }

    pub async fn remove_worktree(
        &self,
        root: &str,
        worktree: &WorktreeInfo,
        force: bool,
        force_branch: Option<bool>,
        delete_branch: bool,
    ) -> GitOutcome {
        if worktree.is_main {
            return GitOutcome::new(
                false,
                &worktree.path,
                "Essa é a checkout principal — ela não se remove",
            );
        }
        let r = if worktree.prunable {
            self.sh("git worktree prune", Some(root)).await
        } else {
            let flag = if force { "--force " } else { "" };
            self.sh(&format!("git worktree remove {flag}{}", q(&worktree.path)), Some(root)).await
        };
        if !r.ok() {
            return GitOutcome::new(false, &worktree.path, stderr_or_stdout(&r));
        }
        let mut said = vec![if worktree.prunable {
            "Registros fantasmas limpos".to_owned()
        } else {
            format!("Worktree removida: {}", basename(&worktree.path))
        }];
        if delete_branch && worktree.branch != DETACHED {
            let flag = if force_branch.unwrap_or(force) { "-D" } else { "-d" };
            let b = self.sh(&format!("git branch {flag} {}", q(&worktree.branch)), Some(root)).await;
            said.push(if b.ok() {
                format!("branch {} apagada", worktree.branch)
            } else {
                format!("branch mantida ({})", stderr_or_stdout(&b))
            });
        }
        GitOutcome::new(true, &worktree.path, said.join(" · "))
    }

    pub async fn prune(&self, root: &str) -> GitOutcome {
        let r = self.sh("git worktree prune -v", Some(root)).await;
        if !r.ok() {
            return GitOutcome::new(false, root, stderr_or_stdout(&r));
        }
        let cleared = count_lines(&format!("{}\n{}", r.stdout, r.stderr));
        let message = if cleared == 0 {
            "Nenhuma worktree fantasma pra limpar".to_owned()
        } else {
            format!("{cleared} registro(s) limpo(s)")
        };
        GitOutcome::new(true, root, message)
    }
}
