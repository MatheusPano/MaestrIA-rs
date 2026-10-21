use std::future::Future;
use std::path::PathBuf;

use mx_core::model::Folder;
use mx_sys::history::{self, ChatEntry, ChatStanding, ReadOptions};

use crate::restore::is_dir;
use crate::store::Store;
use crate::tabs_open::ClaudeSpec;

impl Store {
    pub fn chat_home(&self) -> PathBuf {
        self.opts.chat_home.clone()
    }

    pub fn all_chats(&self) -> impl Future<Output = Vec<ChatEntry>> + Send + 'static {
        let root = self.chat_home();
        async move { history::read(&root, &ReadOptions::default()).await }
    }

    pub fn chats_in(&self, root: &str) -> impl Future<Output = Vec<ChatEntry>> + Send + 'static {
        let home = self.chat_home();
        let cwds = (root != self.loose.root).then(|| {
            let mut cwds = vec![root.to_owned()];
            cwds.extend(self.worktrees_of(root).iter().map(|w| w.path.clone()));
            cwds
        });
        async move {
            let opts = ReadOptions { cwds, ..ReadOptions::default() };
            history::read(&home, &opts).await
        }
    }

    pub fn folder_at(&self, cwd: &str) -> Option<&Folder> {
        if cwd.is_empty() {
            return None;
        }
        let inside = |root: &str| cwd == root || cwd.starts_with(&format!("{root}/"));
        self.folders.iter().find(|f| inside(&f.root)).or_else(|| {
            self.folders
                .iter()
                .find(|f| self.worktrees_of(&f.root).iter().any(|w| inside(&w.path)))
        })
    }

    pub fn standing_of(&self, chat: &ChatEntry) -> ChatStanding {
        if self.tabs.iter().any(|t| t.resume_id() == Some(chat.session_id.as_str())) {
            return ChatStanding::OnScreen;
        }
        if self
            .agents
            .iter()
            .any(|a| a.session_id.as_deref() == Some(chat.session_id.as_str()))
        {
            return ChatStanding::Live;
        }
        if chat.missing || chat.cwd.is_empty() || !is_dir(&chat.cwd) {
            return ChatStanding::Gone;
        }
        ChatStanding::Fresh
    }

    pub fn resume_chat(&mut self, chat: &ChatEntry, folder: Option<&str>, feature: Option<&str>) -> Option<String> {
        match self.standing_of(chat) {
            ChatStanding::OnScreen => {
                let id = self
                    .tabs
                    .iter()
                    .find(|t| t.resume_id() == Some(chat.session_id.as_str()))?
                    .id
                    .clone();
                self.select(&id);
                Some(id)
            }
            ChatStanding::Live => {
                self.show_banner_sticky(&format!(
                    "\"{}\" ainda está rodando fora do MaestrIA — o Claude só retoma uma conversa depois que ela sai",
                    chat.label()
                ));
                None
            }
            ChatStanding::Gone => {
                let text = if chat.cwd.is_empty() {
                    format!("Não sei em que pasta \"{}\" rodou", chat.label())
                } else {
                    format!("A pasta dessa conversa não existe mais: {}", chat.cwd)
                };
                self.show_banner_sticky(&text);
                None
            }
            ChatStanding::Fresh => {
                let folder = folder
                    .and_then(|r| self.folder_or_loose(r))
                    .or_else(|| self.folder_at(&chat.cwd).cloned())
                    .unwrap_or_else(|| self.loose.clone());
                let mut spec = ClaudeSpec::new(&folder, &chat.cwd);
                spec.label = Some(chat.label());
                spec.resume_id = Some(chat.session_id.clone());
                spec.feature = feature.map(str::to_owned);
                Some(self.open_claude(spec))
            }
        }
    }
}
