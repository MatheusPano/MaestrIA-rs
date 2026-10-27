#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Permission {
    Hooks,
    TerminalWrite,
    SessionsCreate,
    SessionsClose,
    Notifications,
}

impl Permission {
    pub const ALL: [Permission; 5] = [
        Self::Hooks,
        Self::TerminalWrite,
        Self::SessionsCreate,
        Self::SessionsClose,
        Self::Notifications,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Hooks => "hooks",
            Self::TerminalWrite => "terminal.write",
            Self::SessionsCreate => "sessions.create",
            Self::SessionsClose => "sessions.close",
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
            Self::SessionsClose => "encerrar sessões do Claude quando você pedir",
            Self::Notifications => "mandar notificações do sistema",
        }
    }

    pub fn by_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }
}
