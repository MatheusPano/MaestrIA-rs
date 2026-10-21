use std::path::Path;

use mx_core::model::WorktreeInfo;
use mx_sys::code_workspace::looks_like_one;
use mx_sys::git::GitOutcome;

use crate::changes::Areas;
use crate::msg::Done;
use crate::store::Store;
use crate::tabs_open::ClaudeSpec;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GitSnapshot {
    pub roots: Vec<(String, Option<Vec<WorktreeInfo>>)>,
    pub tabs: Vec<(String, Option<usize>, Option<String>)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewTask {
    pub folder_root: String,
    pub task_id: String,
    pub branch_pattern: String,
    pub dir_pattern: String,
    pub base_ref: Option<String>,
    pub setup_command: Option<String>,
    pub feature: Option<String>,
    pub prompt: Option<String>,
    pub select: bool,
    pub reply: Option<u64>,
}

impl Store {
    pub fn refresh_git(&mut self) {
        if self.bg.git_busy {
            self.bg.git_again = true;
            return;
        }
        self.bg.git_busy = true;
        let roots: Vec<String> = self.folders.iter().map(|f| f.root.clone()).collect();
        let tabs: Vec<(String, String, bool)> = self
            .tabs
            .iter()
            .filter(|t| !t.is_passive())
            .map(|t| (t.id.clone(), t.cwd.clone(), t.branch.is_empty()))
            .collect();
        let git = self.git.clone();
        self.spawn(async move {
            let mut snap = GitSnapshot::default();
            for root in roots {
                let list = git.worktrees(&root).await;
                snap.roots.push((root, list));
            }
            for (id, cwd, needs_branch) in tabs {
                let dirty = git.dirty_count(&cwd).await;
                let branch = if needs_branch { Some(git.branch_of(&cwd).await) } else { None };
                snap.tabs.push((id, dirty, branch));
            }
            Done::Git(snap)
        });
    }

    pub(crate) fn apply_git(&mut self, snap: GitSnapshot) {
        self.bg.git_busy = false;
        for (root, list) in snap.roots {
            let Some(list) = list else { continue };
            let Some(f) = self.folders.iter_mut().find(|f| f.root == root) else {
                continue;
            };
            let branch = list
                .iter()
                .find(|w| w.is_main)
                .map(|w| w.branch.clone())
                .unwrap_or_default();
            let repo = !list.is_empty();
            let mut changed = f.is_repo != repo || f.branch != branch;
            f.is_repo = repo;
            f.branch = branch;
            if self.worktrees.get(&root) != Some(&list) {
                self.worktrees.insert(root, list);
                changed = true;
            }
            if changed {
                self.mark(Areas::SIDEBAR);
            }
        }
        for (id, dirty, branch) in snap.tabs {
            let Some(t) = self.tab_mut(&id) else { continue };
            let mut changed = false;
            if let Some(d) = dirty
                && t.dirty != d
            {
                t.dirty = d;
                changed = true;
            }
            if let Some(b) = branch
                && t.branch.is_empty()
                && !b.is_empty()
            {
                t.branch = b;
                changed = true;
            }
            if changed {
                self.mark_tab(&id);
            }
        }
        if std::mem::take(&mut self.bg.git_again) {
            self.refresh_git();
        }
    }

    pub fn new_task(&mut self, task: NewTask) {
        let git = self.git.clone();
        self.spawn(async move {
            let base = match &task.base_ref {
                Some(b) => b.clone(),
                None => git
                    .default_remote_ref(&task.folder_root)
                    .await
                    .unwrap_or_else(|| "origin/master".into()),
            };
            let branch = task.branch_pattern.replace("{id}", &task.task_id);
            let dir = task.dir_pattern.replace("{id}", &task.task_id).replace('#', "-");
            let outcome = git.add_worktree(&task.folder_root, &dir, &branch, &base).await;
            Done::Worktree { task, outcome }
        });
    }

    pub(crate) fn worktree_added(&mut self, task: NewTask, outcome: GitOutcome) {
        self.show_banner_with(&outcome.message, !outcome.ok);
        let folder = self.folder_or_loose(&task.folder_root);
        let Some(folder) = folder.filter(|_| outcome.ok) else {
            self.answer_created(task.reply, None);
            return;
        };
        if let Some(setup) = task.setup_command.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            let id = self.open_shell(&folder, Some(&outcome.path), Some(setup), task.feature.as_deref(), None);
            if let Some(t) = self.tab_mut(&id) {
                t.custom_label = Some(format!("{} setup", task.task_id));
            }
        }
        self.refresh_git();
        let mut spec = ClaudeSpec::new(&folder, &outcome.path);
        spec.label = Some(task.task_id.clone());
        spec.feature = task.feature.clone();
        spec.prompt = task.prompt.clone();
        let id = self.open_claude(spec);
        if task.select {
            self.select(&id);
        }
        self.answer_created(task.reply, Some(&id));
    }

    pub fn remove_worktree(&mut self, root: &str, w: &WorktreeInfo, force: bool, force_branch: Option<bool>, delete_branch: bool) {
        if self.tab_at(&w.path).is_some() {
            self.show_banner_sticky("Feche o painel que está nessa worktree antes de excluí-la");
            return;
        }
        let (git, root, w) = (self.git.clone(), root.to_owned(), w.clone());
        self.spawn(async move {
            let o = git.remove_worktree(&root, &w, force, force_branch, delete_branch).await;
            Done::GitFinished { message: o.message, ok: o.ok }
        });
    }

    pub fn prune_worktrees(&mut self, root: &str) {
        let (git, root) = (self.git.clone(), root.to_owned());
        self.spawn(async move {
            let o = git.prune(&root).await;
            Done::GitFinished { message: o.message, ok: o.ok }
        });
    }

    pub(crate) fn git_done(&mut self, message: String, ok: bool) {
        self.show_banner_with(&message, !ok);
        self.refresh_git();
    }

    pub fn open_in_editor(&mut self, path: &str) {
        if !Path::new(path).exists() {
            let text = if looks_like_one(path) {
                format!("Esse workspace não está mais lá: {path}")
            } else {
                format!("Essa pasta não existe mais: {path}")
            };
            self.show_banner_sticky(&text);
            return;
        }
        let (runner, platform, path) = (self.p.runner.clone(), self.opts.platform, path.to_owned());
        self.spawn(async move {
            let ok = mx_sys::editor::open_on(runner.as_ref(), platform, &path, None).await;
            Done::EditorOpened { path, ok }
        });
    }

    pub(crate) fn editor_opened(&mut self, path: &str, ok: bool) {
        if ok {
            let name = mx_core::text::last_segment(path).to_owned();
            self.show_banner(&format!("Aberto no vscode: {name}"));
        } else {
            self.show_banner_sticky("Não achei o vscode — nem o `code` no PATH, nem o app");
        }
    }
}
