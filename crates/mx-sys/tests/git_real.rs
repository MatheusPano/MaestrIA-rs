use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use mx_sys::LoginShell;
use mx_sys::git::Git;

fn sh_git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(status.status.success(), "{args:?}: {}", String::from_utf8_lossy(&status.stderr));
}

fn repo() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    sh_git(&root, &["init", "-q"]);
    std::fs::write(root.join("a.txt"), "a").unwrap();
    sh_git(&root, &["add", "."]);
    sh_git(&root, &["commit", "-qm", "init"]);
    (dir, root.to_string_lossy().into_owned())
}

fn real() -> Git {
    Git::new(Arc::new(LoginShell))
}

#[tokio::test]
async fn a_plain_dir_is_not_a_repo_and_has_no_worktrees() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().unwrap();
    let g = real();
    assert_eq!(g.worktrees(path).await, Some(vec![]));
    assert_eq!(g.dirty_count(path).await, Some(0));
    assert_eq!(g.branch_of(path).await, "");
    assert_eq!(g.main_root(path).await, None);
}

#[tokio::test]
async fn a_fresh_repo_lists_itself_as_main_with_its_branch() {
    let (_dir, root) = repo();
    let g = real();
    let list = g.worktrees(&root).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].path, root);
    assert_eq!(list[0].branch, "main");
    assert!(list[0].is_main);
    assert_eq!(g.branch_of(&root).await, "main");
    assert_eq!(g.default_remote_ref(&root).await, None);
}

#[tokio::test]
async fn dirty_count_counts_changed_and_untracked_files() {
    let (_dir, root) = repo();
    std::fs::write(Path::new(&root).join("a.txt"), "changed").unwrap();
    std::fs::write(Path::new(&root).join("new.txt"), "n").unwrap();
    assert_eq!(real().dirty_count(&root).await, Some(2));
}

#[tokio::test]
async fn add_list_and_remove_a_worktree_with_its_branch() {
    let (_dir, root) = repo();
    let g = real();
    let added = g.add_worktree(&root, "TASK-1", "feature/TASK#1", "HEAD").await;
    assert!(added.ok, "{}", added.message);
    let wt_path = format!("{root}/.claude/worktrees/TASK-1");
    assert_eq!(added.path, wt_path);
    assert!(Path::new(&wt_path).is_dir());

    let again = g.add_worktree(&root, "TASK-1", "feature/TASK#1", "HEAD").await;
    assert!(!again.ok);
    assert!(again.message.starts_with("Worktree já existe em"));

    let list = g.worktrees(&root).await.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[1].branch, "feature/TASK#1");
    assert_eq!(list[1].short_label(), "TASK#1");
    assert_eq!(g.main_root(&wt_path).await.as_deref(), Some(root.as_str()));

    let removed = g.remove_worktree(&root, &list[1], false, None, true).await;
    assert!(removed.ok, "{}", removed.message);
    assert_eq!(removed.message, "Worktree removida: TASK-1 · branch feature/TASK#1 apagada");
    assert_eq!(g.worktrees(&root).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_worktree_whose_folder_vanished_is_prunable_and_prune_clears_it() {
    let (_dir, root) = repo();
    let g = real();
    assert!(g.add_worktree(&root, "gone", "gone", "HEAD").await.ok);
    std::fs::remove_dir_all(format!("{root}/.claude/worktrees/gone")).unwrap();

    let list = g.worktrees(&root).await.unwrap();
    assert!(list[1].prunable);
    let pruned = g.prune(&root).await;
    assert!(pruned.ok);
    assert_eq!(pruned.message, "1 registro(s) limpo(s)");
    assert_eq!(g.prune(&root).await.message, "Nenhuma worktree fantasma pra limpar");
    assert_eq!(g.worktrees(&root).await.unwrap().len(), 1);
}
