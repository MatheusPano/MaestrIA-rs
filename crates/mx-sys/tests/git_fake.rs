use std::sync::Arc;

use mx_sys::git::{Git, WorktreeInfo, WorktreeSafety, parse_worktrees};
use mx_sys::runner::Call;
use mx_sys::{FakeRunner, ShResult, output};

fn ok(out: &str) -> ShResult {
    output(0, out, "")
}

fn fail(err: &str) -> ShResult {
    output(128, "", err)
}

fn git(fake: &Arc<FakeRunner>) -> Git {
    Git::new(fake.clone())
}

fn wt(path: &str, branch: &str, is_main: bool, prunable: bool) -> WorktreeInfo {
    WorktreeInfo { path: path.into(), branch: branch.into(), is_main, prunable }
}

const PORCELAIN: &str = "worktree /repo\nHEAD abc\nbranch refs/heads/master\n\n\
worktree /repo/.claude/worktrees/TASK-47730\nHEAD def\nbranch refs/heads/feature/TASK#47730\n\n\
worktree /gone/TASK-45371\nHEAD 123\ndetached\nprunable gitdir file points to non-existent location\n";

#[test]
fn porcelain_blocks_parse_into_worktrees_with_the_first_as_main() {
    assert_eq!(
        parse_worktrees(PORCELAIN),
        vec![
            wt("/repo", "master", true, false),
            wt("/repo/.claude/worktrees/TASK-47730", "feature/TASK#47730", false, false),
            wt("/gone/TASK-45371", "(detached)", false, true),
        ]
    );
}

#[test]
fn short_label_prefers_the_task_id_then_the_last_branch_segment_then_the_path() {
    assert_eq!(wt("/x", "feature/TASK#47730", false, false).short_label(), "TASK#47730");
    assert_eq!(wt("/x", "feature/bar", false, false).short_label(), "bar");
    assert_eq!(wt("/a/TASK-1", "fix/", false, false).short_label(), "TASK-1");
}

#[tokio::test]
async fn worktrees_runs_porcelain_list_in_the_root() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond("git worktree list --porcelain", ok(PORCELAIN));
    let list = git(&fake).worktrees("/repo").await.unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(
        fake.calls(),
        vec![Call { command: "git worktree list --porcelain".into(), cwd: Some("/repo".into()) }]
    );
}

#[tokio::test]
async fn worktrees_of_a_non_repo_is_empty_but_other_failures_are_none() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond_once("git worktree", fail("fatal: not a git repository (or any parent)"));
    fake.respond_once("git worktree", fail("fatal: Unable to create index.lock"));
    let g = git(&fake);
    assert_eq!(g.worktrees("/x").await, Some(vec![]));
    assert_eq!(g.worktrees("/x").await, None);
}

#[tokio::test]
async fn main_root_is_the_first_worktree_line() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond_once("git worktree", ok(PORCELAIN));
    fake.respond_once("git worktree", fail("x"));
    let g = git(&fake);
    assert_eq!(g.main_root("/repo/sub").await.as_deref(), Some("/repo"));
    assert_eq!(g.main_root("/repo/sub").await, None);
}

#[tokio::test]
async fn branch_of_is_stdout_or_empty() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond_once("git rev-parse --abbrev-ref HEAD", ok("main"));
    let g = git(&fake);
    assert_eq!(g.branch_of("/r").await, "main");
    assert_eq!(g.branch_of("/r").await, "");
}

#[tokio::test]
async fn dirty_count_counts_non_blank_lines_and_tells_non_repo_from_failure() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond_once("git status", ok(" M a\n?? b\n\n"));
    fake.respond_once("git status", ok(""));
    fake.respond_once("git status", fail("fatal: not a work tree"));
    fake.respond_once("git status", fail("index.lock exists"));
    let g = git(&fake);
    assert_eq!(g.dirty_count("/r").await, Some(2));
    assert_eq!(g.dirty_count("/r").await, Some(0));
    assert_eq!(g.dirty_count("/r").await, Some(0));
    assert_eq!(g.dirty_count("/r").await, None);
}

#[tokio::test]
async fn default_remote_ref_verifies_origin_head_before_trusting_it() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond("git symbolic-ref", ok("refs/remotes/origin/main"));
    fake.respond("git rev-parse --verify --quiet 'origin/main'", ok("abc"));
    assert_eq!(git(&fake).default_remote_ref("/r").await.as_deref(), Some("origin/main"));
    assert_eq!(
        fake.commands(),
        [
            "git symbolic-ref --quiet refs/remotes/origin/HEAD || echo",
            "git rev-parse --verify --quiet 'origin/main'",
        ]
    );
}

#[tokio::test]
async fn default_remote_ref_falls_back_to_master_then_main() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond("git symbolic-ref", ok("refs/remotes/origin/trunk"));
    fake.respond("git rev-parse --verify 'origin/main'", ok("abc"));
    assert_eq!(git(&fake).default_remote_ref("/r").await.as_deref(), Some("origin/main"));
    assert_eq!(
        fake.commands(),
        [
            "git symbolic-ref --quiet refs/remotes/origin/HEAD || echo",
            "git rev-parse --verify --quiet 'origin/trunk'",
            "git rev-parse --verify 'origin/master'",
            "git rev-parse --verify 'origin/main'",
        ]
    );
    let none = Arc::new(FakeRunner::new());
    none.respond("git symbolic-ref", ok(""));
    assert_eq!(git(&none).default_remote_ref("/r").await, None);
    assert_eq!(none.commands().len(), 3);
}

#[tokio::test]
async fn add_worktree_refuses_an_existing_dir() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond("test -d", ok(""));
    let out = git(&fake).add_worktree("/r", "TASK-1", "feature/TASK#1", "origin/main").await;
    assert!(!out.ok);
    assert_eq!(out.path, "/r/.claude/worktrees/TASK-1");
    assert_eq!(out.message, "Worktree já existe em /r/.claude/worktrees/TASK-1");
    assert_eq!(
        fake.calls(),
        vec![Call { command: "test -d '/r/.claude/worktrees/TASK-1'".into(), cwd: None }]
    );
}

#[tokio::test]
async fn add_worktree_fetches_then_adds_with_exact_argv() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond("git fetch", fail("no origin"));
    fake.respond("git worktree add", ok(""));
    let out = git(&fake).add_worktree("/r", "TASK-1", "feature/TASK#1", "origin/main").await;
    assert!(out.ok);
    assert_eq!(out.message, "Worktree criada em /r/.claude/worktrees/TASK-1 (branch feature/TASK#1)");
    let calls = fake.calls();
    assert_eq!(calls[1], Call { command: "git fetch origin --quiet".into(), cwd: Some("/r".into()) });
    assert_eq!(
        calls[2].command,
        "git worktree add '/r/.claude/worktrees/TASK-1' -b 'feature/TASK#1' 'origin/main'"
    );
}

#[tokio::test]
async fn add_worktree_failure_reports_stderr_or_stdout() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond_once("git worktree add", output(1, "only out", ""));
    fake.respond_once("git worktree add", output(1, "out", "fatal: x"));
    let g = git(&fake);
    assert_eq!(g.add_worktree("/r", "d", "b", "base").await.message, "only out");
    assert_eq!(g.add_worktree("/r", "d", "b", "base").await.message, "fatal: x");
}

#[tokio::test]
async fn safety_of_a_prunable_worktree_runs_nothing() {
    let fake = Arc::new(FakeRunner::new());
    let s = git(&fake).safety_of(&wt("/w", "b", false, true), "/r").await;
    assert_eq!(s, WorktreeSafety { dirty: Some(0), unmerged: 0, base: None });
    assert!(fake.calls().is_empty());
}

#[tokio::test]
async fn safety_of_counts_dirty_and_unmerged_against_the_base() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond("git status", ok(" M a"));
    fake.respond("git symbolic-ref", ok(""));
    fake.respond("git rev-parse --verify 'origin/master'", ok("x"));
    fake.respond("git rev-list --count 'origin/master'..HEAD", ok("3"));
    let s = git(&fake).safety_of(&wt("/w", "b", false, false), "/r").await;
    assert_eq!(s, WorktreeSafety { dirty: Some(1), unmerged: 3, base: Some("origin/master".into()) });
    assert!(s.risky() && s.force_branch());
    let last = fake.calls().pop().unwrap();
    assert_eq!(last.cwd.as_deref(), Some("/w"));
}

#[tokio::test]
async fn a_failed_count_means_no_base_not_zero() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond("git status", ok(""));
    fake.respond("git symbolic-ref", ok(""));
    fake.respond("git rev-parse --verify 'origin/master'", ok("x"));
    fake.respond("git rev-list", ok("garbage"));
    let s = git(&fake).safety_of(&wt("/w", "b", false, false), "/r").await;
    assert_eq!(s, WorktreeSafety { dirty: Some(0), unmerged: 0, base: None });
    assert!(!s.risky() && !s.force_branch());
}

#[test]
fn unknown_dirty_is_risky_but_only_a_counted_base_forces_the_branch() {
    let s = WorktreeSafety { dirty: None, unmerged: 0, base: None };
    assert!(s.risky());
    assert!(!s.force_branch());
}

#[tokio::test]
async fn the_main_checkout_is_never_removed() {
    let fake = Arc::new(FakeRunner::new());
    let out = git(&fake).remove_worktree("/r", &wt("/r", "master", true, false), true, None, true).await;
    assert!(!out.ok);
    assert_eq!(out.message, "Essa é a checkout principal — ela não se remove");
    assert!(fake.calls().is_empty());
}

#[tokio::test]
async fn remove_worktree_forces_and_deletes_the_branch_with_capital_d() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond("git", ok(""));
    let w = wt("/r/.claude/worktrees/TASK-1", "feature/TASK#1", false, false);
    let out = git(&fake).remove_worktree("/r", &w, true, None, true).await;
    assert!(out.ok);
    assert_eq!(out.message, "Worktree removida: TASK-1 · branch feature/TASK#1 apagada");
    assert_eq!(
        fake.commands(),
        [
            "git worktree remove --force '/r/.claude/worktrees/TASK-1'",
            "git branch -D 'feature/TASK#1'",
        ]
    );
}

#[tokio::test]
async fn force_branch_overrides_force_and_a_kept_branch_says_why() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond("git worktree", ok(""));
    fake.respond("git branch", fail("error: not fully merged"));
    let w = wt("/r/wt/x", "b", false, false);
    let out = git(&fake).remove_worktree("/r", &w, true, Some(false), true).await;
    assert_eq!(out.message, "Worktree removida: x · branch mantida (error: not fully merged)");
    assert_eq!(fake.commands()[1], "git branch -d 'b'");
}

#[tokio::test]
async fn a_prunable_worktree_is_removed_by_prune_and_detached_keeps_no_branch() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond("git", ok(""));
    let w = wt("/gone", "(detached)", false, true);
    let out = git(&fake).remove_worktree("/r", &w, false, None, true).await;
    assert_eq!(out.message, "Registros fantasmas limpos");
    assert_eq!(fake.commands(), ["git worktree prune"]);
}

#[tokio::test]
async fn remove_failure_reports_stderr_or_stdout() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond("git worktree remove", output(1, "", "fatal: contains modified files"));
    let out = git(&fake).remove_worktree("/r", &wt("/w", "b", false, false), false, None, true).await;
    assert!(!out.ok);
    assert_eq!(out.message, "fatal: contains modified files");
    assert_eq!(fake.commands(), ["git worktree remove '/w'"]);
}

#[tokio::test]
async fn prune_counts_lines_from_both_streams() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond_once("git worktree prune -v", output(0, "", "Removing worktrees/a: x\nRemoving worktrees/b: y"));
    fake.respond_once("git worktree prune -v", ok(""));
    fake.respond_once("git worktree prune -v", fail("boom"));
    let g = git(&fake);
    assert_eq!(g.prune("/r").await.message, "2 registro(s) limpo(s)");
    assert_eq!(g.prune("/r").await.message, "Nenhuma worktree fantasma pra limpar");
    let failed = g.prune("/r").await;
    assert!(!failed.ok);
    assert_eq!(failed.message, "boom");
}
