use mx_core::model::FeatureOrHotfixKind;

use crate::chrome::dialog::confirm::{self, WorktreeSafety};
use crate::chrome::dialog::lists::{no_features_banner, none_label};
use crate::chrome::dialog::specs;

#[test]
fn clearing_two_groups_asks_with_facts() {
    let c = confirm::clear_groups(2);
    assert_eq!(c.title, "Esquecer os 2 grupos?");
    assert_eq!(c.primary, "Esquecer");
    assert!(c.facts[1].2.contains("2 arranjos guardados"));
}

#[test]
fn worktree_with_uncommitted_files_is_risky() {
    let s = WorktreeSafety { dirty: Some(1), base: Some("main".into()), unmerged: 0 };
    let c = confirm::remove_worktree("TASK#1", "/r/TASK-1", false, &s, Some("feature/TASK#1"));
    assert_eq!(c.title, "Excluir TASK#1?");
    assert_eq!(c.primary, "Excluir de qualquer forma");
    assert_eq!(c.facts[0].2, "1 arquivo não commitado — isso se perde");
    assert_eq!(c.facts[1].2, "Todo commit daqui já está em main");
    assert_eq!(c.check.as_deref(), Some("Apagar também a branch feature/TASK#1"));
    let clean = WorktreeSafety { dirty: Some(0), base: Some("main".into()), unmerged: 0 };
    assert_eq!(confirm::remove_worktree("x", "/x", false, &clean, None).primary, "Excluir");
    let ghost = confirm::remove_worktree("x", "/x", true, &clean, None);
    assert_eq!(ghost.title, "Limpar x?");
}

#[test]
fn close_workspace_counts_folders_and_sessions() {
    let c = confirm::close_workspace("ATRIUM", None, 1, 3);
    assert_eq!(c.facts[0].2, "1 pasta sai da lateral");
    assert_eq!(c.facts[2].2, "3 sessões abertas são encerradas");
    assert_eq!(confirm::close_workspace("A", None, 2, 0).facts.len(), 2);
}

#[test]
fn completing_a_busy_feature_warns_and_goes_anyway() {
    let c = confirm::complete_feature("x", FeatureOrHotfixKind::Hotfix, 0, 1, 0, false);
    assert_eq!(c.primary, "Concluir de qualquer forma");
    assert_eq!(c.facts[0].2, "Não tem painel aberto nesse hotfix");
    assert!(c.facts.last().unwrap().2.contains("o hotfix só existia aqui"));
}

#[test]
fn new_feature_and_task_titles() {
    assert_eq!(specs::new_feature(FeatureOrHotfixKind::Feature, "repo").title, "Nova feature em repo");
    assert_eq!(specs::new_feature(FeatureOrHotfixKind::Hotfix, "repo").title, "Novo hotfix em repo");
    let t = specs::new_task("repo");
    assert_eq!(t.fields[1].initial, "feature/TASK#{id}");
    assert_eq!(t.fields[2].initial, "TASK-{id}");
}

#[test]
fn launcher_name_defaults_to_first_word_of_command() {
    assert_eq!(specs::launcher_name("npm run dev", " "), "npm");
    assert_eq!(specs::launcher_name("npm run dev", "Servidor"), "Servidor");
}

#[test]
fn move_dialog_strings() {
    assert_eq!(none_label(true), "Nenhum — solto na bandeja");
    assert_eq!(none_label(false), "Nenhum — solto na pasta");
    assert!(no_features_banner(false).ends_with("crie pelo + da pasta"));
}
