use mx_claude::setup::{self, SetupSection};

use super::*;
use crate::setup::tests_support::Scratch;

fn root() -> (Scratch, String) {
    let dir = Scratch::new();
    let root = dir.0.to_string_lossy().into_owned();
    (dir, root)
}

#[test]
fn an_existing_file_loads_from_disk_and_a_missing_one_loads_its_template() {
    let (_d, root) = root();
    setup::write(&root, "CLAUDE.md", "# real\n").unwrap();
    let slots = setup::scan(&root);
    let claude = slot_for(&slots, &root, "CLAUDE.md");
    let doc = DocState::load(Some(&claude), &root, "proj");
    assert_eq!(doc.disk, "# real\n");
    assert_eq!(doc.loaded, doc.disk);
    let mcp = slot_for(&slots, &root, ".mcp.json");
    let doc = DocState::load(Some(&mcp), &root, "proj");
    assert_eq!(doc.disk, "");
    assert_eq!(doc.loaded, "{\n  \"mcpServers\": {}\n}\n");
    assert!(doc.dirty(&doc.loaded));
    assert!(!doc.edited(&doc.loaded));
    assert_eq!(doc.state(false, &doc.loaded).0, "Não existe ainda — salvar cria");
    assert!(doc.can_save(false, &doc.loaded));
}

#[test]
fn a_disk_change_replaces_an_untouched_editor_and_flags_an_edited_one() {
    let mut doc = DocState { disk: "a".into(), loaded: "a".into(), outside: false };
    assert_eq!(doc.poll("a".into(), "a"), Poll::Same);
    assert_eq!(doc.poll("b".into(), "a"), Poll::Replace("b".into()));
    assert_eq!(doc.disk, "b");
    assert_eq!(doc.poll("c".into(), "typed"), Poll::Outside);
    assert!(doc.outside);
    assert_eq!(doc.poll("c".into(), "typed"), Poll::Same);
    assert_eq!(doc.poll("d".into(), "typed"), Poll::Same);
    assert!(doc.outside);
    assert_eq!(doc.disk, "b");
    assert_eq!(doc.state(true, "typed"), ("Mudou no disco enquanto você editava", StateTone::Warn));
    assert!(doc.can_revert("b"));
}

#[test]
fn state_text_follows_outside_then_missing_then_dirty_then_saved() {
    let mut doc = DocState { disk: "x".into(), loaded: "x".into(), outside: false };
    assert_eq!(doc.state(true, "x"), ("Salvo", StateTone::Faint));
    assert_eq!(doc.state(true, "y"), ("Não salvo", StateTone::Warn));
    assert!(!doc.can_save(true, "x"));
    assert!(!doc.can_revert("x"));
    doc.saved("y");
    assert_eq!(doc.state(true, "y").0, "Salvo");
}

#[test]
fn a_selected_rel_missing_from_the_scan_becomes_a_phantom_in_its_section() {
    let (_d, root) = root();
    let slots = setup::scan(&root);
    let ghost = slot_for(&slots, &root, ".claude/agents/revisor.md");
    assert_eq!(ghost.section, SetupSection::Agents);
    assert_eq!(ghost.label, "revisor.md");
    assert!(!ghost.exists);
    assert_eq!(slot_for(&slots, &root, "notes/x.json").section, SetupSection::Settings);
}

#[test]
fn the_list_only_counts_as_changed_when_a_rel_or_its_existence_changes() {
    let (_d, root) = root();
    let before = setup::scan(&root);
    assert!(same_list(&before, &setup::scan(&root)));
    setup::write(&root, "CLAUDE.md", "x").unwrap();
    assert!(!same_list(&before, &setup::scan(&root)));
}

#[test]
fn prompt_and_delete_texts_depend_on_the_section() {
    assert_eq!(prompt_label(SetupSection::Skills), "Nome (vira /nome)");
    assert_eq!(prompt_label(SetupSection::Decisions), "Título da decisão");
    assert_eq!(prompt_label(SetupSection::Rules), "Nome");
    let (_d, root) = root();
    let skill = slot_for(&[], &root, ".claude/skills/release/SKILL.md");
    assert_eq!(delete_detail(&skill, &root), "A pasta da skill vai junto, com o que mais houver dentro dela.");
    let rule = slot_for(&[], &root, ".claude/rules/x.md");
    assert_eq!(delete_detail(&rule, &root), setup::path_of(&root, ".claude/rules/x.md"));
}
