use mx_core::model::{FeatureOrHotfix, FeatureOrHotfixKind, Tab, Tint, ids};

use crate::changes::Areas;
use crate::store::Store;

impl Store {
    pub fn features(&self) -> &[FeatureOrHotfix] {
        &self.features
    }

    pub fn features_of(&self, root: &str) -> Vec<&FeatureOrHotfix> {
        self.features.iter().filter(|f| f.folder_root == root).collect()
    }

    pub fn feature_by_id(&self, id: &str) -> Option<&FeatureOrHotfix> {
        self.features.iter().find(|f| f.id == id)
    }

    pub fn feature_of(&self, tab: &Tab) -> Option<&FeatureOrHotfix> {
        tab.feature_or_hotfix_id.as_deref().and_then(|f| self.feature_by_id(f))
    }

    pub fn focused_feature(&self) -> Option<&FeatureOrHotfix> {
        self.focused_tab().and_then(|t| self.feature_of(t))
    }

    fn edit_feature(&mut self, id: &str, edit: impl FnOnce(&mut FeatureOrHotfix) -> bool) {
        let Some(f) = self.features.iter_mut().find(|f| f.id == id) else {
            return;
        };
        if edit(f) {
            self.save();
            self.mark(Areas::SIDEBAR);
        }
    }

    pub fn add_feature(&mut self, root: &str, name: &str, brief: &str, kind: FeatureOrHotfixKind) -> String {
        let id = self.fresh_id(ids::feature_id, |id| self.feature_by_id(id).is_some());
        let mut f = FeatureOrHotfix::new(id.clone(), root, name, kind);
        f.brief = brief.to_owned();
        self.features.push(f);
        self.save();
        self.mark(Areas::SIDEBAR);
        id
    }

    pub fn edit_feature_text(&mut self, id: &str, name: Option<&str>, brief: Option<&str>) {
        self.edit_feature(id, |f| {
            if let Some(n) = name.map(str::trim).filter(|n| !n.is_empty()) {
                f.name = n.to_owned();
            }
            if let Some(b) = brief {
                f.brief = b.to_owned();
            }
            true
        });
    }

    pub fn set_feature_tint(&mut self, id: &str, tint: Option<Tint>) {
        self.edit_feature(id, |f| {
            f.tint = tint;
            true
        });
    }

    pub fn set_feature_kind(&mut self, id: &str, kind: FeatureOrHotfixKind) {
        self.edit_feature(id, |f| {
            let changed = f.kind != kind;
            f.kind = kind;
            changed
        });
    }

    pub fn toggle_feature_collapsed(&mut self, id: &str) {
        self.edit_feature(id, |f| {
            f.collapsed = !f.collapsed;
            true
        });
    }

    pub fn complete_feature(&mut self, id: &str) -> usize {
        let inside: Vec<String> = self.tabs_in(id).iter().map(|t| t.id.clone()).collect();
        for t in &inside {
            self.close_tab(t);
        }
        self.features.retain(|f| f.id != id);
        self.save();
        self.mark(Areas::SIDEBAR);
        inside.len()
    }

    pub fn remove_feature(&mut self, id: &str) {
        self.features.retain(|f| f.id != id);
        for t in &mut self.tabs {
            if t.feature_or_hotfix_id.as_deref() == Some(id) {
                t.feature_or_hotfix_id = None;
            }
        }
        self.save();
        self.mark(Areas::SIDEBAR);
    }

    pub fn assign(&mut self, tab: &str, feature: Option<&str>) {
        let Some(t) = self.tab(tab) else { return };
        if let Some(f) = feature
            && self.feature_by_id(f).is_none_or(|f| f.folder_root != t.folder_root)
        {
            return;
        }
        if t.feature_or_hotfix_id.as_deref() == feature {
            return;
        }
        if let Some(t) = self.tab_mut(tab) {
            t.feature_or_hotfix_id = feature.map(str::to_owned);
        }
        self.save();
        self.mark_tab(tab);
    }

    pub fn tabs_in(&self, feature: &str) -> Vec<&Tab> {
        self.tabs
            .iter()
            .filter(|t| t.feature_or_hotfix_id.as_deref() == Some(feature))
            .collect()
    }

    pub fn needing_human_in(&self, feature: &str) -> usize {
        self.tabs_in(feature)
            .iter()
            .filter(|t| !t.done && t.status().needs_human())
            .count()
    }
}
