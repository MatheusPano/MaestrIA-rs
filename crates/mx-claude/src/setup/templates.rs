use std::sync::OnceLock;

use chrono::Local;
use regex::Regex;

use super::SetupSection;
use super::scan::path_of;

const ACCENTED: &str = "áàâãäéèêëíìîïóòôõöúùûüçñ";
const PLAIN: &str = "aaaaaeeeeiiiiooooouuuucn";

fn regex(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).unwrap())
}

pub fn slug(name: &str) -> String {
    static NON_ALNUM: OnceLock<Regex> = OnceLock::new();
    let mapped: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            ACCENTED
                .chars()
                .position(|a| a == c)
                .and_then(|i| PLAIN.chars().nth(i))
                .unwrap_or(c)
        })
        .collect();
    regex(&NON_ALNUM, "[^a-z0-9]+")
        .replace_all(&mapped, "-")
        .trim_matches('-')
        .to_owned()
}

pub fn next_adr(root: &str) -> u32 {
    static NUMBERED: OnceLock<Regex> = OnceLock::new();
    let Ok(entries) = std::fs::read_dir(path_of(root, "docs/adr")) else {
        return 1;
    };
    let re = regex(&NUMBERED, r"^([0-9]{1,5})[-_]");
    entries
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            re.captures(&name)?.get(1)?.as_str().parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0)
        + 1
}

pub fn rel_for(section: SetupSection, name: &str, root: &str) -> Option<String> {
    let s = slug(name);
    if s.is_empty() {
        return None;
    }
    match section {
        SetupSection::Rules => Some(format!(".claude/rules/{s}.md")),
        SetupSection::Skills => Some(format!(".claude/skills/{s}/SKILL.md")),
        SetupSection::Agents => Some(format!(".claude/agents/{s}.md")),
        SetupSection::Decisions => Some(format!("docs/adr/{:04}-{s}.md", next_adr(root))),
        SetupSection::Memory | SetupSection::Settings => None,
    }
}

const SETTINGS_TEMPLATE: &str = "{\n  \"permissions\": {\n    \"allow\": [],\n    \"deny\": []\n  },\n  \"env\": {},\n  \"hooks\": {}\n}\n";

pub fn template(rel: &str, folder_name: &str, title: &str) -> String {
    static LEADING_DIGITS: OnceLock<Regex> = OnceLock::new();
    let base = rel.rsplit('/').next().unwrap_or(rel);
    let name = if title.is_empty() {
        base.replace(".md", "")
    } else {
        title.to_owned()
    };
    match rel {
        "CLAUDE.md" => {
            return format!(
                "# {folder_name}\n\n## Comandos\n\n- build: \n- testes: \n- lint: \n\n## Convenções\n\n- \n\n## Armadilhas\n\n- \n\n## Mapa\n\n- `lib/` — \n\n## Decisões\n\nAs decisões de arquitetura estão em `docs/adr/`. Leia antes de mudar algo estrutural.\n"
            );
        }
        "CLAUDE.local.md" => {
            return format!(
                "# {folder_name} — só nesta máquina\n\nNão vai no git: acrescente `CLAUDE.local.md` ao `.gitignore`.\n\n- \n"
            );
        }
        ".claude/settings.json" | ".claude/settings.local.json" => {
            return SETTINGS_TEMPLATE.to_owned();
        }
        ".mcp.json" => return "{\n  \"mcpServers\": {}\n}\n".to_owned(),
        _ => {}
    }
    if rel.starts_with(".claude/rules/") {
        return format!("---\npaths:\n  - \"**/*\"\n---\n\n# {name}\n\n- \n");
    }
    if rel.starts_with(".claude/skills/") {
        let skill = rel.split('/').nth(2).unwrap_or("");
        return format!(
            "---\nname: {skill}\ndescription: Quando usar esta skill, em uma frase que o Claude consiga reconhecer no pedido.\n---\n\n# {skill}\n\n## Passos\n\n1. \n"
        );
    }
    if rel.starts_with(".claude/commands/") {
        return format!("# {name}\n\n");
    }
    if rel.starts_with(".claude/agents/") {
        return format!(
            "---\nname: {name}\ndescription: Quando delegar pra este agente.\ntools: Read, Grep, Glob, Bash\nmodel: inherit\n---\n\nVocê é \n"
        );
    }
    if rel.starts_with("docs/adr/") {
        let date = Local::now().format("%Y-%m-%d");
        let prefix = regex(&LEADING_DIGITS, "^([0-9]+)")
            .captures(base)
            .map(|c| format!("ADR {}: ", &c[1]))
            .unwrap_or_default();
        return format!(
            "# {prefix}{name}\n\n- **Status:** proposta\n- **Data:** {date}\n\n## Contexto\n\n\n## Decisão\n\n\n## Consequências\n\n"
        );
    }
    String::new()
}
