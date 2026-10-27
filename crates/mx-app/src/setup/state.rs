use mx_claude::setup::{self, SetupSection, SetupSlot};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocState {
    pub disk: String,
    pub loaded: String,
    pub outside: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Poll {
    Same,
    Outside,
    Replace(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateTone {
    Faint,
    Warn,
}

impl DocState {
    pub fn load(slot: Option<&SetupSlot>, root: &str, folder: &str) -> Self {
        match slot {
            None => Self::default(),
            Some(s) if s.exists => {
                let disk = setup::read(root, &s.rel);
                Self { loaded: disk.clone(), disk, outside: false }
            }
            Some(s) => Self {
                disk: String::new(),
                loaded: setup::template(&s.rel, folder, ""),
                outside: false,
            },
        }
    }

    pub fn dirty(&self, text: &str) -> bool {
        text != self.disk
    }

    pub fn edited(&self, text: &str) -> bool {
        text != self.loaded
    }

    pub fn poll(&mut self, now: String, text: &str) -> Poll {
        if now == self.disk {
            return Poll::Same;
        }
        if self.edited(text) {
            if std::mem::replace(&mut self.outside, true) {
                return Poll::Same;
            }
            return Poll::Outside;
        }
        self.disk = now.clone();
        self.loaded = now.clone();
        Poll::Replace(now)
    }

    pub fn saved(&mut self, text: &str) {
        self.disk = text.to_owned();
        self.loaded = text.to_owned();
        self.outside = false;
    }

    pub fn state(&self, exists: bool, text: &str) -> (&'static str, StateTone) {
        if self.outside {
            ("Mudou no disco enquanto você editava", StateTone::Warn)
        } else if !exists {
            ("Não existe ainda — salvar cria", StateTone::Faint)
        } else if self.dirty(text) {
            ("Não salvo", StateTone::Warn)
        } else {
            ("Salvo", StateTone::Faint)
        }
    }

    pub fn can_save(&self, exists: bool, text: &str) -> bool {
        self.dirty(text) || !exists
    }

    pub fn can_revert(&self, text: &str) -> bool {
        self.edited(text) || self.outside
    }
}

pub fn same_list(a: &[SetupSlot], b: &[SetupSlot]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.rel == y.rel && x.exists == y.exists)
}

pub fn slot_for(slots: &[SetupSlot], root: &str, rel: &str) -> SetupSlot {
    slots
        .iter()
        .find(|s| s.rel == rel)
        .cloned()
        .unwrap_or_else(|| setup::phantom_slot(root, rel))
}

pub fn prompt_label(section: SetupSection) -> &'static str {
    match section {
        SetupSection::Skills => "Nome (vira /nome)",
        SetupSection::Decisions => "Título da decisão",
        _ => "Nome",
    }
}

pub fn delete_detail(slot: &SetupSlot, root: &str) -> String {
    if slot.section == SetupSection::Skills && slot.rel.ends_with("/SKILL.md") {
        "A pasta da skill vai junto, com o que mais houver dentro dela.".into()
    } else {
        setup::path_of(root, &slot.rel)
    }
}

pub fn save_tooltip() -> &'static str {
    if cfg!(target_os = "macos") { "Salvar  ⌘S" } else { "Salvar  ctrl+S" }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
