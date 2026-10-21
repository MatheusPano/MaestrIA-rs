use mx_claude::mcp::{self, NewSuggestion, SuggestionQueued, TaskRef, Withdrawal};
use mx_core::model::{NoticeKind, SuggestedTask, SuggestionChoice, SuggestionEnd, TabKind};
use mx_core::model::hook_state::MAX_SUGGESTIONS;

use mx_plugins::rpc::CreatedSession;

use crate::git_ops::NewTask;
use crate::plugins::Reply;
use crate::store::Store;
use crate::tabs_open::ClaudeSpec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tracking {
    pub plugin: String,
    pub tab_id: String,
    pub choice: SuggestionChoice,
}

fn task_ref(t: &SuggestedTask) -> TaskRef {
    TaskRef { id: t.id.clone(), title: t.title.clone() }
}

fn wire(end: SuggestionEnd) -> mcp::SuggestionEnd {
    match end {
        SuggestionEnd::Started => mcp::SuggestionEnd::Started,
        SuggestionEnd::Dismissed => mcp::SuggestionEnd::Dismissed,
    }
}

impl Store {
    pub fn suggest(&mut self, s: NewSuggestion) -> Option<SuggestionQueued> {
        let tab = self.tab(&s.tab_id)?;
        if tab.kind != TabKind::Claude {
            return None;
        }
        let task = SuggestedTask {
            id: mcp::new_task_id(),
            title: s.title,
            tldr: s.tldr,
            prompt: s.prompt,
            at: self.now(),
        };
        let mut evicted = Vec::new();
        let t = self.tab_mut(&s.tab_id)?;
        t.hooks.suggestions.push(task.clone());
        while t.hooks.suggestions.len() > MAX_SUGGESTIONS {
            evicted.push(t.hooks.suggestions.remove(0));
        }
        let pending: Vec<TaskRef> = t.hooks.suggestions.iter().map(task_ref).collect();
        for gone in &evicted {
            self.settled.insert(gone.id.clone(), SuggestionEnd::Dismissed);
        }
        let detail = (!task.title.is_empty()).then(|| task.title.clone());
        self.push_notice(&s.tab_id, NoticeKind::Suggested, Some(task.id.clone()), detail);
        self.save();
        self.mark_tab(&s.tab_id);
        Some(SuggestionQueued {
            task: task_ref(&task),
            pending,
            evicted: evicted.iter().map(task_ref).collect(),
        })
    }

    pub fn withdraw(&mut self, tab: &str, task: &str) -> Withdrawal {
        if self.pending_suggestion(tab, task).is_none() {
            return Withdrawal {
                withdrawn: false,
                already: self.settled.get(task).copied().map(wire),
            };
        }
        self.settle_suggestion(tab, task, SuggestionEnd::Dismissed);
        Withdrawal { withdrawn: true, already: None }
    }

    pub fn pending_suggestion(&self, tab: &str, task: &str) -> Option<&SuggestedTask> {
        self.tab(tab)?.hooks.suggestions.iter().find(|s| s.id == task)
    }

    pub fn settled_as(&self, task: &str) -> Option<SuggestionEnd> {
        self.settled.get(task).copied()
    }

    pub fn settle_suggestion(&mut self, tab: &str, task: &str, end: SuggestionEnd) {
        if let Some(t) = self.tab_mut(tab) {
            t.hooks.suggestions.retain(|s| s.id != task);
        }
        self.settled.insert(task.to_owned(), end);
        self.save();
        self.mark_tab(tab);
    }

    pub fn suggestion_of(&self, notice: u64) -> Option<&SuggestedTask> {
        let n = &self.notice(notice)?.notice;
        self.pending_suggestion(&n.tab_id, n.suggestion_id.as_deref()?)
    }

    pub fn dismiss_suggestion(&mut self, tab: &str, task: &str) {
        self.settle_suggestion(tab, task, SuggestionEnd::Dismissed);
    }

    pub fn run_suggestion_here(&mut self, tab: &str, task: &str, prompt: Option<&str>) -> bool {
        let Some(t) = self.tab(tab) else { return false };
        if t.exited || t.hibernated {
            let title = self.title(t);
            self.show_banner_sticky(&format!("{title} não está rodando — retome a conversa antes"));
            return false;
        }
        let Some(text) = prompt
            .map(str::to_owned)
            .or_else(|| self.pending_suggestion(tab, task).map(|s| s.prompt.clone()))
        else {
            return false;
        };
        self.settle_suggestion(tab, task, SuggestionEnd::Started);
        self.submit(tab, &text);
        true
    }

    pub fn start_suggestion_here(&mut self, tab: &str, task: &str, prompt: Option<&str>, label: Option<&str>) -> Option<String> {
        let t = self.tab(tab)?;
        let s = self.pending_suggestion(tab, task)?.clone();
        let mut spec = ClaudeSpec::new(self.folder_of(t), &t.cwd);
        spec.label = label
            .map(str::to_owned)
            .or_else(|| (!s.title.is_empty()).then(|| s.title.clone()));
        spec.feature = t.feature_or_hotfix_id.clone();
        spec.prompt = Some(prompt.map_or(s.prompt, str::to_owned));
        self.settle_suggestion(tab, task, SuggestionEnd::Started);
        let id = self.open_claude(spec);
        self.select(&id);
        Some(id)
    }

    pub fn track_suggestion(&mut self, task: &str, plugin: &str, tab: &str, choice: SuggestionChoice) {
        self.tracking.insert(
            task.to_owned(),
            Tracking { plugin: plugin.to_owned(), tab_id: tab.to_owned(), choice },
        );
    }

    pub fn untrack_suggestion(&mut self, task: &str) {
        self.tracking.remove(task);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn suggestion_tracked(
        &mut self,
        plugin: &str,
        task: &str,
        reference: &str,
        intro: Option<&str>,
        branch: Option<&str>,
        dir: Option<&str>,
        reply: Option<Reply<Option<CreatedSession>>>,
    ) {
        let answer = |reply: Option<Reply<Option<CreatedSession>>>, v: Option<CreatedSession>| {
            if let Some(r) = reply {
                let _ = r.send(Ok(v));
            }
        };
        if self.tracking.get(task).is_none_or(|t| t.plugin != plugin) {
            return answer(reply, None);
        }
        let Some(tr) = self.tracking.remove(task) else { return answer(reply, None) };
        let Some(s) = self.pending_suggestion(&tr.tab_id, task).cloned() else {
            self.show_banner(&mx_plugins::suggestions::gone_banner(reference));
            return answer(reply, None);
        };
        let branch = mx_plugins::suggestions::worktree_branch(branch, reference);
        let on_branch = (tr.choice == SuggestionChoice::Worktree).then_some(branch.as_str());
        let prompt = mx_plugins::suggestions::suggestion_prompt(intro.unwrap_or(""), &s.prompt, on_branch);
        let created = |s: &Store, id: &str| s.tab(id).map(|t| CreatedSession { tab_id: id.to_owned(), cwd: t.cwd.clone() });
        match tr.choice {
            SuggestionChoice::Here => {
                let ok = self.run_suggestion_here(&tr.tab_id, task, Some(&prompt));
                let v = if ok { created(self, &tr.tab_id) } else { None };
                answer(reply, v);
            }
            SuggestionChoice::Local => {
                let id = self.start_suggestion_here(&tr.tab_id, task, Some(&prompt), Some(reference));
                let v = id.and_then(|id| created(self, &id));
                answer(reply, v);
            }
            SuggestionChoice::Worktree => {
                let Some(t) = self.tab(&tr.tab_id) else { return answer(reply, None) };
                let (root, feature) = (t.folder_root.clone(), t.feature_or_hotfix_id.clone());
                let reply_id = reply.map(|r| self.park_reply(r));
                let job = NewTask {
                    folder_root: root,
                    task_id: reference.to_owned(),
                    branch_pattern: branch.clone(),
                    dir_pattern: mx_plugins::suggestions::worktree_dir(dir, reference),
                    feature,
                    prompt: Some(prompt),
                    select: true,
                    reply: reply_id,
                    ..NewTask::default()
                };
                self.new_task(job);
                self.settle_suggestion(&tr.tab_id, task, SuggestionEnd::Started);
            }
            SuggestionChoice::Dismiss => {
                self.dismiss_suggestion(&tr.tab_id, task);
                answer(reply, None);
            }
        }
    }

    fn park_reply(&mut self, reply: Reply<Option<CreatedSession>>) -> u64 {
        let id = self.plugin.next_reply;
        self.plugin.next_reply += 1;
        self.plugin.created.insert(id, reply);
        id
    }

    pub(crate) fn answer_created(&mut self, reply: Option<u64>, tab: Option<&str>) {
        let Some(r) = reply.and_then(|id| self.plugin.created.remove(&id)) else { return };
        let v = tab.and_then(|id| self.tab(id)).map(|t| CreatedSession { tab_id: t.id.clone(), cwd: t.cwd.clone() });
        let _ = r.send(Ok(v));
    }
}
