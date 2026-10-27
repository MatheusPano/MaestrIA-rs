use serde_json::{Value, json};

use crate::json::{Obj, str_of};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FollowUpKind {
    KeepGoing,
    NewSession,
    Command,
    Handoff,
}

impl FollowUpKind {
    pub const ALL: [FollowUpKind; 4] = [
        Self::KeepGoing,
        Self::NewSession,
        Self::Command,
        Self::Handoff,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::KeepGoing => "keepGoing",
            Self::NewSession => "newSession",
            Self::Command => "command",
            Self::Handoff => "handoff",
        }
    }

    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::KeepGoing => "Continuar aqui",
            Self::NewSession => "Abrir outra sessão",
            Self::Command => "Rodar um comando",
            Self::Handoff => "Passar pra outro painel",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::KeepGoing => "Continuar",
            Self::NewSession => "Outra sessão",
            Self::Command => "Comando",
            Self::Handoff => "Passar a bola",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Self::KeepGoing => "Manda o texto de volta pra esta mesma sessão",
            Self::NewSession => "Abre uma sessão nova na mesma pasta, já com esse prompt",
            Self::Command => "Sobe um terminal rodando a linha abaixo",
            Self::Handoff => "Entrega o recado final desta sessão a outro painel",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::KeepGoing => "O que mandar pra ela em seguida",
            Self::NewSession => "O prompt com que a sessão nova abre",
            Self::Command => "flutter analyze && flutter test",
            Self::Handoff => "O recado que vai junto com a última mensagem dela",
        }
    }

    pub fn hands_back(self) -> bool {
        self == Self::KeepGoing
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowUp {
    pub kind: FollowUpKind,
    pub text: String,
    pub target_tab_id: Option<String>,
}

impl FollowUp {
    pub fn to_json(&self) -> Value {
        let mut m = Obj::new();
        m.insert("kind".into(), json!(self.kind.name()));
        m.insert("text".into(), json!(self.text));
        if let Some(t) = &self.target_tab_id {
            m.insert("target".into(), json!(t));
        }
        Value::Object(m)
    }

    pub fn from_json(v: &Value) -> Option<FollowUp> {
        let m = v.as_object()?;
        let kind = FollowUpKind::by_name(str_of(m, "kind")?)?;
        Some(FollowUp {
            kind,
            text: str_of(m, "text").unwrap_or("").to_owned(),
            target_tab_id: str_of(m, "target").map(str::to_owned),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowHold {
    Go,
    Working,
    Forks,
    Quiet,
}

impl FlowHold {
    pub fn label(self) -> &'static str {
        match self {
            Self::Go => "disparando",
            Self::Working => "a sessão ainda está trabalhando",
            Self::Forks => "esperando os agentes que ela abriu",
            Self::Quiet => "confirmando que ela parou mesmo",
        }
    }
}
