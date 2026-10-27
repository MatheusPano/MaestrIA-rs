use serde_json::{Value, json};

use super::Rig;

fn load(rig: &mut Rig, config: Value) {
    let state = rig.s.opts.state_dir.clone();
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("config.json"), config.to_string()).unwrap();
    rig.s.load();
}

fn with_plugins(rig: &mut Rig) {
    let root = rig.mkdir("plugins");
    let data = rig.mkdir("plugin-data");
    rig.s.attach_plugins(mx_plugins::Plugins::with_dirs(root.into(), data.into()));
}

#[test]
fn the_app_auto_update_toggle_is_read_from_the_config() {
    let mut rig = Rig::new();
    load(&mut rig, json!({"folders": []}));
    assert!(!rig.s.app_updater().active());
    load(&mut rig, json!({"folders": [], "autoUpdate": true}));
    assert!(rig.s.app_updater().active());
}

#[test]
fn the_app_auto_update_toggle_is_saved_only_when_on() {
    let mut rig = Rig::new();
    rig.s.set_auto_update(true);
    assert_eq!(rig.written()["autoUpdate"], json!(true));
    rig.s.set_auto_update(false);
    assert!(rig.written().get("autoUpdate").is_none());
}

#[test]
fn this_build_never_enables_the_app_updater() {
    let mut rig = Rig::new();
    rig.s.set_auto_update(true);
    assert!(!rig.s.app_updater().enabled());
    assert!(!rig.s.app_updater().shows_chip());
    assert!(rig.s.install_update().is_none());
}

#[test]
fn the_plugin_auto_update_switch_writes_false_inside_plugins() {
    let mut rig = Rig::new();
    with_plugins(&mut rig);
    rig.s.set_plugin_auto_update(false);
    assert_eq!(rig.written()["plugins"]["autoUpdate"], json!(false));
    rig.s.set_plugin_manual_update("mx.git", true);
    assert_eq!(rig.written()["plugins"]["manualUpdate"], json!(["mx.git"]));
    assert!(rig.written().get("autoUpdate").is_none());
}

#[test]
fn the_catalog_list_is_the_official_one_unless_overridden() {
    let urls = crate::catalog_urls();
    match std::env::var(crate::CATALOG_ENV) {
        Ok(custom) if !custom.trim().is_empty() => assert_eq!(urls, [custom]),
        _ => assert_eq!(urls, [mx_plugins::catalog::OFFICIAL_CATALOG]),
    }
}

#[test]
fn plugin_updates_attach_to_the_registry_without_starting() {
    let mut rig = Rig::new();
    assert!(rig.s.attach_plugin_updates(vec!["http://127.0.0.1:9/catalog.json".into()]).is_none());
    with_plugins(&mut rig);
    let u = rig.s.attach_plugin_updates(vec!["http://127.0.0.1:9/catalog.json".into()]).unwrap();
    assert_eq!(u.catalogs(), ["http://127.0.0.1:9/catalog.json"]);
    assert!(rig.s.plugin_updates().is_some());
    assert!(u.checked_at().is_none());
}
