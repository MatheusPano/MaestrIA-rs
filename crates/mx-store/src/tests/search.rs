use mx_core::model::{ClaudeStatus, FeatureOrHotfixKind, Filter, TabKind};

use super::Rig;

fn hits(rig: &Rig) -> Vec<String> {
    rig.s.hits().iter().map(|t| t.id.clone()).collect()
}

#[test]
fn qs1_query_matches_the_title() {
    let mut rig = Rig::new();
    rig.s1();
    rig.panel("permissão do drive");
    rig.panel("migração do banco");
    rig.s.set_query("drive");
    assert_eq!(hits(&rig), ["permissão do drive"]);
}

#[test]
fn qs2_qs3_haystack_has_folder_feature_branch_and_every_term() {
    let mut rig = Rig::new();
    rig.s1();
    rig.panel("um");
    let f = rig.s.add_feature("/repo", "permissão do google", "", FeatureOrHotfixKind::Feature);
    rig.s.assign("um", Some(&f));
    rig.tm("um").branch = "feature/TASK#47730".into();
    for (q, hit) in [
        ("meu-repo", true),
        ("google", true),
        ("TASK#47730", true),
        ("shell", false),
        ("google perm", true),
        ("google banco", false),
    ] {
        rig.s.set_query(q);
        assert_eq!(rig.s.matches(rig.t("um")), hit, "{q}");
    }
}

#[test]
fn qs4_qs5_folder_and_feature_filters_or_together() {
    let mut rig = Rig::new();
    rig.s1();
    rig.folder("/outro", "outro");
    rig.add_tab("here", "/repo", TabKind::Shell, "/repo", None);
    rig.add_tab("there", "/outro", TabKind::Shell, "/outro", None);
    rig.add_tab("neither", "/outro", TabKind::Shell, "/outro", None);
    rig.s.toggle_filter_folder("/repo");
    assert_eq!(hits(&rig), ["here"]);
    let f = rig.s.add_feature("/outro", "x", "", FeatureOrHotfixKind::Feature);
    rig.s.assign("there", Some(&f));
    rig.s.toggle_filter_feature(&f);
    assert_eq!(hits(&rig), ["here", "there"]);
}

#[test]
fn qs6_qs7_flags_or_within_a_group_and_across_groups() {
    let mut rig = Rig::new();
    rig.s1();
    rig.panel("waiting");
    rig.tm("waiting").hooks.status = ClaudeStatus::WaitingPermission;
    rig.panel("working");
    rig.tm("working").hooks.status = ClaudeStatus::Working;
    rig.shell("shell");
    rig.s.toggle_filter(Filter::Claude);
    rig.s.toggle_filter(Filter::Waiting);
    assert_eq!(hits(&rig), ["waiting"]);
    rig.s.toggle_filter(Filter::Working);
    assert_eq!(hits(&rig), ["waiting", "working"]);
    rig.s.set_done("waiting", true);
    rig.s.toggle_filter(Filter::Working);
    assert!(hits(&rig).is_empty());
    rig.s.toggle_filter(Filter::Waiting);
    rig.s.toggle_filter(Filter::Finished);
    assert_eq!(hits(&rig), ["waiting"]);
}

#[test]
fn qs8_clear_search_resets_everything() {
    let mut rig = Rig::new();
    rig.s1();
    rig.panel("um");
    rig.s.set_query("um");
    rig.s.toggle_filter(Filter::Shell);
    assert!(hits(&rig).is_empty());
    rig.s.clear_search();
    assert!(!rig.s.filtering());
    assert_eq!((rig.s.search().query.as_str(), rig.s.active_filters()), ("", 0));
    assert_eq!(hits(&rig).len(), 1);
}
