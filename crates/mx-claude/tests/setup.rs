use std::collections::HashMap;
use std::path::Path;

use mx_claude::setup::{
    MxSetup, SetupSection, next_adr, phantom_slot, read, rel_for, remove, scan, slug, template,
    write,
};
use serde_json::json;

fn touch(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn root_of(dir: &tempfile::TempDir) -> String {
    dir.path().to_string_lossy().into_owned()
}

#[test]
fn lists_the_fixed_files_even_when_none_exists() {
    let repo = tempfile::tempdir().unwrap();
    let slots = scan(&root_of(&repo));
    let rels: Vec<&str> = slots.iter().map(|s| s.rel.as_str()).collect();
    assert_eq!(
        rels,
        [
            "CLAUDE.md",
            "CLAUDE.local.md",
            ".claude/settings.json",
            ".claude/settings.local.json",
            ".mcp.json"
        ]
    );
    assert!(slots.iter().all(|s| !s.exists && s.fixed));
}

#[test]
fn sees_what_exists_and_discovers_rules_skills_agents_and_adrs() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();
    touch(root, "CLAUDE.md", "# oi");
    touch(root, ".claude/rules/tests.md", "");
    touch(root, ".claude/rules/sub/ui.md", "");
    touch(root, ".claude/skills/release/SKILL.md", "");
    std::fs::create_dir_all(root.join(".claude/skills/vazia")).unwrap();
    touch(root, ".claude/commands/old.md", "");
    touch(root, ".claude/agents/revisor.md", "");
    touch(root, "docs/adr/0001-flutter.md", "");

    let slots = scan(&root_of(&repo));
    let by_rel: HashMap<&str, _> = slots.iter().map(|s| (s.rel.as_str(), s)).collect();
    assert!(by_rel["CLAUDE.md"].exists);
    assert!(!by_rel["CLAUDE.local.md"].exists);
    assert_eq!(
        by_rel[".claude/rules/tests.md"].section,
        SetupSection::Rules
    );
    assert_eq!(by_rel[".claude/rules/sub/ui.md"].label, "sub/ui.md");
    assert_eq!(by_rel[".claude/skills/release/SKILL.md"].label, "release");
    assert!(
        by_rel[".claude/skills/release/SKILL.md"]
            .about
            .ends_with("Invocado com /release.")
    );
    assert!(by_rel.keys().all(|r| !r.contains("vazia")));
    assert_eq!(
        by_rel[".claude/commands/old.md"].section,
        SetupSection::Skills
    );
    assert_eq!(
        by_rel[".claude/agents/revisor.md"].section,
        SetupSection::Agents
    );
    assert_eq!(
        by_rel["docs/adr/0001-flutter.md"].section,
        SetupSection::Decisions
    );
    assert!(!by_rel["docs/adr/0001-flutter.md"].fixed);
    assert!(by_rel[".mcp.json"].is_json());
}

#[test]
fn name_becomes_a_slug_and_the_path_depends_on_the_section() {
    let repo = tempfile::tempdir().unwrap();
    let root = root_of(&repo);
    assert_eq!(slug("Gerar Release  do App!"), "gerar-release-do-app");
    assert_eq!(slug("Decisão sobre PTY"), "decisao-sobre-pty");
    assert_eq!(slug("***"), "");
    assert_eq!(
        rel_for(SetupSection::Rules, "testes", &root).as_deref(),
        Some(".claude/rules/testes.md")
    );
    assert_eq!(
        rel_for(SetupSection::Skills, "release", &root).as_deref(),
        Some(".claude/skills/release/SKILL.md")
    );
    assert_eq!(
        rel_for(SetupSection::Agents, "Revisor", &root).as_deref(),
        Some(".claude/agents/revisor.md")
    );
    assert_eq!(rel_for(SetupSection::Memory, "x", &root), None);
    assert_eq!(rel_for(SetupSection::Rules, "!!!", &root), None);
}

#[test]
fn an_adr_gets_the_next_number_in_sequence() {
    let repo = tempfile::tempdir().unwrap();
    let root = root_of(&repo);
    assert_eq!(next_adr(&root), 1);
    touch(repo.path(), "docs/adr/0001-a.md", "");
    touch(repo.path(), "docs/adr/0007-b.md", "");
    touch(repo.path(), "docs/adr/README.md", "");
    assert_eq!(next_adr(&root), 8);
    assert_eq!(
        rel_for(SetupSection::Decisions, "usar PTY próprio", &root).as_deref(),
        Some("docs/adr/0008-usar-pty-proprio.md")
    );
}

#[test]
fn writing_creates_parents_and_removing_a_skill_takes_its_folder() {
    let repo = tempfile::tempdir().unwrap();
    let root = root_of(&repo);
    write(&root, ".claude/settings.json", "{}").unwrap();
    assert_eq!(read(&root, ".claude/settings.json"), "{}");
    write(&root, ".claude/skills/x/SKILL.md", "---\nname: x\n---").unwrap();
    write(&root, ".claude/skills/x/extra.txt", "y").unwrap();
    remove(&root, ".claude/skills/x/SKILL.md").unwrap();
    assert!(!repo.path().join(".claude/skills/x").exists());
    write(&root, "CLAUDE.md", "z").unwrap();
    remove(&root, "CLAUDE.md").unwrap();
    assert!(!repo.path().join("CLAUDE.md").exists());
    remove(&root, "CLAUDE.md").unwrap();
}

#[test]
fn reading_never_blanks_on_bad_bytes_or_missing_files() {
    let repo = tempfile::tempdir().unwrap();
    let root = root_of(&repo);
    assert_eq!(read(&root, "CLAUDE.md"), "");
    std::fs::write(repo.path().join("CLAUDE.md"), b"caf\xe9").unwrap();
    assert_eq!(read(&root, "CLAUDE.md"), "caf\u{fffd}");
}

#[test]
fn every_section_explains_itself_with_an_example() {
    for section in SetupSection::ALL {
        assert!(!section.about().is_empty(), "{section:?}");
        assert!(section.help().contains("## O que é"), "{section:?}");
        assert!(section.help().contains("```"), "{section:?}");
    }
    assert!(SetupSection::Agents.help().contains("subagente"));
    assert!(SetupSection::Agents.help().contains("`tools`"));
    assert!(
        SetupSection::Settings
            .help()
            .contains("\"Bearer ${WIBOOR_TOKEN}\"")
    );
    let open: Vec<bool> = SetupSection::ALL.iter().map(|s| s.open()).collect();
    assert_eq!(open, [false, false, true, true, true, true]);
}

#[test]
fn every_known_file_has_a_template_and_the_skill_one_carries_its_name() {
    let repo = tempfile::tempdir().unwrap();
    for slot in scan(&root_of(&repo)) {
        assert!(
            !template(&slot.rel, "meu-repo", "").is_empty(),
            "{}",
            slot.rel
        );
    }
    assert!(template(".claude/skills/release/SKILL.md", "r", "").contains("name: release"));
    assert!(
        template("docs/adr/0003-pty.md", "r", "PTY próprio")
            .starts_with("# ADR 0003: PTY próprio\n")
    );
    assert!(template("CLAUDE.md", "meu-repo", "").starts_with("# meu-repo\n\n## Comandos\n"));
    assert_eq!(
        template(".mcp.json", "r", ""),
        "{\n  \"mcpServers\": {}\n}\n"
    );
    assert_eq!(template("outro.txt", "r", ""), "");
}

#[test]
fn a_selected_file_missing_from_the_scan_becomes_a_phantom_slot() {
    let cases = [
        (".claude/rules/a.md", SetupSection::Rules),
        (".claude/commands/a.md", SetupSection::Skills),
        (".claude/agents/a.md", SetupSection::Agents),
        ("docs/adr/0001-a.md", SetupSection::Decisions),
        ("outro.json", SetupSection::Settings),
        ("NOTAS.md", SetupSection::Memory),
    ];
    for (rel, section) in cases {
        let slot = phantom_slot("/nao/existe", rel);
        assert_eq!(slot.section, section, "{rel}");
        assert!(!slot.exists);
    }
}

#[test]
fn the_recipe_keeps_root_and_open_file() {
    let setup = MxSetup {
        root: "/repo".into(),
        selected: Some("CLAUDE.md".into()),
    };
    assert_eq!(MxSetup::from_json(&setup.to_json()), Some(setup.clone()));
    assert_eq!(setup.name(), "repo");
    assert_eq!(MxSetup::from_json(&json!({})), None);
    assert_eq!(MxSetup::from_json(&json!({"root": "  "})), None);
}
