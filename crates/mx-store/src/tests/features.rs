use mx_core::model::{FeatureOrHotfix, FeatureOrHotfixKind, TabKind};

use super::Rig;

fn s1() -> Rig {
    let mut rig = Rig::new();
    rig.s1();
    rig
}

fn add(rig: &mut Rig, root: &str, name: &str) -> String {
    rig.s.add_feature(root, name, "", FeatureOrHotfixKind::Feature)
}

#[test]
fn f1_feature_round_trips() {
    let mut rig = s1();
    let id = rig.s.add_feature("/repo", "permissão do google", "contexto", FeatureOrHotfixKind::Feature);
    let f = rig.s.feature_by_id(&id).unwrap();
    let back = FeatureOrHotfix::from_json(&f.to_json()).unwrap();
    assert_eq!((&back.id, &back.name, &back.brief, &back.folder_root), (&f.id, &f.name, &f.brief, &f.folder_root));
}

#[test]
fn f2_assign_refuses_a_feature_of_another_folder() {
    let mut rig = s1();
    rig.folder("/outro", "outro");
    let mine = add(&mut rig, "/repo", "minha");
    let theirs = add(&mut rig, "/outro", "deles");
    rig.panel("t");
    rig.s.assign("t", Some(&mine));
    assert_eq!(rig.t("t").feature_or_hotfix_id.as_deref(), Some(mine.as_str()));
    rig.s.assign("t", None);
    rig.s.assign("t", Some(&theirs));
    assert!(rig.t("t").feature_or_hotfix_id.is_none());
}

#[test]
fn f3_dissolving_keeps_the_panels() {
    let mut rig = s1();
    let f = add(&mut rig, "/repo", "x");
    rig.panel("t");
    rig.s.assign("t", Some(&f));
    rig.s.remove_feature(&f);
    assert_eq!(rig.s.tabs().len(), 1);
    assert!(rig.t("t").feature_or_hotfix_id.is_none());
    assert!(rig.s.features().is_empty());
}

#[test]
fn f4_completing_closes_its_panels_only() {
    let mut rig = s1();
    let f = add(&mut rig, "/repo", "x");
    for id in ["a", "b"] {
        rig.panel(id);
        rig.s.assign(id, Some(&f));
    }
    let home = rig.s.loose().root.clone();
    rig.add_tab("solta", &home, TabKind::Shell, &home, None);
    assert_eq!(rig.s.complete_feature(&f), 2);
    assert_eq!(rig.ids(), ["solta"]);
    assert!(rig.s.features().is_empty());
}

#[test]
fn f5_completing_an_empty_feature_returns_zero() {
    let mut rig = s1();
    let f = add(&mut rig, "/repo", "x");
    assert_eq!(rig.s.complete_feature(&f), 0);
    assert!(rig.s.features().is_empty());
}

#[test]
fn f6_removing_the_folder_drops_its_features() {
    let mut rig = s1();
    add(&mut rig, "/repo", "x");
    rig.s.remove_folder("/repo");
    assert!(rig.s.features().is_empty());
}

#[test]
fn f7_move_tab_takes_the_target_feature() {
    let mut rig = s1();
    let f = add(&mut rig, "/repo", "x");
    for id in ["inside", "outside", "plain"] {
        rig.panel(id);
    }
    rig.s.assign("inside", Some(&f));
    rig.s.move_tab("outside", "inside");
    assert_eq!(rig.t("outside").feature_or_hotfix_id.as_deref(), Some(f.as_str()));
    rig.s.move_tab("outside", "plain");
    assert!(rig.t("outside").feature_or_hotfix_id.is_none());
}

#[test]
fn f8_f9_f10_loose_features_belong_to_the_tray() {
    let mut rig = s1();
    let home = rig.s.loose().root.clone();
    let f = add(&mut rig, &home, "solta");
    assert_eq!(rig.s.feature_by_id(&f).unwrap().folder_root, home);
    assert_eq!(rig.s.features_of(&home).len(), 1);
    assert!(rig.s.features_of("/repo").is_empty());
    rig.add_tab("l", &home, TabKind::Shell, &home, None);
    rig.tm("l").loose = true;
    rig.s.assign("l", Some(&f));
    assert_eq!(rig.t("l").feature_or_hotfix_id.as_deref(), Some(f.as_str()));
    rig.panel("p");
    rig.s.assign("p", Some(&f));
    assert!(rig.t("p").feature_or_hotfix_id.is_none());
    let back = FeatureOrHotfix::from_json(&rig.s.feature_by_id(&f).unwrap().to_json()).unwrap();
    assert_eq!((back.folder_root.as_str(), back.name.as_str()), (home.as_str(), "solta"));
}

#[test]
fn f13_f14_kind_round_trips_and_flips() {
    let mut rig = s1();
    let f = rig.s.add_feature("/repo", "quebrou", "", FeatureOrHotfixKind::Hotfix);
    let j = rig.s.feature_by_id(&f).unwrap().to_json();
    assert_eq!(j["kind"], "hotfix");
    assert_eq!(FeatureOrHotfix::from_json(&j).unwrap().kind, FeatureOrHotfixKind::Hotfix);
    rig.s.set_feature_kind(&f, FeatureOrHotfixKind::Feature);
    assert_eq!(rig.s.feature_by_id(&f).unwrap().kind, FeatureOrHotfixKind::Feature);
    rig.s.set_feature_kind(&f, FeatureOrHotfixKind::Hotfix);
    assert_eq!(rig.s.feature_by_id(&f).unwrap().kind, FeatureOrHotfixKind::Hotfix);
}

#[test]
fn edit_feature_trims_name_and_keeps_brief_verbatim() {
    let mut rig = s1();
    let f = add(&mut rig, "/repo", "x");
    rig.s.edit_feature_text(&f, Some("  novo  "), Some(" b "));
    rig.s.edit_feature_text(&f, Some("   "), None);
    let x = rig.s.feature_by_id(&f).unwrap();
    assert_eq!((x.name.as_str(), x.brief.as_str()), ("novo", " b "));
}
