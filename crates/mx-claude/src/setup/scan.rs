use std::io;
use std::path::Path;

use super::{SetupSection, SetupSlot};

const FIXED: [(SetupSection, &str, &str); 5] = [
    (
        SetupSection::Memory,
        "CLAUDE.md",
        "Carregado em toda sessão e versionado com o repo. Comandos de build e teste, convenções, armadilhas, um mapa curto das pastas.",
    ),
    (
        SetupSection::Memory,
        "CLAUDE.local.md",
        "O mesmo, só seu: não vai no git. Pra caminho de máquina, preferência pessoal e anotação que não é do time.",
    ),
    (
        SetupSection::Settings,
        ".claude/settings.json",
        "Permissões, hooks e variáveis de ambiente do projeto, versionados. É o que o time inteiro compartilha.",
    ),
    (
        SetupSection::Settings,
        ".claude/settings.local.json",
        "O mesmo, só desta máquina: não vai no git. Onde as permissões que você aprovou vão parar.",
    ),
    (
        SetupSection::Settings,
        ".mcp.json",
        "Os servidores MCP do projeto, versionados -- as ferramentas de fora que toda sessão aqui ganha.",
    ),
];

const RULE_ABOUT: &str = "Entra no contexto quando um arquivo que bate no `paths` do cabeçalho é lido ou editado. Sem `paths`, entra sempre.";
const COMMAND_ABOUT: &str = "Um comando de barra do jeito antigo: o corpo é o prompt que /nome dispara. Skills fazem o mesmo com mais estrutura.";
const AGENT_ABOUT: &str = "Um subagente: o cabeçalho diz nome, quando usá-lo e com que ferramentas; o corpo é o prompt de sistema dele.";
const ADR_ABOUT: &str = "Uma decisão: o contexto em que foi tomada, o que se decidiu e o que isso custa. Curta, e nunca reescrita -- uma decisão nova é outro ADR.";

pub fn path_of(root: &str, rel: &str) -> String {
    format!("{root}/{rel}")
}

fn base_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub fn scan(root: &str) -> Vec<SetupSlot> {
    let mut out: Vec<SetupSlot> = FIXED
        .iter()
        .map(|(section, rel, about)| SetupSlot {
            section: *section,
            rel: (*rel).to_owned(),
            label: base_name(rel).to_owned(),
            about: (*about).to_owned(),
            exists: Path::new(&path_of(root, rel)).is_file(),
            fixed: true,
        })
        .collect();
    out.extend(files(
        root,
        SetupSection::Rules,
        ".claude/rules",
        RULE_ABOUT,
    ));
    out.extend(skills(root));
    out.extend(files(
        root,
        SetupSection::Skills,
        ".claude/commands",
        COMMAND_ABOUT,
    ));
    out.extend(files(
        root,
        SetupSection::Agents,
        ".claude/agents",
        AGENT_ABOUT,
    ));
    out.extend(files(root, SetupSection::Decisions, "docs/adr", ADR_ABOUT));
    out
}

pub fn phantom_slot(root: &str, rel: &str) -> SetupSlot {
    let section = SetupSection::of_rel(rel);
    SetupSlot {
        section,
        rel: rel.to_owned(),
        label: base_name(rel).to_owned(),
        about: section.about().to_owned(),
        exists: Path::new(&path_of(root, rel)).is_file(),
        fixed: false,
    }
}

fn rel_of(root: &str, path: &str) -> String {
    let base = if root.ends_with('/') {
        root.to_owned()
    } else {
        format!("{root}/")
    };
    path.strip_prefix(&base).unwrap_or(path).to_owned()
}

fn walk_files(dir: &str, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = format!("{dir}/{}", entry.file_name().to_string_lossy());
        if kind.is_dir() {
            walk_files(&path, out);
        } else if kind.is_file() {
            out.push(path);
        }
    }
}

fn files(root: &str, section: SetupSection, dir: &str, about: &str) -> Vec<SetupSlot> {
    let at = path_of(root, dir);
    let mut found = Vec::new();
    walk_files(&at, &mut found);
    found.retain(|p| p.to_lowercase().ends_with(".md"));
    found.sort();
    found
        .into_iter()
        .map(|path| SetupSlot {
            section,
            rel: rel_of(root, &path),
            label: rel_of(&at, &path),
            about: about.to_owned(),
            exists: true,
            fixed: false,
        })
        .collect()
}

fn skills(root: &str) -> Vec<SetupSlot> {
    let at = path_of(root, ".claude/skills");
    let Ok(entries) = std::fs::read_dir(&at) else {
        return Vec::new();
    };
    let mut dirs: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| format!("{at}/{}", e.file_name().to_string_lossy()))
        .collect();
    dirs.sort();
    dirs.into_iter()
        .filter(|d| Path::new(&format!("{d}/SKILL.md")).is_file())
        .map(|d| {
            let name = base_name(&d).to_owned();
            SetupSlot {
                section: SetupSection::Skills,
                rel: rel_of(root, &format!("{d}/SKILL.md")),
                about: format!(
                    "Um fluxo com nome: o cabeçalho diz quando ele vale, o corpo diz o passo a passo. Invocado com /{name}."
                ),
                label: name,
                exists: true,
                fixed: false,
            }
        })
        .collect()
}

pub fn read(root: &str, rel: &str) -> String {
    match std::fs::read(path_of(root, rel)) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => String::new(),
    }
}

pub fn write(root: &str, rel: &str, text: &str) -> io::Result<()> {
    let path = path_of(root, rel);
    if let Some(parent) = Path::new(&path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)
}

pub fn remove(root: &str, rel: &str) -> io::Result<()> {
    let path = path_of(root, rel);
    if rel.starts_with(".claude/skills/") && rel.ends_with("/SKILL.md") {
        if let Some(dir) = Path::new(&path).parent().filter(|d| d.is_dir()) {
            std::fs::remove_dir_all(dir)?;
        }
        return Ok(());
    }
    if Path::new(&path).is_file() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
