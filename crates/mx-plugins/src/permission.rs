#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Permission {
    Hooks,
    TerminalWrite,
    SessionsCreate,
    Notifications,
}

impl Permission {
    pub const ALL: [Permission; 4] = [
        Self::Hooks,
        Self::TerminalWrite,
        Self::SessionsCreate,
        Self::Notifications,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Hooks => "hooks",
            Self::TerminalWrite => "terminal.write",
            Self::SessionsCreate => "sessions.create",
            Self::Notifications => "notifications",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Hooks => {
                "ver os eventos das sessões do Claude — o que você pede, que ferramentas rodam e com quais argumentos"
            }
            Self::TerminalWrite => "digitar e enviar texto nos seus painéis",
            Self::SessionsCreate => "abrir sessões do Claude e terminais",
            Self::Notifications => "mandar notificações do sistema",
        }
    }

    pub fn by_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }
}
