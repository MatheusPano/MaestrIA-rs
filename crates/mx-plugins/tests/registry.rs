mod common;

use std::fs;

use common::{AURORA, aurora, plugins_in, write_plugin};
use mx_plugins::{MAX_SIDEBAR_COMMANDS, PluginState};
use mx_theme::{Color, Palette, Themes};
use serde_json::{Value, json};

#[test]
fn a_plugin_theme_reads_from_json_and_missing_brights_fall_back() {
    let p = Palette::from_json(&aurora()).unwrap();
    assert_eq!(p.id, "exemplo-aurora");
    assert_eq!(p.accent, Color::parse_hex("#5EE3C1").unwrap());
    assert_eq!(p.ansi.bright_black(), Color::parse_hex("#5F7385").unwrap());
    assert_eq!(p.ansi.bright_red(), p.ansi.red);
}

#[test]
fn a_theme_missing_a_field_names_it() {
    let mut a = aurora();
    a.as_object_mut().unwrap().remove("bgHover");
    assert!(Palette::from_json(&a).unwrap_err().to_string().contains("bgHover"));
    let mut b = aurora();
    b["ansi"]["cyan"] = json!("azul");
    assert!(Palette::from_json(&b).unwrap_err().to_string().contains("ansi.cyan"));
}

#[test]
fn by_id_finds_plugin_themes_and_falls_back_to_the_default() {
    let mut themes = Themes::new();
    themes.register(Palette::from_json(&aurora()).unwrap()).unwrap();
    assert_eq!(themes.by_id(Some("exemplo-aurora")).label, "Aurora");
    assert_eq!(themes.by_id(Some("nao-existe")).id, "maestria");
}

#[test]
fn the_scan_keeps_good_plugins_and_marks_broken_and_duplicate_ones_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("plugins");
    write_plugin(
        &root,
        "a-tema",
        json!({"id": "a.tema", "version": "1", "contributes": {"themes": ["t.json"]}}),
        &[("t.json", AURORA)],
    );
    write_plugin(&root, "b-quebrado", json!({"id": "b"}), &[]);
    write_plugin(&root, "c-repetido", json!({"id": "a.tema", "version": "2"}), &[]);
    fs::create_dir_all(root.join(".staging")).unwrap();
    let plugins = plugins_in(tmp.path());
    plugins.scan();
    let all = plugins.all();
    let states: Vec<PluginState> = all.iter().map(|p| p.state).collect();
    assert_eq!(states, [PluginState::Idle, PluginState::Invalid, PluginState::Invalid]);
    assert_eq!(all[1].problem.as_deref(), Some("\"version\" é obrigatório"));
    assert_eq!(all[2].problem.as_deref(), Some("outro plugin já usa o id \"a.tema\""));
    assert_eq!(all[2].id, "c-repetido");
    let extra: Vec<String> = plugins.themes().extra().iter().map(|t| t.id.clone()).collect();
    assert_eq!(extra, ["exemplo-aurora"]);
    assert_eq!(plugins.get("a.tema").unwrap().palettes.len(), 1);
}

#[tokio::test]
async fn disabling_a_plugin_drops_its_commands_and_themes_and_is_persisted() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        &tmp.path().join("plugins"),
        "p",
        json!({"id": "p", "version": "1", "contributes": {
            "themes": ["t.json"], "commands": [{"id": "ls", "title": "listar", "run": "ls"}]}}),
        &[("t.json", AURORA)],
    );
    let plugins = plugins_in(tmp.path());
    plugins.scan();
    assert_eq!(plugins.commands().len(), 1);
    plugins.set_enabled("p", false).await;
    assert!(plugins.commands().is_empty());
    assert!(plugins.themes().extra().is_empty());
    assert_eq!(plugins.to_json(), json!({"disabled": ["p"]}));
    let again = plugins_in(tmp.path());
    again.load(&plugins.to_json());
    again.scan();
    assert_eq!(again.get("p").unwrap().state, PluginState::Disabled);
}

#[test]
fn a_theme_with_a_built_in_id_is_logged_and_the_plugin_stays_loaded() {
    let tmp = tempfile::tempdir().unwrap();
    let mut nord = aurora();
    nord["id"] = json!("nord");
    write_plugin(
        &tmp.path().join("plugins"),
        "p",
        json!({"id": "p", "version": "1", "contributes": {"themes": ["t.json"]}}),
        &[("t.json", &nord.to_string())],
    );
    let plugins = plugins_in(tmp.path());
    plugins.scan();
    assert!(plugins.themes().extra().is_empty());
    let log = plugins.log_of("p");
    assert_eq!(log.len(), 1);
    assert!(log[0].contains("embutido"));
    assert!(log[0].ends_with("tema t.json: \"nord\" é o id de um tema embutido"));
    assert_eq!(plugins.get("p").unwrap().state, PluginState::Idle);
}

#[test]
fn log_lines_carry_a_clock_and_two_spaces() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(&tmp.path().join("plugins"), "p", json!({"id": "p", "version": "1"}), &[]);
    let plugins = plugins_in(tmp.path());
    plugins.scan();
    plugins.note("p", "oi");
    let line = &plugins.log_of("p")[0];
    assert_eq!(line.len(), "HH:MM:SS  oi".len());
    assert_eq!(&line[2..3], ":");
    assert_eq!(&line[8..], "  oi");
    for i in 0..450 {
        plugins.note("p", &format!("{i}"));
    }
    assert_eq!(plugins.log_of("p").len(), mx_plugins::LOG_CAP);
}

fn cfg_manifest() -> Value {
    json!({"id": "cfg", "version": "1", "contributes": {"settings": [
        {"id": "colors.app", "type": "color", "default": "", "title": "log do app", "group": "cores"},
        {"id": "colors.error", "type": "color", "default": "red", "title": "erro", "group": "cores"},
        {"id": "max", "type": "number", "default": 5000},
        {"id": "modo", "type": "select", "options": ["a", "b"]},
        {"id": "ligado", "type": "boolean"}
    ]}})
}

#[test]
fn only_changed_settings_are_persisted_and_null_resets_to_the_default() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(&tmp.path().join("plugins"), "cfg", cfg_manifest(), &[]);
    let plugins = plugins_in(tmp.path());
    plugins.scan();
    assert_eq!(plugins.settings_of("cfg")["colors.error"], json!("red"));
    assert_eq!(plugins.to_json(), json!({}));
    plugins.set_setting("cfg", "colors.app", Some(json!("#4FC1FF")));
    let saved = plugins.to_json();
    assert_eq!(saved.to_string(), r##"{"settings":{"cfg":{"colors.app":"#4FC1FF"}}}"##);
    let again = plugins_in(tmp.path());
    again.load(&saved);
    again.scan();
    assert_eq!(again.settings_of("cfg")["colors.app"], json!("#4FC1FF"));
    plugins.set_setting("cfg", "colors.app", None);
    assert_eq!(plugins.settings_of("cfg")["colors.app"], json!(""));
    assert_eq!(plugins.to_json(), json!({}));
}

#[test]
fn sidebar_commands_are_capped() {
    let tmp = tempfile::tempdir().unwrap();
    let mut commands: Vec<Value> = (0..5)
        .map(|i| json!({"id": format!("c{i}"), "title": format!("C{i}"), "run": "ls", "icon": "bug", "sidebar": true}))
        .collect();
    commands.push(json!({"id": "no", "title": "No", "run": "ls"}));
    write_plugin(
        &tmp.path().join("plugins"),
        "p",
        json!({"id": "p", "version": "1", "contributes": {"commands": commands}}),
        &[],
    );
    let plugins = plugins_in(tmp.path());
    plugins.scan();
    let all = plugins.commands();
    assert_eq!(all[0].icon.as_deref(), Some("bug"));
    assert!(!all.last().unwrap().sidebar);
    assert_eq!(plugins.sidebar_commands().len(), MAX_SIDEBAR_COMMANDS);
    assert_eq!(MAX_SIDEBAR_COMMANDS, 3);
}

#[tokio::test]
async fn installing_from_a_folder_stages_commits_updates_and_uninstalls() {
    let tmp = tempfile::tempdir().unwrap();
    let src = write_plugin(tmp.path(), "fonte", json!({"id": "meu.plugin", "version": "1.0.0"}), &[]);
    let plugins = plugins_in(tmp.path());
    let root = tmp.path().join("plugins");
    let staged = plugins.stage(src.to_str().unwrap()).await.unwrap();
    assert_eq!(staged.manifest.id, "meu.plugin");
    assert_eq!(staged.replacing, None);
    assert!(!root.join("meu.plugin").exists());
    let p = plugins.commit(&staged).await.unwrap();
    assert_eq!(p.dir, root.join("meu.plugin"));
    assert!(root.join("meu.plugin/maestria-plugin.json").exists());
    assert!(!staged.staging.exists());
    assert!(plugins.log_of("meu.plugin").last().unwrap().contains("instalado de "));

    fs::write(src.join("maestria-plugin.json"), json!({"id": "meu.plugin", "version": "1.1.0"}).to_string()).unwrap();
    let staged = plugins.stage(src.to_str().unwrap()).await.unwrap();
    assert_eq!(staged.replacing.as_deref(), Some("1.0.0"));
    plugins.commit(&staged).await.unwrap();
    let all = plugins.all();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].manifest.as_ref().unwrap().version, "1.1.0");

    plugins.uninstall("meu.plugin").await;
    assert!(plugins.all().is_empty());
    assert!(!root.join("meu.plugin").exists());
    assert!(src.join("maestria-plugin.json").exists());
}

#[tokio::test]
async fn a_package_folder_is_entered_ignoring_macosx_and_discard_removes_the_staging() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = tmp.path().join("pacote");
    write_plugin(&pack, "maestria-plugin-x-main", json!({"id": "x", "version": "1"}), &[]);
    fs::create_dir_all(pack.join("__MACOSX")).unwrap();
    let plugins = plugins_in(tmp.path());
    let staged = plugins.stage(pack.to_str().unwrap()).await.unwrap();
    assert!(staged.dir.ends_with("maestria-plugin-x-main"));
    plugins.discard(&staged);
    assert!(!staged.staging.exists());
}

#[tokio::test]
async fn staging_an_empty_or_missing_source_fails_and_leaves_no_staging() {
    let tmp = tempfile::tempdir().unwrap();
    let empty = tmp.path().join("vazia");
    fs::create_dir_all(&empty).unwrap();
    let plugins = plugins_in(tmp.path());
    let e = plugins.stage(empty.to_str().unwrap()).await.unwrap_err();
    assert_eq!(e.0, "Não achei um maestria-plugin.json ali");
    let missing = tmp.path().join("nada");
    let e = plugins.stage(missing.to_str().unwrap()).await.unwrap_err();
    assert_eq!(e.0, format!("\"{}\" não é uma URL de git, um .zip nem uma pasta que exista", missing.display()));
    assert_eq!(plugins.stage("  ").await.unwrap_err().0, "Diga de onde instalar");
    let staging = tmp.path().join("plugins/.staging");
    assert_eq!(fs::read_dir(staging).unwrap().count(), 0);
}

#[tokio::test]
async fn a_zip_is_unpacked_and_a_bad_git_url_reports_the_last_stderr_line() {
    let tmp = tempfile::tempdir().unwrap();
    let src = write_plugin(tmp.path(), "zipme", json!({"id": "z", "version": "1"}), &[]);
    let zip = tmp.path().join("p.zip");
    let ok = std::process::Command::new("zip")
        .args(["-qr", zip.to_str().unwrap(), "zipme"])
        .current_dir(tmp.path())
        .status();
    let plugins = plugins_in(tmp.path());
    if ok.is_ok_and(|s| s.success()) {
        let staged = plugins.stage(zip.to_str().unwrap()).await.unwrap();
        assert!(staged.dir.ends_with("zipme"));
        plugins.discard(&staged);
    }
    let _ = src;
    let bad = format!("{}/nao-existe.git", tmp.path().display());
    let e = plugins.stage(&bad).await.unwrap_err();
    assert!(!e.0.is_empty());
}

#[tokio::test]
async fn a_dev_link_is_a_symlink_that_uninstall_removes_without_touching_the_source() {
    let tmp = tempfile::tempdir().unwrap();
    let dev = write_plugin(tmp.path(), "dev", json!({"id": "dev.plugin", "version": "0.0.1"}), &[]);
    let plugins = plugins_in(tmp.path());
    let p = plugins.link(dev.to_str().unwrap()).await.unwrap();
    assert!(p.linked);
    let link = tmp.path().join("plugins/dev.plugin");
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    let again = plugins.link(dev.to_str().unwrap()).await.unwrap_err();
    assert_eq!(again.0, "Já existe um plugin \"dev.plugin\" — remova ele antes");
    plugins.uninstall("dev.plugin").await;
    assert!(fs::symlink_metadata(&link).is_err());
    assert!(dev.join("maestria-plugin.json").exists());
    let e = plugins.link(tmp.path().join("nada").to_str().unwrap()).await.unwrap_err();
    assert!(e.0.ends_with("não é uma pasta"));
}

#[test]
fn trust_texts_describe_what_the_plugin_does() {
    let m = mx_plugins::PluginManifest::parse_for(
        &common::obj(json!({"id": "x", "version": "2", "author": "ana", "main": ["node", "m.js"],
            "permissions": ["notifications"], "contributes": {"themes": ["a", "b"],
            "commands": [{"id": "a", "title": "Abrir", "key": "ctrl+alt+o"}, {"id": "b", "title": "B"}]}})),
        true,
    )
    .unwrap();
    assert_eq!(mx_plugins::registry::trust_headline(&m, Some("1")), "x 2 · de ana · no lugar da 1");
    assert_eq!(
        mx_plugins::registry::trust_facts(&m, true),
        [
            "Roda um programa seu, com as suas permissões: node m.js",
            "Pode mandar notificações do sistema",
            "Comandos: Abrir (Ctrl+Alt+O), B",
            "2 temas",
        ]
    );
    assert_eq!(mx_plugins::registry::installed_banner("X", "2", Some("1")), "X atualizado de 1 pra 2");
    assert_eq!(mx_plugins::registry::installed_banner("X", "2", None), "X 2 instalado");
}
