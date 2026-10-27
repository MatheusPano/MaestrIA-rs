use mx_core::model::{Filter, FilterGroup, Tab};

use crate::changes::Areas;
use crate::store::Store;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Search {
    pub query: String,
    pub terms: Vec<String>,
    pub roots: Vec<String>,
    pub features: Vec<String>,
    pub flags: Vec<Filter>,
}

fn flip<T: PartialEq>(list: &mut Vec<T>, item: T) {
    match list.iter().position(|x| *x == item) {
        Some(i) => {
            list.remove(i);
        }
        None => list.push(item),
    }
}

impl Store {
    pub fn search(&self) -> &Search {
        &self.search
    }

    fn searched(&mut self) {
        self.mark(Areas::SEARCH | Areas::SIDEBAR);
    }

    pub fn set_query(&mut self, q: &str) {
        if self.search.query == q {
            return;
        }
        self.search.query = q.to_owned();
        self.search.terms = q
            .to_lowercase()
            .split(' ')
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
            .collect();
        self.searched();
    }

    pub fn toggle_filter(&mut self, f: Filter) {
        flip(&mut self.search.flags, f);
        self.searched();
    }

    pub fn toggle_filter_folder(&mut self, root: &str) {
        flip(&mut self.search.roots, root.to_owned());
        self.searched();
    }

    pub fn toggle_filter_feature(&mut self, id: &str) {
        flip(&mut self.search.features, id.to_owned());
        self.searched();
    }

    pub fn filtering(&self) -> bool {
        let s = &self.search;
        !s.terms.is_empty() || !s.roots.is_empty() || !s.features.is_empty() || !s.flags.is_empty()
    }

    pub fn active_filters(&self) -> usize {
        let s = &self.search;
        s.roots.len() + s.features.len() + s.flags.len()
    }

    pub fn clear_search(&mut self) {
        if !self.filtering() && self.search.query.is_empty() {
            return;
        }
        self.search = Search::default();
        self.searched();
    }

    fn haystack(&self, t: &Tab) -> String {
        let feature = self.feature_of(t).map(|f| f.name.as_str()).unwrap_or("");
        [
            self.title(t).as_str(),
            t.custom_label.as_deref().unwrap_or(""),
            self.folder_of(t).name.as_str(),
            feature,
            &t.branch,
            &t.cwd,
            t.agent_name.as_deref().unwrap_or(""),
            t.kind.name(),
        ]
        .join(" ")
        .to_lowercase()
    }

    pub fn matches(&self, t: &Tab) -> bool {
        let s = &self.search;
        if !s.terms.is_empty() {
            let hay = self.haystack(t);
            if !s.terms.iter().all(|term| hay.contains(term.as_str())) {
                return false;
            }
        }
        if !s.roots.is_empty() || !s.features.is_empty() {
            let in_root = s.roots.contains(&t.folder_root);
            let in_feature = t
                .feature_or_hotfix_id
                .as_ref()
                .is_some_and(|f| s.features.contains(f));
            if !in_root && !in_feature {
                return false;
            }
        }
        [FilterGroup::Kind, FilterGroup::State].iter().all(|g| {
            let picks: Vec<&Filter> = s.flags.iter().filter(|f| f.group() == *g).collect();
            picks.is_empty() || picks.iter().any(|f| f.matches(t))
        })
    }

    pub fn has_hits(&self, root: &str) -> bool {
        self.tabs_of(root).iter().any(|t| self.matches(t))
    }

    pub fn hits(&self) -> Vec<&Tab> {
        self.tabs.iter().filter(|t| self.matches(t)).collect()
    }
}
