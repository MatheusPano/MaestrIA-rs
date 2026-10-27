use std::path::PathBuf;

use chrono::{DateTime, Local, NaiveDate};
use mx_core::model::{Doc, DocSource, Folder, Tab};
use mx_core::text::iso8601;
use mx_plugins::host_json::{ChatJson, FeatureJson, FolderJson, SessionActivity, SessionJson, WorkspaceJson};
use mx_plugins::rpc::{CreatedSession, OpenShell, RpcError, SuggestionCreated};
use mx_sys::history::{self, ReadOptions};

use super::Reply;
use crate::readers::DocPlace;
use crate::store::Store;

pub struct ChatLister {
    home: PathBuf,
    roots: Vec<String>,
}

impl ChatLister {
    pub async fn list(self, day: NaiveDate) -> Vec<ChatJson> {
        let opts = ReadOptions { cwds: None, on: Some(day), limit: usize::MAX };
        let inside = |cwd: &str, root: &str| cwd == root || cwd.starts_with(&format!("{root}/"));
        history::read(&self.home, &opts)
            .await
            .into_iter()
            .map(|c| {
                let folder = self.roots.iter().find(|r| inside(&c.cwd, r)).cloned().unwrap_or_default();
                let at: DateTime<Local> = c.at.into();
                ChatJson {
                    session_id: c.session_id.clone(),
                    title: c.title.clone(),
                    cwd: c.cwd.clone(),
                    folder,
                    at: iso8601(&at),
                    size: c.bytes as i64,
                }
            })
            .collect()
    }
}

impl Store {
    pub(crate) fn session_json(&self, t: &Tab) -> SessionJson {
        let feature = self.feature_of(t).map(|f| (f.name.clone(), f.kind));
        SessionJson {
            id: t.id.clone(),
            kind: t.kind,
            title: self.title(t),
            cwd: t.cwd.clone(),
            folder: t.folder_root.clone(),
            branch: t.branch.clone(),
            status: t.status(),
            done: t.done,
            session_id: t.resume_id().map(str::to_owned),
            hibernated: t.hibernated,
            exited: t.exited,
            on_screen: self.is_open(&t.id),
            focused: self.focused.as_deref() == Some(t.id.as_str()),
            started_at: iso8601(&t.started_at),
            feature_or_hotfix: feature,
            workspaces: self.workspaces_of(&t.folder_root).iter().map(|w| w.name.clone()).collect(),
            launcher: t.launcher_id.as_deref().and_then(|l| self.launcher_by_id(l)).map(|l| l.name.clone()),
            context: t.context,
            owner: t.owner.clone(),
            owner_tag: t.owner_tag.clone(),
            embedded: t.embedded,
            activity: SessionActivity {
                prompts: t.hooks.prompts as i64,
                tools: t.hooks.tools as i64,
                touched: t.hooks.touched.clone(),
                last_prompt: t.hooks.last_prompt.clone(),
                last_message: t.hooks.last_message.clone(),
            },
        }
    }

    pub(crate) fn session_list(&self) -> Vec<SessionJson> {
        let modal = self.plugin.modal.as_ref().map(|(t, _)| t);
        self.tabs.iter().chain(modal).map(|t| self.session_json(t)).collect()
    }

    pub(crate) fn plugin_focus(&mut self, tab: Option<String>) -> Result<(), RpcError> {
        let id = match tab {
            Some(id) if self.has_tab(&id) => id,
            Some(id) => return Err(RpcError::no_tab(&id)),
            None => self.focused_tab_id().ok_or_else(RpcError::no_focused_tab)?,
        };
        self.select(&id);
        Ok(())
    }

    pub(crate) fn folder_list(&self) -> Vec<FolderJson> {
        self.folders
            .iter()
            .map(|f| FolderJson {
                name: f.name.clone(),
                root: f.root.clone(),
                is_repo: f.is_repo,
                branch: f.branch.clone(),
                worktrees: self.worktrees_of(&f.root).to_vec(),
                workspaces: self.workspaces_of(&f.root).iter().map(|w| w.name.clone()).collect(),
            })
            .collect()
    }

    pub(crate) fn feature_list(&self) -> Vec<FeatureJson> {
        self.features
            .iter()
            .map(|f| FeatureJson {
                id: f.id.clone(),
                name: f.name.clone(),
                kind: f.kind,
                folder: f.folder_root.clone(),
                brief: f.brief.clone(),
            })
            .collect()
    }

    pub(crate) fn workspace_list(&self) -> Vec<WorkspaceJson> {
        self.workspaces
            .iter()
            .map(|w| WorkspaceJson {
                id: w.id.clone(),
                name: w.name.clone(),
                folders: w.folder_roots.clone(),
                code_workspace_path: w.code_workspace_path.clone(),
            })
            .collect()
    }

    pub(crate) fn chat_lister(&self) -> ChatLister {
        ChatLister { home: self.chat_home(), roots: self.folders.iter().map(|f| f.root.clone()).collect() }
    }

    pub(crate) fn plugin_editor(&mut self, path: &str, line: Option<i64>) {
        let Some(line) = line.and_then(|l| u32::try_from(l).ok()) else {
            self.open_in_editor(path);
            return;
        };
        let (runner, platform, path) = (self.p.runner.clone(), self.opts.platform, path.to_owned());
        self.spawn(async move {
            let ok = mx_sys::editor::open_on(runner.as_ref(), platform, &path, Some(line)).await;
            crate::msg::Done::EditorOpened { path, ok }
        });
    }

    pub(crate) fn plugin_place(&self, cwd: Option<String>) -> (Folder, String) {
        match cwd {
            Some(cwd) => {
                let folder = self.folder_at(&cwd).cloned().unwrap_or_else(|| self.loose.clone());
                (folder, cwd)
            }
            None => match self.focused_tab() {
                Some(t) => (self.folder_of(t).clone(), t.cwd.clone()),
                None => {
                    let f = self.focused_folder().clone();
                    let root = f.root.clone();
                    (f, root)
                }
            },
        }
    }

    pub(crate) fn plugin_open_shell(&mut self, plugin: &str, req: OpenShell) -> String {
        let (folder, cwd) = self.plugin_place(req.cwd);
        let id = self.open_shell_with(&folder, Some(&cwd), req.command.as_deref(), None, None, !req.embedded);
        if let Some(t) = self.tab_mut(&id) {
            t.custom_label = req.label;
            t.embedded = req.embedded;
            if req.owned {
                t.owner = Some(plugin.to_owned());
                t.owner_tag = req.tag;
            }
        }
        self.mark_tab(&id);
        id
    }

    pub(crate) fn plugin_suggestion_created(&mut self, plugin: &str, req: SuggestionCreated, reply: Reply<Option<CreatedSession>>) {
        let intro = Some(req.intro.as_str()).filter(|i| !i.is_empty());
        self.suggestion_tracked(
            plugin,
            &req.suggestion_id,
            &req.reference,
            intro,
            req.branch.as_deref(),
            req.dir.as_deref(),
            Some(reply),
        );
    }

    pub(crate) fn plugin_markdown(&mut self, plugin_name: &str, title: String, text: String) -> String {
        let doc = Doc {
            source: DocSource::Plugin,
            title,
            text,
            path: None,
            origin: Some(plugin_name.to_owned()),
            at: self.now(),
        };
        self.show_doc(doc, DocPlace::default())
    }
}
