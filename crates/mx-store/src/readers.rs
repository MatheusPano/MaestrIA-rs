use std::path::Path;

use mx_claude::docs::{LinkAction, follow_link};
use mx_core::model::{Doc, DocSource, Folder, Setup, is_markdown_path};

use crate::changes::Areas;
use crate::msg::Done;
use crate::store::Store;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocPlace {
    pub from: Option<String>,
    pub folder: Option<String>,
    pub cwd: Option<String>,
    pub feature: Option<String>,
}

impl DocPlace {
    pub fn from_tab(id: &str) -> DocPlace {
        DocPlace { from: Some(id.to_owned()), ..DocPlace::default() }
    }
}

impl Store {
    pub fn reading(&self) -> Option<&Doc> {
        self.open_panes().into_iter().find(|t| t.is_reader())?.doc.as_ref()
    }

    pub fn readable(path: &str) -> bool {
        is_markdown_path(path)
    }

    fn shown(&mut self, id: &str) {
        self.save();
        self.mark_tab(id);
        self.mark(Areas::PANES);
    }

    pub fn show_doc(&mut self, doc: Doc, place: DocPlace) -> String {
        let source = place.from.clone().or_else(|| self.focused_tab_id());
        let reuse = self
            .open_panes()
            .into_iter()
            .find(|t| t.is_reader())
            .or_else(|| self.tabs.iter().find(|t| t.is_reader()))
            .map(|t| t.id.clone());
        if let Some(id) = reuse {
            if let Some(t) = self.tab_mut(&id) {
                t.doc = Some(doc);
                t.custom_label = None;
            }
            if !self.is_open(&id) {
                self.place_beside(&id, source.as_deref());
            }
            self.focused = Some(id.clone());
            self.shown(&id);
            return id;
        }
        let from = source.as_deref().and_then(|s| self.tab(s));
        let (folder, cwd, feature): (Folder, Option<String>, Option<String>) = match &place.folder {
            Some(root) => (
                self.folder_or_loose(root).unwrap_or_else(|| self.focused_folder().clone()),
                place.cwd.clone(),
                place.feature.clone(),
            ),
            None => (
                from.map_or_else(|| self.focused_folder().clone(), |t| self.folder_of(t).clone()),
                from.map(|t| t.cwd.clone()),
                from.and_then(|t| t.feature_or_hotfix_id.clone()),
            ),
        };
        let id = self.new_reader(doc, &folder, cwd.as_deref(), feature.as_deref());
        self.place_beside(&id, source.as_deref());
        self.shown(&id);
        id
    }

    pub fn show_plan(&mut self, tab: &str, note: Option<usize>) -> Option<String> {
        let t = self.tab(tab)?;
        let title = self.title(t);
        let plans = &t.hooks.plans;
        let Some(which) = note.filter(|i| *i < plans.len()).or_else(|| plans.len().checked_sub(1)) else {
            self.show_banner(&format!("{title} não apresentou nenhum plano ainda"));
            return None;
        };
        let plan = plans[which].clone();
        let prefix = if which + 1 < plans.len() {
            format!("Plano {}/{}", which + 1, plans.len())
        } else {
            "Plano".to_owned()
        };
        let doc = Doc {
            source: DocSource::Plan,
            title: format!("{prefix} de {title}"),
            text: plan.text,
            path: None,
            origin: Some(title),
            at: plan.at,
        };
        Some(self.show_doc(doc, DocPlace::from_tab(tab)))
    }

    pub fn show_message(&mut self, tab: &str) -> Option<String> {
        let t = self.tab(tab)?;
        let title = self.title(t);
        let Some(text) = t.hooks.last_message_full.clone().filter(|m| !m.trim().is_empty()) else {
            self.show_banner(&format!("{title} ainda não disse nada ao terminar um turno"));
            return None;
        };
        let doc = Doc {
            source: DocSource::Message,
            title: format!("Recado de {title}"),
            text,
            path: None,
            origin: Some(title),
            at: self.now(),
        };
        Some(self.show_doc(doc, DocPlace::from_tab(tab)))
    }

    pub fn show_file(&mut self, path: &str, place: DocPlace) -> Option<String> {
        if !Path::new(path).exists() {
            self.show_banner_sticky(&format!("Esse arquivo não está mais lá: {path}"));
            return None;
        }
        let origin = place.from.as_deref().map(|f| self.title_of(f)).filter(|t| !t.is_empty());
        let doc = Doc::file(path, origin, self.now());
        Some(self.show_doc(doc, place))
    }

    pub fn follow_link(&mut self, href: &str, from: Option<&str>, base: Option<&str>) {
        let base = base
            .map(str::to_owned)
            .or_else(|| from.and_then(|f| self.tab(f)).map(|t| t.cwd.clone()));
        match follow_link(href, base.as_deref(), self.opts.home.as_deref()) {
            LinkAction::Nothing => {}
            LinkAction::OpenExternal(url) => self.p.notifier.open_link(&url),
            LinkAction::Missing(target) => self.show_banner_sticky(&format!(
                "Esse link aponta pra um arquivo que não existe: {target}"
            )),
            LinkAction::Read(target) => {
                let place = DocPlace { from: from.map(str::to_owned), ..DocPlace::default() };
                self.show_file(&target, place);
            }
            LinkAction::QuickLook(target) => self.p.notifier.quick_look(&target),
        }
    }

    pub fn open_markdown(&mut self, place: DocPlace) {
        let start = place
            .cwd
            .clone()
            .or_else(|| place.folder.clone())
            .or_else(|| self.focused_tab().map(|t| t.cwd.clone()))
            .unwrap_or_else(|| self.focused_folder().root.clone());
        let choose = self.p.notifier.choose_markdown(Some(&start));
        self.spawn(async move {
            let path = choose.await;
            Done::MarkdownChosen { path, place }
        });
    }

    pub fn show_setup(&mut self, folder: &str, cwd: Option<&str>, feature: Option<&str>) -> Option<String> {
        let folder = self.folder_or_loose(folder)?;
        let root = cwd.unwrap_or(&folder.root).to_owned();
        let focused = self.focused_tab_id();
        let existing = self
            .tabs
            .iter()
            .find(|t| t.setup.as_ref().is_some_and(|s| s.root == root))
            .map(|t| t.id.clone());
        let id = match existing {
            Some(id) => {
                if !self.is_open(&id) {
                    self.place_beside(&id, focused.as_deref());
                }
                self.focused = Some(id.clone());
                id
            }
            None => {
                let setup = Setup { root: root.clone(), selected: None };
                let id = self.new_setup(setup, &folder, cwd, feature);
                self.place_beside(&id, focused.as_deref());
                id
            }
        };
        self.shown(&id);
        Some(id)
    }

    pub fn set_setup_selected(&mut self, tab: &str, selected: Option<&str>) {
        if let Some(s) = self.tab_mut(tab).and_then(|t| t.setup.as_mut()) {
            s.selected = selected.map(str::to_owned);
            self.touch();
        }
    }
}
