mod common;

use std::path::PathBuf;
use std::sync::Arc;

use chrono::NaiveDate;
use common::{obj, write_plugin};
use mx_core::model::{SuggestedTask, SuggestionChoice};
use mx_plugins::commands::{CommandRun, CommandValues, background_banner, expand_values, plan};
use mx_plugins::rail::{RailBadge, footer_commands, move_rail_plugin, rail_badge, rail_plugins, rail_tooltip, sidebar_placeholder};
use mx_plugins::suggestions::{SuggestionOrigin, SuggestionTracker, Tracking, start_params, suggestion_prompt, worktree_branch};
use mx_plugins::ui::calendar::{Calendar, CalendarBlock, value_of};
use mx_plugins::ui::dialogs::{PickRequest, cycle_highlight};
use mx_plugins::ui::form::{FormPress, PluginForm};
use mx_plugins::ui::icons::{Glyph, glyph, plugin_color, svg_of};
use mx_plugins::{PluginInfo, PluginManifest, PluginState};
use serde_json::json;

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

#[test]
fn a_range_calendar_opens_with_one_day_and_closes_in_either_order() {
    let max = d("2026-09-23");
    let mut cal = Calendar::new(true, Some(max), None, Some(max), d("2026-10-21"));
    assert_eq!(cal.header(), "Setembro de 2026");
    assert_eq!(cal.tap(d("2026-09-19")), Some((d("2026-09-19"), d("2026-09-19"))));
    assert_eq!(cal.summary(Some(d("2026-09-19")), Some(d("2026-09-19"))), "Começa em 19/09 — clique no último dia");
    assert_eq!(cal.tap(d("2026-09-15")), Some((d("2026-09-15"), d("2026-09-19"))));
    assert_eq!(cal.summary(Some(d("2026-09-15")), Some(d("2026-09-19"))), "15/09 a 19/09 · 5 dias");
    assert_eq!(value_of(true, d("2026-09-15"), d("2026-09-19")).to_string(), r#"{"from":"2026-09-15","to":"2026-09-19"}"#);
}

#[test]
fn a_single_calendar_ignores_days_after_max_and_cannot_go_past_its_month() {
    let max = d("2026-09-23");
    let mut cal = Calendar::new(false, Some(max), None, Some(max), d("2026-10-21"));
    assert_eq!(cal.tap(d("2026-09-28")), None);
    cal.next();
    assert_eq!(cal.header(), "Setembro de 2026");
    cal.prev();
    assert_eq!(cal.header(), "Agosto de 2026");
    assert_eq!(cal.tap(d("2026-08-25")), Some((d("2026-08-25"), d("2026-08-25"))));
    assert_eq!(cal.summary(Some(d("2026-08-25")), Some(d("2026-08-25"))), "Terça, 25 de agosto");
    assert_eq!(cal.summary(None, None), "Escolha um dia");
}

#[test]
fn calendar_bounds_accept_today_and_presets_default_their_label() {
    let b = CalendarBlock::parse(
        "p".into(),
        &obj(json!({"mode": "range", "max": "today", "presets": [{"from": "2026-09-01"}, {"label": "x", "from": "bad"}]})),
    );
    assert!(b.range);
    assert_eq!(b.max_day(d("2026-10-21")), Some(d("2026-10-21")));
    assert_eq!(b.presets.len(), 1);
    assert_eq!(b.presets[0].label, "2026-09-01");
    assert_eq!(b.presets[0].to, d("2026-09-01"));
}

fn ssh_form() -> PluginForm {
    PluginForm::parse(
        &obj(json!({"title": "novo host", "fields": [
            {"id": "name", "label": "nome"},
            {"id": "host", "label": "host", "required": true},
            {"id": "user", "label": "usuário", "half": true},
            {"id": "port", "label": "porta", "half": true, "value": 22},
            {"id": "group", "label": "grupo", "type": "select", "value": "AWS",
                "options": [{"value": "", "label": "sem grupo"}, "AWS", {"value": "::novo", "label": "novo grupo…"}]},
            {"id": "newGroup", "label": "nome do novo grupo", "required": true, "showIf": {"field": "group", "value": "::novo"}}],
            "buttons": [{"action": "apagar", "style": "danger"}, {"action": "conectar", "label": "salvar e conectar"},
                {"action": "salvar", "style": "primary"}], "error": " a porta é um número de 1 a 65535 "})),
        "SSH",
    )
    .unwrap()
}

fn set(values: &mut [(String, String)], id: &str, v: &str) {
    values.iter_mut().find(|(k, _)| k == id).unwrap().1 = v.into();
}

#[test]
fn a_required_field_blocks_non_danger_buttons_and_enter_presses_the_primary() {
    let form = ssh_form();
    let mut values = form.initial_values();
    let rows: Vec<usize> = form.rows(&values).iter().map(Vec::len).collect();
    assert_eq!(rows, [1, 1, 2, 1]);
    let salvar = form.enter_button().unwrap().clone();
    assert_eq!(salvar.action, "salvar");
    assert_eq!(form.press(&salvar, &values), FormPress::Missing { field: "host".into(), text: "falta host".into() });
    set(&mut values, "host", "exemplo.com");
    let FormPress::Submit(answer) = form.press(&salvar, &values) else { panic!() };
    assert_eq!(
        answer.to_json().to_string(),
        r#"{"action":"salvar","values":{"name":"","host":"exemplo.com","user":"","port":"22","group":"AWS","newGroup":""}}"#
    );
    assert_eq!(form.error.as_deref(), Some("a porta é um número de 1 a 65535"));
}

#[test]
fn show_if_reveals_a_field_that_then_becomes_required() {
    let form = ssh_form();
    let mut values = form.initial_values();
    set(&mut values, "host", "h");
    set(&mut values, "group", "::novo");
    assert_eq!(form.rows(&values).len(), 5);
    let conectar = form.buttons[1].clone();
    let FormPress::Missing { text, .. } = form.press(&conectar, &values) else { panic!() };
    assert_eq!(text, "falta nome do novo grupo");
    set(&mut values, "newGroup", "Clientes");
    let FormPress::Submit(a) = form.press(&conectar, &values) else { panic!() };
    assert_eq!(a.action, "conectar");
    let apagar = form.buttons[0].clone();
    let empty = form.initial_values();
    assert!(matches!(form.press(&apagar, &empty), FormPress::Submit(_)));
}

#[test]
fn form_parse_errors_are_verbatim() {
    let e = |v: serde_json::Value| PluginForm::parse(&obj(v), "x").unwrap_err();
    assert_eq!(e(json!({"fields": [1]})), "cada campo é um objeto");
    assert_eq!(e(json!({"fields": [{}]})), "campo sem \"id\"");
    assert_eq!(e(json!({"fields": [{"id": "a"}, {"id": "a"}]})), "campo repetido: \"a\"");
    let ok = PluginForm::parse(&obj(json!({"fields": [{"id": "a", "type": "select"}]})), "Fallback").unwrap();
    assert_eq!(ok.title, "Fallback");
    assert_eq!(ok.buttons[0].label, "ok");
    assert_eq!(ok.initial_values(), [("a".to_owned(), String::new())]);
}

#[test]
fn quick_pick_filters_by_every_term_and_arrows_cycle() {
    let pick = PickRequest::parse(
        &obj(json!({"title": "projeto", "items": [{"value": "/x/app-a", "label": "app-a"},
            {"value": "/x/app-b", "label": "app-b"}, {"value": "/y/outro", "label": "outro"}]})),
        "P",
    );
    let shown = pick.filter("app");
    assert_eq!(shown.len(), 2);
    let at = cycle_highlight(0, 1, shown.len());
    assert_eq!(shown[at].value, "/x/app-b");
    assert_eq!(cycle_highlight(0, -1, 3), 2);
    assert_eq!(pick.filter("APP b").len(), 1);
    assert_eq!(pick.hint(), "Filtrar");
}

#[test]
fn declarative_commands_quote_shell_values_and_leave_unknown_vars() {
    let values = CommandValues { cwd: "/repo/it's".into(), title: "x $(rm -rf ~)".into(), ..CommandValues::default() };
    assert_eq!(
        expand_values("cd ${cwd} && echo ${title} ${nada}", &values, true),
        r#"cd '/repo/it'\''s' && echo 'x $(rm -rf ~)' ${nada}"#
    );
    assert_eq!(expand_values("revise ${cwd}", &values, false), "revise /repo/it's");
}

#[test]
fn command_plans_follow_the_target() {
    let m = PluginManifest::parse_for(
        &obj(json!({"id": "x", "version": "1", "main": ["m"], "contributes": {"commands": [
            {"id": "a", "title": "A"}, {"id": "b", "title": "B", "run": "ls ${cwd}", "in": "background"},
            {"id": "c", "title": "C", "send": "revise ${cwd}"}]}})),
        true,
    )
    .unwrap();
    let values = CommandValues { cwd: "/r".into(), folder: "/r".into(), ..CommandValues::default() };
    let CommandRun::Plugin { context } = plan(&m.commands[0], &values, Some("tab1")) else { panic!() };
    assert_eq!(
        serde_json::Value::Object(context).to_string(),
        r#"{"cwd":"/r","folder":"/r","sessionId":"","title":"","pluginDir":"","tabId":"tab1"}"#
    );
    assert_eq!(plan(&m.commands[1], &values, None), CommandRun::Background { command: "ls '/r'".into() });
    assert_eq!(plan(&m.commands[2], &values, None), CommandRun::Session { text: "revise /r".into() });
    assert_eq!(background_banner("st", 0, "a\nb\n\n", ""), ("st: b".into(), false));
    assert_eq!(background_banner("st", 0, "", ""), ("st: feito".into(), false));
    assert_eq!(background_banner("st", 2, "out", "boom\n"), ("st falhou (código 2): boom".into(), true));
    assert_eq!(background_banner("st", 2, "", ""), ("st falhou (código 2)".into(), true));
}

fn fake(id: &str, manifest: serde_json::Value) -> PluginInfo {
    let m = PluginManifest::parse_for(&obj(manifest), true).unwrap();
    PluginInfo {
        id: id.into(),
        dir: PathBuf::from("/p"),
        data_dir: PathBuf::from("/d"),
        manifest: Some(Arc::new(m)),
        problem: None,
        linked: false,
        enabled: true,
        crash: None,
        state: PluginState::Idle,
        palettes: Vec::new(),
    }
}

#[test]
fn the_rail_needs_an_icon_and_commands_and_footer_buttons_belong_to_icon_less_plugins() {
    let ssh = fake("ssh", json!({"id": "ssh", "name": "SSH", "version": "1", "icon": "terminal", "main": ["x"],
        "contributes": {"commands": [{"id": "hosts", "title": "hosts", "sidebar": true}]}}));
    let rel = fake("relatorio", json!({"id": "relatorio", "version": "1", "main": ["x"],
        "contributes": {"commands": [{"id": "dia", "title": "relatório", "sidebar": true}]}}));
    let all = vec![ssh.clone(), rel.clone()];
    let rail = rail_plugins(&all, &[], |_| false);
    assert_eq!(rail, ["ssh"]);
    let commands: Vec<_> = all.iter().flat_map(|p| p.manifest.as_ref().unwrap().commands.clone()).collect();
    let footer: Vec<String> = footer_commands(&commands, &rail).iter().map(|c| c.full_id()).collect();
    assert_eq!(footer, ["relatorio/dia"]);
    let owner = rail_plugins(&all, &[], |id| id == "relatorio");
    assert_eq!(owner, ["ssh", "relatorio"]);
    let mut off = ssh.clone();
    off.enabled = false;
    assert!(rail_plugins(&[off], &[], |_| false).is_empty());
}

#[test]
fn rail_order_survives_a_disabled_plugin() {
    let icon = |id: &str| {
        fake(id, json!({"id": id, "version": "1", "icon": "terminal", "contributes": {"commands": [{"id": "c", "title": "c", "run": "x"}]}}))
    };
    let all = vec![icon("a"), icon("b"), icon("c")];
    let rail = rail_plugins(&all, &[], |_| false);
    let order = move_rail_plugin(&rail, &[], "a", 1);
    assert_eq!(order, ["b", "a", "c"]);
    let without_a = vec![icon("b"), icon("c")];
    let rail = rail_plugins(&without_a, &order, |_| false);
    assert_eq!(rail, ["b", "c"]);
    let order = move_rail_plugin(&rail, &order, "c", 0);
    assert_eq!(order, ["c", "b", "a"]);
    assert_eq!(rail_plugins(&all, &order, |_| false), ["c", "b", "a"]);
}

#[test]
fn rail_badges_prefer_busy_then_the_plugin_badge_then_the_window_count() {
    assert_eq!(rail_badge(true, Some("3"), 2, false), Some(RailBadge::Busy));
    assert_eq!(rail_badge(false, Some("3"), 2, false), Some(RailBadge::Said("3".into())));
    assert_eq!(rail_badge(false, None, 2, false), Some(RailBadge::Windows(2)));
    assert_eq!(rail_badge(false, None, 2, true), None);
    assert_eq!(rail_tooltip("SSH", 1), "SSH — 1 janela aberta");
    assert_eq!(rail_tooltip("SSH", 2), "SSH — 2 janelas abertas");
    assert_eq!(sidebar_placeholder("Git", None), "Abrindo Git…");
    assert_eq!(sidebar_placeholder("Git", Some("x")), "Git caiu: x");
}

#[test]
fn svg_icons_stay_inside_the_plugin_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = write_plugin(tmp.path(), "p", json!({}), &[("icons/logo.svg", "<svg/>")]);
    assert!(svg_of("icons/logo.svg", &dir).is_some());
    assert_eq!(svg_of("../p/icons/logo.svg", &dir), None);
    assert_eq!(svg_of("icons/nada.svg", &dir), None);
    assert!(matches!(glyph(Some("icons/logo.svg"), Some(&dir)), Glyph::Svg(_)));
    assert_eq!(glyph(Some("bug"), Some(&dir)), Glyph::Named("bug_report_outlined"));
    assert_eq!(glyph(Some("reload"), None), Glyph::Named("bolt_rounded"));
    assert_eq!(glyph(Some("nao-existe"), Some(&dir)), Glyph::Generic);
}

#[test]
fn setting_colors_accept_hex_theme_names_or_empty() {
    let p = mx_theme::default_theme();
    assert_eq!(plugin_color("#4FC1FF", &p), mx_theme::Color::parse_hex("#4FC1FF"));
    assert_eq!(plugin_color("", &p), Some(p.fg));
    assert_eq!(plugin_color("red", &p), Some(p.red));
    assert_eq!(plugin_color("azul", &p), None);
    assert_eq!(plugin_color("#12", &p), None);
}

#[test]
fn suggestion_start_params_and_the_prompt_follow_the_card_choice() {
    let task = SuggestedTask {
        id: "task_00000001".into(),
        title: "x".into(),
        tldr: String::new(),
        prompt: "o pedido".into(),
        at: chrono::Local::now(),
    };
    let origin = SuggestionOrigin {
        tab_id: "a".into(),
        cwd: "/repo".into(),
        root: "/repo".into(),
        folder: "meu-repo".into(),
        is_repo: true,
        branch: "main".into(),
    };
    assert_eq!(
        start_params(&task, SuggestionChoice::Here, &origin).to_string(),
        r#"{"suggestion":{"id":"task_00000001","title":"x","tldr":"","prompt":"o pedido"},"choice":"here","origin":{"tabId":"a","cwd":"/repo","root":"/repo","folder":"meu-repo","isRepo":true,"branch":"main"}}"#
    );
    assert_eq!(suggestion_prompt(" Vamos trabalhar na TASK#1 ", "o pedido", None), "Vamos trabalhar na TASK#1\n\no pedido");
    let branch = worktree_branch(None, "TASK#1");
    assert_eq!(branch, "feature/TASK#1");
    assert_eq!(
        suggestion_prompt("", "p", Some(&branch)),
        "p\n\nVocê está na branch `feature/TASK#1`. Leia o código antes de propor o plano."
    );
}

#[test]
fn an_orphan_or_foreign_suggestion_answer_is_ignored() {
    let mut t = SuggestionTracker::default();
    assert_eq!(t.take("tracker", "task_00000001"), None);
    t.track("s", Tracking { plugin_id: "tracker".into(), tab_id: "a".into(), choice: SuggestionChoice::Here });
    assert_eq!(t.take("outro", "s"), None);
    assert!(t.is_tracked("s"));
    assert_eq!(t.take("tracker", "s").unwrap().tab_id, "a");
    assert!(!t.is_tracked("s"));
}
