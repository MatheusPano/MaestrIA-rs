#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ClaudeStatus {
    Unknown,
    #[default]
    Starting,
    Ready,
    Working,
    Tool,
    WaitingInput,
    WaitingAnswer,
    WaitingPermission,
    Idle,
    Ended,
}

impl ClaudeStatus {
    pub const ALL: [ClaudeStatus; 10] = [
        Self::Unknown,
        Self::Starting,
        Self::Ready,
        Self::Working,
        Self::Tool,
        Self::WaitingInput,
        Self::WaitingAnswer,
        Self::WaitingPermission,
        Self::Idle,
        Self::Ended,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Working => "working",
            Self::Tool => "tool",
            Self::WaitingInput => "waitingInput",
            Self::WaitingAnswer => "waitingAnswer",
            Self::WaitingPermission => "waitingPermission",
            Self::Idle => "idle",
            Self::Ended => "ended",
        }
    }

    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "—",
            Self::Starting => "Abrindo",
            Self::Ready => "Pronto",
            Self::Working => "Pensando",
            Self::Tool => "Rodando",
            Self::WaitingInput => "Esperando você",
            Self::WaitingAnswer => "Pergunta",
            Self::WaitingPermission => "Permissão",
            Self::Idle => "Ocioso",
            Self::Ended => "Encerrada",
        }
    }

    pub fn at_rest(self) -> bool {
        matches!(self, Self::Idle | Self::WaitingInput)
    }

    pub fn needs_human(self) -> bool {
        matches!(
            self,
            Self::WaitingInput | Self::WaitingAnswer | Self::WaitingPermission
        )
    }

    pub fn call_to_action(self) -> &'static str {
        match self {
            Self::WaitingPermission => "quer aprovação",
            Self::WaitingAnswer => "te faz uma pergunta",
            _ => "te espera",
        }
    }
}
