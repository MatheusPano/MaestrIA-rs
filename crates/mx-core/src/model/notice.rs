use crate::text::Time;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoticeKind {
    Question,
    Permission,
    Finished,
    Suggested,
}

impl NoticeKind {
    pub fn verb(self) -> &'static str {
        match self {
            Self::Question => "te fez uma pergunta",
            Self::Permission => "quer aprovação",
            Self::Finished => "terminou o trabalho",
            Self::Suggested => "sugeriu uma tarefa",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub tab_id: String,
    pub kind: NoticeKind,
    pub title: String,
    pub at: Time,
    pub suggestion_id: Option<String>,
    pub detail: Option<String>,
    pub read: bool,
}

pub const NOTICE_CAP: usize = 50;
pub const TOAST_CAP: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuggestionChoice {
    Worktree,
    Local,
    Here,
    Dismiss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuggestionEnd {
    Started,
    Dismissed,
}
