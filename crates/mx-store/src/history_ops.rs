use std::future::Future;
use std::path::PathBuf;

use mx_claude::sh_quote;
use mx_core::model::{AgentInfo, Folder, Tab, TabKind};
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
        self.folders
            .iter()
            .filter_map(|f| {
                let reach = std::iter::once(f.root.as_str())
                    .chain(self.worktrees_of(&f.root).iter().map(|w| w.path.as_str()))
                    .filter(|p| inside(p))
                    .map(str::len)
                    .max()?;
                Some((reach, f))
            })
            .rev()
            .max_by_key(|(reach, _)| *reach)
            .map(|(_, f)| f)
    }

    fn tab_holding(&self, sid: &str) -> Option<&Tab> {
        self.tabs.iter().find(|t| {
            (t.kind == TabKind::Claude || !t.exited)
                && (t.resume_id() == Some(sid) || t.hooks.session_id.as_deref() == Some(sid))
        })
    }

    fn elsewhere(&self, sid: &str) -> Option<&AgentInfo> {
        let ours: Vec<i64> = self.terms.values().filter_map(|t| t.pid().map(i64::from)).collect();
        self.agents
            .iter()
            .find(|a| a.session_id.as_deref() == Some(sid) && a.pid.is_none_or(|p| !ours.contains(&p)))
    }

    pub fn standing_of(&self, chat: &ChatEntry) -> ChatStanding {
        let sid = chat.session_id.as_str();
        if self.tab_holding(sid).is_some() {
            return ChatStanding::OnScreen;
        }
        if let Some(agent) = self.elsewhere(sid) {
            return if agent.kind == "background" && agent.id.is_some() {
                ChatStanding::Background
            } else {
                ChatStanding::Live
            };
        }
        if chat.missing || chat.cwd.is_empty() || !is_dir(&chat.cwd) {
            return ChatStanding::Gone;
        }
        ChatStanding::Fresh
    }

    fn attach(&mut self, chat: &ChatEntry, agent: AgentInfo, folder: Option<&str>, feature: Option<&str>) -> Option<String> {
        let job = agent.id.as_deref()?;
        let folder = self.chat_folder(chat, folder);
        let cwd = if is_dir(&chat.cwd) { chat.cwd.clone() } else { folder.root.clone() };
        let command = format!("claude attach {}", sh_quote(job));
        let id = self.open_shell(&folder, Some(&cwd), Some(&command), feature, None);
        if let Some(t) = self.tab_mut(&id) {
            t.custom_label = Some(chat.label());
            t.agent_name = agent.name.clone();
            t.session_id = Some(chat.session_id.clone());
        }
        self.mark_tab(&id);
        Some(id)
    }

    fn chat_folder(&self, chat: &ChatEntry, folder: Option<&str>) -> Folder {
        folder
            .and_then(|r| self.folder_or_loose(r))
            .or_else(|| self.folder_at(&chat.cwd).cloned())
            .unwrap_or_else(|| self.loose.clone())
    }

    pub fn resume_chat(&mut self, chat: &ChatEntry, folder: Option<&str>, feature: Option<&str>) -> Option<String> {
        match self.standing_of(chat) {
            ChatStanding::OnScreen => {
                let id = self.tab_holding(&chat.session_id)?.id.clone();
                self.select(&id);
                Some(id)
            }
            ChatStanding::Background => {
                let agent = self.elsewhere(&chat.session_id)?.clone();
                self.attach(chat, agent, folder, feature)
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
                let folder = self.chat_folder(chat, folder);
                let mut spec = ClaudeSpec::new(&folder, &chat.cwd);
                spec.label = Some(chat.label());
                spec.resume_id = Some(chat.session_id.clone());
                spec.feature = feature.map(str::to_owned);
                Some(self.open_claude(spec))
            }
        }
    }
}
