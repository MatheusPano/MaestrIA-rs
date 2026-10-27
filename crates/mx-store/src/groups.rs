use mx_core::json::{Obj, is_true, str_of};
use mx_core::layout::{self, PaneNode};
use mx_core::model::group::summarize;
use mx_core::model::{Doc, PaneGroup, Setup, ids};
use mx_core::persist::PaneEntry;
use serde_json::Value;

use crate::changes::Areas;
use crate::store::Store;

impl Store {
    pub fn groups(&self) -> &[PaneGroup] {
        &self.groups
    }

    pub fn group_by_id(&self, id: &str) -> Option<&PaneGroup> {
        self.groups.iter().find(|g| g.id == id)
    }

    pub fn group_of(&self, tab: &str) -> Option<&PaneGroup> {
        self.tab(tab)?.group_id.as_deref().and_then(|g| self.group_by_id(g))
    }

    pub fn active_group(&self) -> Option<&PaneGroup> {
        let open = self.open_panes();
        let first = open.first()?.group_id.as_deref()?;
        if !open.iter().all(|t| t.group_id.as_deref() == Some(first)) {
            return None;
        }
        self.group_by_id(first)
    }

    pub fn showing(&self, group: &str) -> bool {
        self.active_group().is_some_and(|g| g.id == group)
    }

    fn groups_changed(&mut self) {
        self.save();
        self.mark(Areas::GROUPS | Areas::SIDEBAR);
    }

    pub fn save_group(&mut self, name: &str) -> Option<String> {
        let title = name.trim().to_owned();
        let open = self.open_pane_ids();
        if title.is_empty() || open.is_empty() {
            return None;
        }
        let lower = title.to_lowercase();
        let at = self.groups.iter().position(|g| g.name.to_lowercase() == lower);
        let id = match at {
            Some(i) => self.groups[i].id.clone(),
            None => self.fresh_id(ids::group_id, |id| self.group_by_id(id).is_some()),
        };
        let panes: Vec<Obj> = open.iter().filter_map(|t| self.tab(t)).map(|t| t.recipe()).collect();
        let index_of = |t: &str| open.iter().position(|x| x == t).map_or(-1, |i| i as i64);
        let tree = layout::to_json(self.panes.as_ref(), &index_of);
        let group = PaneGroup { id: id.clone(), name: title, panes, tree };
        match at {
            Some(i) => self.groups[i] = group,
            None => self.groups.push(group),
        }
        self.stamp(&id, &open);
        self.groups_changed();
        Some(id)
    }

    pub fn group_panes(&mut self) -> Option<String> {
        let open = self.open_panes();
        if open.len() < 2 {
            return None;
        }
        let recipes: Vec<Obj> = open.iter().map(|t| t.recipe()).collect();
        let base = summarize(&recipes);
        let taken = |n: &str| self.groups.iter().any(|g| g.name.to_lowercase() == n.to_lowercase());
        let mut name = base.clone();
        let mut n = 2;
        while taken(&name) {
            name = format!("{base} ({n})");
            n += 1;
        }
        self.save_group(&name)
    }

    fn stamp(&mut self, group: &str, members: &[String]) {
        for t in &mut self.tabs {
            if members.contains(&t.id) {
                t.group_id = Some(group.to_owned());
            } else if t.group_id.as_deref() == Some(group) {
                t.group_id = None;
            }
        }
    }

    pub fn ungroup(&mut self, group: &str) {
        self.stamp(group, &[]);
        self.groups.retain(|g| g.id != group);
        self.groups_changed();
    }

    pub fn remove_group(&mut self, group: &str) {
        self.groups.retain(|g| g.id != group);
        self.groups_changed();
    }

    pub fn clear_groups(&mut self) -> usize {
        let n = self.groups.len();
        if n == 0 {
            return 0;
        }
        for t in &mut self.tabs {
            t.group_id = None;
        }
        self.groups.clear();
        self.groups_changed();
        n
    }

    pub fn rename_group(&mut self, group: &str, name: &str) {
        let title = name.trim().to_owned();
        let Some(g) = self.groups.iter_mut().find(|g| g.id == group) else {
            return;
        };
        if title.is_empty() || g.name == title {
            return;
        }
        g.name = title;
        self.groups_changed();
    }

    pub fn open_group(&mut self, group: &str) {
        let Some(g) = self.group_by_id(group).cloned() else { return };
        let now = self.now();
        self.restoring = true;
        let mut taken: Vec<String> = Vec::new();
        let mut panels: Vec<Option<String>> = Vec::new();
        for pane in &g.panes {
            let id = self
                .adopt_pane(pane, &taken)
                .or_else(|| self.open_pane(&PaneEntry::parse(pane, now)));
            if let Some(id) = &id {
                taken.push(id.clone());
            }
            panels.push(id);
        }
        self.restoring = false;
        let Some(first) = panels.iter().flatten().next().cloned() else {
            self.show_banner_sticky(&format!("O grupo \"{}\" não tem mais nenhum painel pra abrir", g.name));
            return;
        };
        let at = |i: i64| usize::try_from(i).ok().and_then(|i| panels.get(i)).cloned().flatten();
        self.panes = g
            .tree
            .as_ref()
            .and_then(|t| layout::from_json(t, &at))
            .or_else(|| Some(PaneNode::leaf(&first)));
        self.focused = self.order().into_iter().next();
        for id in self.open_pane_ids() {
            if self.tab(&id).is_some_and(|t| t.hibernated) {
                self.wake(&id);
            }
        }
        let members: Vec<String> = panels.into_iter().flatten().collect();
        self.stamp(&g.id, &members);
        self.groups_changed();
        self.mark(Areas::PANES);
    }

    fn adopt_pane(&mut self, pane: &Obj, taken: &[String]) -> Option<String> {
        let free = |id: &String| !taken.contains(id);
        let kind = str_of(pane, "kind");
        match kind {
            Some("reader") => {
                let id = self.tabs.iter().find(|t| t.is_reader() && free(&t.id))?.id.clone();
                let doc = Doc::from_json(pane.get("doc")?, self.now())?;
                let t = self.tab_mut(&id)?;
                t.doc = Some(doc);
                t.custom_label = None;
                Some(id)
            }
            Some("setup") => {
                let setup = Setup::from_json(pane.get("setup")?)?;
                let id = self
                    .tabs
                    .iter()
                    .find(|t| free(&t.id) && t.setup.as_ref().is_some_and(|s| s.root == setup.root))?
                    .id
                    .clone();
                self.tab_mut(&id)?.setup = Some(setup);
                Some(id)
            }
            _ => {
                let root = if is_true(pane, "loose") {
                    Some(self.loose.root.as_str())
                } else {
                    str_of(pane, "folderRoot")
                };
                let cwd = str_of(pane, "cwd");
                let session = pane.get("sessionId").and_then(Value::as_str);
                self.tabs
                    .iter()
                    .find(|t| {
                        free(&t.id)
                            && Some(t.kind.name()) == kind
                            && Some(t.folder_root.as_str()) == root
                            && Some(t.cwd.as_str()) == cwd
                            && !t.exited
                            && session.is_none_or(|s| t.resume_id() == Some(s))
                    })
                    .map(|t| t.id.clone())
            }
        }
    }
}
