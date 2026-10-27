mod scan;
mod templates;

use serde_json::{Map, Value, json};

pub use scan::{path_of, phantom_slot, read, remove, scan, write};
pub use templates::{next_adr, rel_for, slug, template};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SetupSection {
    Memory,
    Settings,
    Rules,
    Skills,
    Agents,
    Decisions,
}

impl SetupSection {
    pub const ALL: [SetupSection; 6] = [
        Self::Memory,
        Self::Settings,
        Self::Rules,
        Self::Skills,
        Self::Agents,
        Self::Decisions,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Memory => "Memória",
            Self::Settings => "Configuração",
            Self::Rules => "Regras",
            Self::Skills => "Skills",
            Self::Agents => "Agentes",
            Self::Decisions => "Decisões (ADR)",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Self::Memory => {
                "O que o Claude lê no começo de toda sessão nesta pasta. Curto e prescritivo rende mais que descritivo: comandos exatos, convenções que não se deduzem do código, armadilhas conhecidas."
            }
            Self::Settings => {
                "O que o harness lê, não o modelo: permissões pré-aprovadas, hooks que rodam antes ou depois de uma ação, variáveis de ambiente e servidores MCP."
            }
            Self::Rules => {
                "Regras que só carregam quando um arquivo que bate no glob delas é tocado. É o que mantém o CLAUDE.md enxuto: o que só importa em `test/` fica num arquivo que só entra lá."
            }
            Self::Skills => {
                "Fluxos repetíveis, invocados com /nome: gerar a release, subir o app na VM, fazer o DMG. Cada uma é uma pasta com um SKILL.md dentro."
            }
            Self::Agents => {
                "Subagentes com prompt e ferramentas próprias, pra revisão com um checklist fixo ou uma busca que não precisa do contexto inteiro."
            }
            Self::Decisions => {
                "Uma decisão de arquitetura por arquivo: contexto, decisão, consequências. Vale sobretudo pra decisão contra-intuitiva, a que o Claude tenderia a \"corrigir\" sem saber. Ele não lê esta pasta sozinho -- aponte pra ela no CLAUDE.md."
            }
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            Self::Memory => include_str!("help/memory.md"),
            Self::Settings => include_str!("help/settings.md"),
            Self::Rules => include_str!("help/rules.md"),
            Self::Skills => include_str!("help/skills.md"),
            Self::Agents => include_str!("help/agents.md"),
            Self::Decisions => include_str!("help/decisions.md"),
        }
    }

    pub fn open(self) -> bool {
        !matches!(self, Self::Memory | Self::Settings)
    }

    pub fn noun(self) -> &'static str {
        match self {
            Self::Memory | Self::Settings => "arquivo",
            Self::Rules => "regra",
            Self::Skills => "skill",
            Self::Agents => "agente",
            Self::Decisions => "decisão",
        }
    }

    pub fn of_rel(rel: &str) -> Self {
        if rel.starts_with(".claude/rules/") {
            Self::Rules
        } else if rel.starts_with(".claude/skills/") || rel.starts_with(".claude/commands/") {
            Self::Skills
        } else if rel.starts_with(".claude/agents/") {
            Self::Agents
        } else if rel.starts_with("docs/adr/") {
            Self::Decisions
        } else if rel.ends_with(".json") {
            Self::Settings
        } else {
            Self::Memory
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupSlot {
    pub section: SetupSection,
    pub rel: String,
    pub label: String,
    pub about: String,
    pub exists: bool,
    pub fixed: bool,
}

impl SetupSlot {
    pub fn is_json(&self) -> bool {
        self.rel.ends_with(".json")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MxSetup {
    pub root: String,
    pub selected: Option<String>,
}

impl MxSetup {
    pub fn name(&self) -> &str {
        self.root.rsplit('/').next().unwrap_or(&self.root)
    }

    pub fn to_json(&self) -> Value {
        let mut out = Map::new();
        out.insert("root".into(), json!(self.root));
        if let Some(selected) = &self.selected {
            out.insert("selected".into(), json!(selected));
        }
        Value::Object(out)
    }

    pub fn from_json(json: &Value) -> Option<Self> {
        let root = json.get("root")?.as_str()?.trim();
        if root.is_empty() {
            return None;
        }
        Some(Self {
            root: root.to_owned(),
            selected: json
                .get("selected")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }
}
