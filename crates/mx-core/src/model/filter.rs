use crate::model::{ClaudeStatus, Tab, TabKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterGroup {
    Kind,
    State,
}

impl FilterGroup {
    pub fn label(self) -> &'static str {
        match self {
            Self::Kind => "Tipo",
            Self::State => "Estado",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Filter {
    Claude,
    Shell,
    Waiting,
    Working,
    Parked,
    Finished,
}

impl Filter {
    pub const ALL: [Filter; 6] = [
        Self::Claude,
        Self::Shell,
        Self::Waiting,
        Self::Working,
        Self::Parked,
        Self::Finished,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Claude => "Sessões do Claude",
            Self::Shell => "Terminais",
            Self::Waiting => "Esperando você",
            Self::Working => "Rodando",
            Self::Parked => "Paradas",
            Self::Finished => "Concluídas",
        }
    }

    pub fn group(self) -> FilterGroup {
        match self {
            Self::Claude | Self::Shell => FilterGroup::Kind,
            _ => FilterGroup::State,
        }
    }

    pub fn matches(self, t: &Tab) -> bool {
        let status = t.status();
        match self {
            Self::Claude => t.kind == TabKind::Claude,
            Self::Shell => t.kind == TabKind::Shell,
            Self::Waiting => !t.done && status.needs_human(),
            Self::Working => matches!(status, ClaudeStatus::Working | ClaudeStatus::Tool),
            Self::Parked => {
                !t.done
                    && (matches!(
                        status,
                        ClaudeStatus::Idle | ClaudeStatus::Ready | ClaudeStatus::Ended
                    ) || t.exited)
            }
            Self::Finished => t.done,
        }
    }
}
