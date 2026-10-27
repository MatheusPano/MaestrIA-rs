use std::future::Future;
use std::sync::Arc;

use mx_claude::ServerEvent;
use mx_claude::mcp::{NewSuggestion, SuggestionQueued, Withdrawal};
use mx_core::model::AgentInfo;
use mx_sys::code_workspace::CodeWorkspace;
use mx_sys::git::GitOutcome;
use tokio::sync::{mpsc, oneshot};

use crate::git_ops::{GitSnapshot, NewTask};
use crate::readers::DocPlace;
use crate::store::Store;

pub enum StoreMsg {
    Server(ServerEvent),
    TermExited {
        tab_id: String,
        code: Option<i32>,
    },
    Suggest(NewSuggestion, oneshot::Sender<Option<SuggestionQueued>>),
    Withdraw {
        tab_id: String,
        task_id: String,
        reply: oneshot::Sender<Withdrawal>,
    },
    Task(TaskDone),
    Run(Box<dyn FnOnce(&mut Store) + Send>),
}

pub struct TaskDone(pub(crate) Done);

pub(crate) enum Done {
    Agents(Option<Vec<AgentInfo>>),
    Git(GitSnapshot),
    Branch { tab_id: String, branch: String },
    FolderFound { path: String, root: String },
    WorkspaceRead { raw: String, found: Option<(CodeWorkspace, Vec<String>)> },
    Worktree { task: NewTask, outcome: GitOutcome },
    GitFinished { message: String, ok: bool },
    EditorOpened { path: String, ok: bool },
    Killed { tab_id: String },
    MarkdownChosen { path: Option<String>, place: DocPlace },
    Banner { text: String, sticky: bool },
    OfferFailed { plugin: String, task: String, message: String },
    PluginsChanged,
}

#[derive(Clone)]
pub struct StoreSender {
    tx: mpsc::UnboundedSender<StoreMsg>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl StoreSender {
    pub fn send(&self, msg: StoreMsg) {
        if self.tx.send(msg).is_ok() {
            (self.wake)();
        }
    }
}

pub(crate) struct Background {
    pub tx: mpsc::UnboundedSender<StoreMsg>,
    pub rx: mpsc::UnboundedReceiver<StoreMsg>,
    pub wake: Arc<dyn Fn() + Send + Sync>,
    pub pending: usize,
    pub git_busy: bool,
    pub git_again: bool,
    pub agents_busy: bool,
}

impl Background {
    pub fn new() -> Background {
        let (tx, rx) = mpsc::unbounded_channel();
        Background {
            tx,
            rx,
            wake: Arc::new(|| {}),
            pending: 0,
            git_busy: false,
            git_again: false,
            agents_busy: false,
        }
    }
}

impl Store {
    pub fn sender(&self) -> StoreSender {
        StoreSender {
            tx: self.bg.tx.clone(),
            wake: self.bg.wake.clone(),
        }
    }

    pub fn set_waker(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        self.bg.wake = wake;
    }

    pub fn pending_tasks(&self) -> usize {
        self.bg.pending
    }

    pub(crate) fn spawn<F>(&mut self, work: F)
    where
        F: Future<Output = Done> + Send + 'static,
    {
        self.bg.pending += 1;
        let tx = self.bg.tx.clone();
        let wake = self.bg.wake.clone();
        self.p.rt.spawn(async move {
            let done = work.await;
            if tx.send(StoreMsg::Task(TaskDone(done))).is_ok() {
                wake();
            }
        });
    }

    pub fn pump(&mut self) -> usize {
        let mut n = 0;
        while let Ok(msg) = self.bg.rx.try_recv() {
            self.apply(msg);
            n += 1;
        }
        n
    }

    pub async fn settle(&mut self) {
        loop {
            self.pump();
            if self.bg.pending == 0 {
                return;
            }
            match self.bg.rx.recv().await {
                Some(msg) => self.apply(msg),
                None => return,
            }
        }
    }

    pub fn apply(&mut self, msg: StoreMsg) {
        match msg {
            StoreMsg::Server(ServerEvent::Hook {
                tab_id,
                name,
                payload,
            }) => self.apply_hook(&tab_id, &name, payload),
            StoreMsg::Server(ServerEvent::Status { tab_id, payload }) => {
                self.apply_status(&tab_id, &serde_json::Value::Object(payload))
            }
            StoreMsg::TermExited { tab_id, code } => self.on_term_exit(&tab_id, code),
            StoreMsg::Suggest(s, reply) => {
                let _ = reply.send(self.suggest(s));
            }
            StoreMsg::Withdraw {
                tab_id,
                task_id,
                reply,
            } => {
                let _ = reply.send(self.withdraw(&tab_id, &task_id));
            }
            StoreMsg::Run(job) => {
                if !self.gone {
                    job(self);
                }
            }
            StoreMsg::Task(TaskDone(done)) => {
                self.bg.pending = self.bg.pending.saturating_sub(1);
                self.finish(done);
            }
        }
    }

    fn finish(&mut self, done: Done) {
        if self.gone {
            return;
        }
        match done {
            Done::Agents(list) => self.agents_polled(list),
            Done::Git(snapshot) => self.apply_git(snapshot),
            Done::Branch { tab_id, branch } => self.apply_branch(&tab_id, branch),
            Done::FolderFound { path, root } => self.folder_found(&path, &root),
            Done::WorkspaceRead { raw, found } => self.workspace_read(&raw, found),
            Done::Worktree { task, outcome } => self.worktree_added(task, outcome),
            Done::GitFinished { message, ok } => self.git_done(message, ok),
            Done::EditorOpened { path, ok } => self.editor_opened(&path, ok),
            Done::Killed { tab_id } => self.woken_after_kill(&tab_id),
            Done::Banner { text, sticky } => self.show_banner_with(&text, sticky),
            Done::OfferFailed { plugin, task, message } => self.offer_failed(&plugin, &task, &message),
            Done::PluginsChanged => self.plugins_changed(),
            Done::MarkdownChosen { path, place } => {
                if let Some(path) = path {
                    self.show_file(&path, place);
                }
            }
        }
    }
}
