mod common;

use common::{base, fixtures, obj};
use mx_plugins::chord::{Chord, Key, KeyPress};
use mx_plugins::{CommandTarget, PluginManifest, SettingType};
use serde_json::{Value, json};

fn parse(v: Value) -> Result<PluginManifest, String> {
    PluginManifest::parse_for(&obj(v), true).map_err(|e| e.0)
}

fn parse_mac(v: Value) -> Result<PluginManifest, String> {
    PluginManifest::parse_for(&obj(v), false).map_err(|e| e.0)
}

fn error_of(v: Value) -> String {
    parse(v).expect_err("should be rejected")
}

#[test]
fn a_minimal_manifest_falls_back_to_the_id_for_the_name_and_has_no_process_nor_commands() {
    let m = parse(json!({"id": "meu.plugin", "version": "0.1.0"})).unwrap();
    assert_eq!(m.name, "meu.plugin");
    assert!(!m.has_process());
    assert!(m.commands.is_empty());
}

#[test]
fn bad_manifests_are_refused_with_the_verbatim_message() {
    assert_eq!(
        error_of(base(json!({"id": "Com Espaço"}))),
        "\"id\" só pode ter minúsculas, dígitos, ponto, hífen e sublinhado: \"Com Espaço\""
    );
    assert_eq!(
        error_of(base(json!({"permissions": ["root"]}))),
        "permissão desconhecida: \"root\" — as que existem são hooks, terminal.write, sessions.create, notifications"
    );
    assert_eq!(
        error_of(base(json!({"maestria": 99}))),
        "o plugin pede a versão 99 da API e esta MaestrIA só conhece até a 1"
    );
    assert_eq!(error_of(base(json!({"main": 3}))), "\"main\" é uma lista: [\"node\", \"main.js\"]");
    let cmd = |c: Value| base(json!({"contributes": {"commands": [c]}}));
    assert_eq!(
        error_of(cmd(json!({"id": "a", "title": "A"}))),
        "o comando \"a\" não tem \"run\" nem \"send\", e o plugin não tem \"main\" pra atendê-lo"
    );
    assert_eq!(
        error_of(cmd(json!({"id": "a", "title": "A", "run": "ls", "send": "oi"}))),
        "o comando \"a\" tem \"run\" e \"send\" — escolha um"
    );
    assert_eq!(
        error_of(cmd(json!({"id": "a", "title": "A", "run": "ls", "in": "nuvem"}))),
        "o comando \"a\" tem \"in\": \"nuvem\" — vale \"terminal\" ou \"background\""
    );
    assert_eq!(error_of(cmd(json!({"id": null, "title": "A"}))), "comando com \"id\" inválido: null");
    assert_eq!(error_of(cmd(json!({"id": "a b", "title": "A"}))), "comando com \"id\" inválido: \"a b\"");
    assert_eq!(error_of(cmd(json!({"id": "a", "title": " "}))), "o comando \"a\" precisa de \"title\"");
    assert_eq!(error_of(cmd(json!(3))), "cada comando é um objeto");
    assert_eq!(error_of(json!({"version": "1"})), "\"id\" é obrigatório");
    assert_eq!(error_of(json!({"id": "x"})), "\"version\" é obrigatório");
}

#[test]
fn version_is_checked_last_so_a_bad_command_wins() {
    let m = json!({"id": "x", "contributes": {"commands": [{"id": "a", "title": "A"}]}});
    assert!(error_of(m).contains("não tem \"main\""));
}

#[test]
fn a_main_string_becomes_a_list_and_commands_get_their_targets_and_full_ids() {
    let m = parse(base(json!({
        "main": "bin/plugin",
        "contributes": {"commands": [
            {"id": "a", "title": "A"},
            {"id": "b", "title": "B", "run": "ls", "in": "background"},
            {"id": "c", "title": "C", "send": "revise"},
            {"id": "d", "title": "D", "run": "htop"}
        ]}
    })))
    .unwrap();
    assert_eq!(m.main, Some(vec!["bin/plugin".to_owned()]));
    let targets: Vec<CommandTarget> = m.commands.iter().map(|c| c.target).collect();
    assert_eq!(
        targets,
        [CommandTarget::Plugin, CommandTarget::Background, CommandTarget::Session, CommandTarget::Terminal]
    );
    assert_eq!(m.commands[0].full_id(), "x/a");
}

fn keyed() -> Value {
    base(json!({"contributes": {"commands": [
        {"id": "q", "title": "Q", "run": "ls", "key": "meta+q"},
        {"id": "g", "title": "G", "run": "ls", "key": "meta+alt+g"},
        {"id": "b", "title": "B", "run": "ls", "key": "banana+z+"}
    ]}}))
}

#[test]
fn on_macos_a_reserved_key_and_an_unparseable_key_become_warnings_and_the_command_keeps_no_key() {
    let m = parse_mac(keyed()).unwrap();
    assert_eq!(m.commands[0].key, None);
    let g = m.commands[1].key.unwrap();
    assert_eq!((g.key, g.meta, g.alt, g.control, g.shift), (Key::Char('g'), true, true, false, false));
    assert_eq!(m.commands[2].key, None);
    assert_eq!(m.warnings.len(), 2);
    assert!(m.warnings[0].contains("⌘Q"));
    assert_eq!(m.warnings[0], "o comando \"q\" pede ⌘Q: o macOS encerra o app com ⌘Q — ficou sem");
    assert_eq!(m.warnings[1], "o comando \"b\" pede a tecla \"banana+z+\", que não dá pra ler — ficou sem");
}

#[test]
fn on_linux_meta_becomes_ctrl_shift_so_meta_q_is_a_valid_key() {
    let m = parse(keyed()).unwrap();
    let q = m.commands[0].key.unwrap();
    assert_eq!((q.control, q.shift, q.meta), (true, true, false));
    assert_eq!(q.label(true), "Ctrl+Shift+Q");
    assert_eq!(m.warnings.len(), 1);
}

#[test]
fn a_manifest_meta_becomes_ctrl_shift_on_linux() {
    let host = Chord::parse("meta+alt+g").unwrap().for_host(true);
    assert_eq!((host.control, host.shift, host.alt, host.meta), (true, true, true, false));
    assert_eq!(host.label(true), "Ctrl+Shift+Alt+G");
    let plain = Chord::parse("ctrl+alt+g").unwrap();
    assert_eq!(plain.for_host(true), plain);
}

#[test]
fn chord_rejections_explain_why() {
    let why = |s: &str, linux| Chord::parse(s).unwrap().for_host(linux).rejection(linux);
    assert_eq!(why("alt+1", true).unwrap(), "é a tecla da 1ª sessão da lateral");
    assert_eq!(why("meta+3", false).unwrap(), "é a tecla da 3ª sessão da lateral");
    assert_eq!(
        why("g", true).unwrap(),
        "precisa de Ctrl ou Alt — sem modificador a tecla sumiria do terminal"
    );
    assert_eq!(why("f5", true), None);
    assert_eq!(why("ctrl+g", true).unwrap(), "o terminal manda Ctrl+G pro processo como código de controle");
    assert_eq!(why("ctrl+shift+c", true).unwrap(), "o painel copia a seleção com Ctrl+Shift+C");
    assert_eq!(why("ctrl+alt+left", true).unwrap(), "o desktop troca de área de trabalho com Ctrl+Alt+setas");
    assert_eq!(Chord::parse("ctrl+hyper+g"), None);
    assert_eq!(Chord::parse("ctrl+plus").unwrap().key, Key::Char('+'));
    assert_eq!(Chord::parse("alt+space").unwrap().label(true), "Alt+Space");
}

fn dead_e(meta: bool, alt: bool) -> KeyPress {
    KeyPress { key: Key::Char('´'), physical: Some(0x08), meta, control: false, alt, shift: false }
}

#[test]
fn a_dead_key_matches_the_chord_by_physical_position_when_modifiers_match() {
    let chord = Chord { key: Key::Char('e'), meta: true, alt: true, control: false, shift: false };
    assert!(!chord.accepts(&dead_e(true, true)));
    assert!(chord.hits(&dead_e(true, true)));
}

#[test]
fn the_physical_fallback_needs_exactly_the_same_modifiers() {
    let chord = Chord { key: Key::Char('e'), meta: true, alt: true, control: false, shift: false };
    assert!(!chord.hits(&dead_e(true, false)));
}

#[test]
fn activation_rules_follow_the_declared_events() {
    let none = parse(base(json!({"main": ["node", "m.js"]}))).unwrap();
    assert!(none.activates_on("onStartup"));
    assert!(!none.activates_on("onHook:Stop"));
    let hook = parse(base(json!({"main": ["node", "m.js"], "activationEvents": ["onHook:Stop"]}))).unwrap();
    assert!(!hook.activates_on("onStartup"));
    assert!(hook.activates_on("onHook:Stop"));
    let star = parse(base(json!({"main": ["node"], "activationEvents": ["*"]}))).unwrap();
    assert!(star.activates_on("onSession"));
    let no_main = parse(base(json!({}))).unwrap();
    assert!(!no_main.activates_on("onStartup"));
}

#[test]
fn the_example_manifests_read_from_disk_without_warnings() {
    for name in ["tema-e-atalhos", "exemplo-completo"] {
        let m = PluginManifest::read(&fixtures().join(name)).unwrap();
        assert!(m.warnings.is_empty(), "{name}: {:?}", m.warnings);
    }
}

#[test]
fn reading_a_folder_reports_missing_bad_json_and_non_objects() {
    let tmp = tempfile::tempdir().unwrap();
    let e = PluginManifest::read(tmp.path()).unwrap_err().0;
    assert_eq!(e, "não tem maestria-plugin.json");
    std::fs::write(tmp.path().join("maestria-plugin.json"), "{oops").unwrap();
    let e = PluginManifest::read(tmp.path()).unwrap_err().0;
    assert!(e.starts_with("maestria-plugin.json não é um json válido: "));
    std::fs::write(tmp.path().join("maestria-plugin.json"), "[1]").unwrap();
    let e = PluginManifest::read(tmp.path()).unwrap_err().0;
    assert_eq!(e, "maestria-plugin.json precisa ser um objeto");
}

#[test]
fn unknown_contributes_keys_are_warnings_and_themes_may_be_plain_strings() {
    let m = parse(base(json!({"contributes": {"themes": ["t.json", {"path": "u.json"}], "menus": []}}))).unwrap();
    assert_eq!(m.themes, ["t.json", "u.json"]);
    assert_eq!(m.warnings, ["\"contributes.menus\" não existe nesta versão — ignorado"]);
    assert_eq!(error_of(base(json!({"contributes": {"themes": [3]}}))), "cada tema é {\"path\": \"themes/x.json\"}");
}

#[test]
fn the_sidebar_contribution_needs_a_bool_and_a_main() {
    let m = parse(base(json!({"main": ["node", "main.js"], "contributes": {"sidebar": true}}))).unwrap();
    assert!(m.sidebar);
    assert!(m.warnings.is_empty());
    assert_eq!(
        error_of(base(json!({"contributes": {"sidebar": true}}))),
        "\"contributes.sidebar\" precisa de \"main\" pra desenhar a aba"
    );
    assert_eq!(
        error_of(base(json!({"main": ["x"], "contributes": {"sidebar": "sim"}}))),
        "\"contributes.sidebar\" é true ou false"
    );
}

fn board(screen: Value) -> Value {
    base(json!({"main": ["true"], "contributes": {"screen": screen, "commands": [{"id": "quadro", "title": "wiboor: o quadro"}]}}))
}

#[test]
fn the_screen_is_true_or_an_object_with_a_declared_home() {
    let t = parse(board(json!(true))).unwrap();
    assert!(t.screen);
    assert_eq!(t.screen_home, None);
    let h = parse(board(json!({"home": "quadro"}))).unwrap();
    assert!(h.screen);
    assert_eq!(h.screen_home.as_deref(), Some("quadro"));
    assert!(!parse(board(Value::Null)).unwrap().screen);
}

#[test]
fn bad_screens_are_refused() {
    assert_eq!(
        error_of(board(json!({"home": "outro"}))),
        "\"contributes.screen.home\" aponta pro comando \"outro\", que o manifesto não declara"
    );
    assert_eq!(
        error_of(board(json!("sim"))),
        "\"contributes.screen\" é true ou um objeto com \"home\": {\"home\": \"quadro\"}"
    );
    assert_eq!(
        error_of(board(json!({}))),
        "\"contributes.screen\" é true ou um objeto com \"home\": {\"home\": \"quadro\"}"
    );
    let no_main = base(json!({"contributes": {"screen": true, "commands": [{"id": "q", "title": "Q", "run": "true"}]}}));
    assert_eq!(error_of(no_main), "\"contributes.screen\" precisa de \"main\" pra abrir as janelas");
}

fn tracker(suggestions: Value) -> Value {
    json!({"id": "tracker", "version": "1", "main": ["./plugin.sh"], "permissions": ["sessions.create"],
        "contributes": {"suggestions": suggestions}})
}

#[test]
fn the_suggestions_label_is_read_trimmed() {
    let m = parse(tracker(json!({"label": "criar a task no Wiboor"}))).unwrap();
    assert_eq!(m.suggestions.as_deref(), Some("criar a task no Wiboor"));
    assert!(m.warnings.is_empty());
    assert_eq!(parse(tracker(Value::Null)).unwrap().suggestions, None);
}

#[test]
fn suggestions_need_a_main_and_a_label() {
    let mut no_main = tracker(json!({"label": "x"}));
    no_main["main"] = Value::Null;
    assert_eq!(
        error_of(no_main),
        "\"contributes.suggestions\" precisa de \"main\": quem recebe a sugestão é o processo"
    );
    let shape = "\"contributes.suggestions\" é um objeto com \"label\": {\"label\": \"criar a task no …\"}";
    assert_eq!(error_of(tracker(json!(true))), shape);
    assert_eq!(error_of(tracker(json!({"label": " "}))), shape);
}

fn settings() -> Value {
    base(json!({"contributes": {"settings": [
        {"id": "colors.app", "type": "color", "default": "", "title": "log do app", "group": "cores"},
        {"id": "colors.error", "type": "color", "default": "red", "title": "erro", "group": "cores"},
        {"id": "max", "type": "number", "default": 5000},
        {"id": "modo", "type": "select", "options": ["a", "b"]},
        {"id": "ligado", "type": "boolean"}
    ]}}))
}

#[test]
fn settings_resolve_to_saved_then_default_then_type_empty() {
    let m = parse(settings()).unwrap();
    let kinds: Vec<SettingType> = m.settings.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        [SettingType::Color, SettingType::Color, SettingType::Number, SettingType::Select, SettingType::Boolean]
    );
    let modo = &m.settings[3];
    assert_eq!(modo.resolve(None), json!("a"));
    assert_eq!(modo.resolve(Some(&json!("b"))), json!("b"));
    assert_eq!(modo.resolve(Some(&json!("z"))), json!("a"));
    assert_eq!(m.settings[2].resolve(Some(&json!("muito"))), json!(5000));
    assert_eq!(m.settings[4].resolve(None), json!(false));
    assert_eq!(m.settings[0].group.as_deref(), Some("cores"));
}

#[test]
fn bad_settings_are_refused() {
    let one = |s: Value| base(json!({"contributes": {"settings": [s]}}));
    assert_eq!(
        error_of(one(json!({"id": "s", "type": "select"}))),
        "a configuração \"s\" é select e não tem \"options\""
    );
    assert_eq!(
        error_of(one(json!({"id": "s", "type": "cor"}))),
        "a configuração \"s\" tem \"type\": \"cor\" — vale string, number, boolean, color, select"
    );
    assert_eq!(error_of(one(json!({"type": "string"}))), "cada configuração precisa de \"id\"");
    assert_eq!(error_of(one(json!(1))), "cada configuração é um objeto");
    assert_eq!(
        error_of(one(json!({"id": "s", "type": "select", "options": [1]}))),
        "a opção de \"s\" é um texto ou {\"value\", \"label\"}"
    );
    let twice = base(json!({"contributes": {"settings": [{"id": "a"}, {"id": "a"}]}}));
    assert_eq!(error_of(twice), "configuração repetida: \"a\"");
}

#[test]
fn author_may_be_a_string_or_an_object_with_name() {
    assert_eq!(parse(base(json!({"author": "ana"}))).unwrap().author, "ana");
    assert_eq!(parse(base(json!({"author": {"name": "bia"}}))).unwrap().author, "bia");
    assert_eq!(parse(base(json!({"author": 3}))).unwrap().author, "");
}

#[test]
fn short_titles_drop_the_plugin_name_prefix() {
    let m = parse(base(json!({"name": "Flutter", "contributes": {"commands": [
        {"id": "reload", "title": "flutter: hot reload", "run": "x"},
        {"id": "outro", "title": "git: outro prefixo", "run": "x"}
    ]}})))
    .unwrap();
    assert_eq!(m.commands[0].short_title(&m.name), "hot reload");
    assert_eq!(m.commands[1].short_title(&m.name), "git: outro prefixo");
}
