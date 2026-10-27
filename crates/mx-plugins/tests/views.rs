mod common;

use common::obj;
use mx_core::json::Obj;
use mx_plugins::floats::{FloatCorner, FloatSpot, PluginFloats};
use mx_plugins::rpc::{FloatContent, RfwUpdate};
use mx_plugins::ui::blocks::{Block, MenuEntry};
use mx_plugins::ui::console::{ConsoleLine, Span};
use mx_plugins::ui::fields::FieldState;
use mx_plugins::ui::view::PluginView;
use serde_json::{Value, json};

fn blocks(v: Value) -> Vec<Obj> {
    v.as_array().unwrap().iter().map(|b| obj(b.clone())).collect()
}

#[test]
fn a_console_keeps_its_lines_across_redraws_and_disappears_with_its_block() {
    let mut view = PluginView::new(
        "p",
        "P",
        "v",
        "v",
        blocks(json!([{"type": "console", "id": "log", "lines": ["um", {"text": "dois", "tone": "red"}], "max": 3}])),
    );
    let c = &view.consoles["log"];
    assert_eq!(c.texts(), ["um", "dois"]);
    assert_eq!(c.lines[1].tone.as_deref(), Some("red"));
    view.console("log").append(&json!(["três", "quatro"]));
    assert_eq!(view.consoles["log"].texts(), ["dois", "três", "quatro"]);
    view.set_blocks(blocks(json!([{"type": "button", "action": "x"}, {"type": "console", "id": "log"}])));
    assert_eq!(view.consoles["log"].texts(), ["dois", "três", "quatro"]);
    view.set_blocks(Vec::new());
    assert!(view.consoles.is_empty());
}

#[test]
fn a_console_inside_a_card_is_registered() {
    let view = PluginView::new(
        "p",
        "P",
        "v",
        "v",
        blocks(json!([{"type": "card", "children": [{"type": "console", "id": "log", "lines": ["um"]}]}])),
    );
    assert_eq!(view.consoles["log"].texts(), ["um"]);
}

#[test]
fn console_spans_join_into_the_line_text() {
    let line = ConsoleLine::parse(&json!({"spans": [
        {"text": "api ", "tone": "cyan", "bold": true}, {"text": "│ "}, {"text": "ERROR", "tone": "red"}]}))
    .unwrap();
    assert_eq!(line.text, "api │ ERROR");
    let spans = line.spans.unwrap();
    assert_eq!(spans[0], Span { text: "api ".into(), tone: Some("cyan".into()), bold: true });
    assert_eq!(spans[1], Span { text: "│ ".into(), tone: None, bold: false });
    assert_eq!(spans[2], Span { text: "ERROR".into(), tone: Some("red".into()), bold: false });
    assert_eq!(ConsoleLine::parse(&json!({"text": null})).unwrap().text, "");
    assert_eq!(ConsoleLine::parse(&json!(3)), None);
}

#[test]
fn reopening_a_view_updates_it_in_place_and_bumps_the_revision() {
    let mut view = PluginView::new("p", "Plugin P", "v", "um", Vec::new());
    view.apply_open("dois", blocks(json!([{"type": "text", "text": "oi"}])), None);
    assert_eq!(view.title, "dois");
    assert_eq!(view.subtitle(), "Plugin · Plugin P");
    assert_eq!(view.revision, 1);
}

#[test]
fn rfw_data_merges_per_top_level_key_and_blocks_clear_the_library_only_on_update() {
    let mut view = PluginView::new("docker", "docker", "x", "x", Vec::new());
    view.set_rfw(&RfwUpdate {
        library: Some("widget root = Text();".into()),
        root: Some("root".into()),
        data: Some(obj(json!({"head": {"title": "a"}}))),
    });
    view.set_rfw(&RfwUpdate { data: Some(obj(json!({"rows": []}))), ..RfwUpdate::default() });
    let keys: Vec<&String> = view.rfw_data.keys().collect();
    assert_eq!(keys, ["head", "rows"]);
    assert_eq!(view.rfw_revision, 2);
    view.apply_open("x", blocks(json!([{"type": "text"}])), None);
    assert!(view.uses_rfw());
    view.apply_update(None, Some(Vec::new()), None);
    assert!(!view.uses_rfw());
}

#[test]
fn a_view_lists_the_terminals_its_blocks_embed() {
    let view = PluginView::new(
        "docker",
        "docker",
        "ct.db",
        "db",
        blocks(json!([{"type": "terminal", "tabId": "t1", "expand": true},
            {"type": "row", "children": [{"type": "terminal", "tabId": "t2", "height": 100}]}])),
    );
    assert_eq!(view.terminal_tabs(), ["t1", "t2"]);
    assert_eq!(view.expanding_index(), Some(0));
}

fn content(id: &str, v: Value) -> FloatContent {
    let p = obj(v);
    FloatContent {
        id: id.into(),
        width: p.get("width").and_then(Value::as_f64),
        height: p.get("height").and_then(Value::as_f64),
        corner: p.get("corner").and_then(FloatCorner::parse),
        blocks: None,
        rfw: mx_plugins::rpc::parse_rfw(&p).unwrap(),
    }
}

#[test]
fn floats_show_update_merge_and_hide() {
    let mut floats = PluginFloats::default();
    assert!(!floats.update("pomodoro", &content("timer", json!({"data": {"clock": "25:00"}}))));
    let f = floats.show(
        "pomodoro",
        "Pomodoro",
        &content("timer", json!({"width": 180, "height": 1000, "corner": "topLeft",
            "rfw": {"library": "lib"}, "data": {"clock": "25:00"}})),
    );
    assert_eq!((f.width, f.height, f.start), (180.0, 320.0, FloatCorner::TopLeft));
    assert_eq!(f.view.rfw_library.as_deref(), Some("lib"));
    assert_eq!(f.tab_id, "float:pomodoro/timer");
    assert_eq!(floats.shown().len(), 1);
    assert!(floats.update("pomodoro", &content("timer", json!({"data": {"phase": "foco"}}))));
    let data = &floats.of("pomodoro", "timer").unwrap().view.rfw_data;
    assert_eq!(Value::Object(data.clone()), json!({"clock": "25:00", "phase": "foco"}));
    let small = floats.show("pomodoro", "Pomodoro", &content("mini", json!({"width": 10, "height": 1})));
    assert_eq!((small.width, small.height, small.start), (80.0, 32.0, FloatCorner::BottomRight));
    floats.hide("pomodoro", "timer");
    floats.drop_plugin("pomodoro");
    assert!(floats.shown().is_empty());
}

#[test]
fn float_spots_round_trip_dropping_invalid_entries() {
    let mut floats = PluginFloats::default();
    floats.read_json(&json!({"pomodoro/timer": {"corner": "bottomRight", "dx": 8, "dy": 120.4},
        "x/y": {"corner": "meio", "dx": 1, "dy": 1}, "z/w": "nada"}));
    assert_eq!(floats.spots.len(), 1);
    assert_eq!(floats.to_json().to_string(), r#"{"pomodoro/timer":{"corner":"bottomRight","dx":8.0,"dy":120.0}}"#);
}

#[test]
fn a_dropped_float_snaps_to_the_nearest_corner_edges() {
    let spot = FloatSpot::dropped(20.0, 15.0, 160.0, 50.0, 800.0, 600.0, 8.0);
    assert_eq!(spot, FloatSpot { corner: FloatCorner::TopLeft, dx: 8.0, dy: 8.0 });
    let far = FloatSpot::dropped(500.0, 400.0, 160.0, 50.0, 800.0, 600.0, 8.0);
    assert_eq!(far, FloatSpot { corner: FloatCorner::BottomRight, dx: 140.0, dy: 150.0 });
    let start = FloatSpot { corner: FloatCorner::BottomRight, dx: 8.0, dy: 8.0 };
    assert_eq!(start.origin(160.0, 50.0, 800.0, 600.0, 8.0), (632.0, 542.0));
}

#[test]
fn typed_text_survives_a_redraw_with_the_same_value_but_not_a_new_one() {
    let mut fields = FieldState::default();
    let page = blocks(json!([{"type": "input", "id": "nome", "value": "ana"},
        {"type": "row", "children": [{"type": "input", "id": "lado", "placeholder": "ao lado"}]}]));
    fields.sync(&page);
    fields.set_text("nome", "bia");
    fields.sync(&page);
    assert_eq!(fields.text("nome"), Some("bia"));
    fields.sync(&blocks(json!([{"type": "input", "id": "nome", "value": "cris"}])));
    assert_eq!(fields.text("nome"), Some("cris"));
    assert_eq!(fields.text("lado"), None);
}

#[test]
fn a_calendar_choice_survives_the_plugin_resending_the_same_period() {
    let page = blocks(json!([{"type": "calendar", "id": "periodo", "mode": "range",
        "value": {"from": "2026-09-22", "to": "2026-09-22"}, "max": "2026-09-23"}, {"type": "button", "action": "ir"}]));
    let mut fields = FieldState::default();
    fields.sync(&page);
    fields.set_pick("periodo", json!({"from": "2026-09-10", "to": "2026-09-18"}));
    fields.sync(&page);
    assert_eq!(Value::Object(fields.values()).to_string(), r#"{"periodo":{"from":"2026-09-10","to":"2026-09-18"}}"#);
}

#[test]
fn values_include_fields_nested_in_cards_and_columns() {
    let page = blocks(json!([{"type": "columns", "children": [
        {"type": "card", "children": [{"type": "progress", "value": 0.12}]},
        {"type": "card", "children": [{"type": "input", "id": "busca", "icon": "search"}]}]}]));
    let mut fields = FieldState::default();
    fields.sync(&page);
    fields.set_text("busca", "nats");
    assert_eq!(Value::Object(fields.values()).to_string(), r#"{"busca":"nats"}"#);
}

#[test]
fn unknown_and_id_less_blocks_render_as_labels() {
    let parsed = Block::parse_all(&blocks(json!([{"type": "carrossel"}, {}, {"type": "input"},
        {"type": "console"}, {"type": "select"}, {"type": "checkbox"}, {"type": "calendar"}])));
    let labels: Vec<String> = parsed
        .iter()
        .map(|b| match b {
            Block::Unknown { label } => label.clone(),
            Block::Broken { message } => message.clone(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        labels,
        [
            "Bloco desconhecido: carrossel",
            "Bloco desconhecido: (sem \"type\")",
            "input sem \"id\"",
            "console sem \"id\"",
            "select sem \"id\"",
            "checkbox sem \"id\"",
            "calendar sem \"id\"",
        ]
    );
}

#[test]
fn list_items_drop_actions_without_a_known_icon_and_toggle_falls_back_to_action() {
    let parsed = Block::parse(&obj(json!({"type": "list", "flat": true, "items": [
        {"title": "main.dart", "badge": "M", "action": "diff", "actions": [
            {"action": "descartar", "icon": "undo", "tooltip": "descartar"},
            {"action": "preparar", "icon": "add", "tooltip": "preparar"},
            {"action": "sem-icone"}],
         "menu": [{"action": "editar", "label": "editar…", "icon": "edit"}, {"type": "divider"},
            {"action": "apagar", "label": "apagar", "tone": "red"}],
         "expanded": true, "avatar": "box"}]})));
    let Block::List { items, empty, flat, .. } = parsed else { panic!("not a list") };
    assert!(flat);
    assert_eq!(empty, "Nada aqui");
    let it = &items[0];
    let actions: Vec<&str> = it.actions.iter().map(|a| a.action.as_str()).collect();
    assert_eq!(actions, ["descartar", "preparar"]);
    assert_eq!(it.toggle.as_deref(), Some("diff"));
    assert_eq!(it.expanded, Some(true));
    assert_eq!(it.avatar.as_ref().unwrap().icon.as_deref(), Some("box"));
    assert_eq!(it.menu[1], MenuEntry::Divider);
    assert!(matches!(&it.menu[0], MenuEntry::Item { label, .. } if label == "editar…"));
}

#[test]
fn header_actions_are_icon_buttons_and_tab_counts_are_text() {
    let parsed = Block::parse_all(&blocks(json!([
        {"type": "header", "title": "Containers", "actions": [{"action": "atualizar", "icon": "refresh", "tooltip": "atualizar"}]},
        {"type": "tabs", "items": [{"label": "Containers", "action": "aba:c", "active": true, "count": 49}]},
        {"type": "button", "id": "ir"},
        {"type": "select", "id": "cor", "options": ["azul", {"value": "v", "label": "verde"}], "value": "v"}])));
    let Block::Header { actions, .. } = &parsed[0] else { panic!() };
    assert!(actions[0].icon_only());
    let Block::Tabs { items, .. } = &parsed[1] else { panic!() };
    assert_eq!(items[0].count.as_deref(), Some("49"));
    let Block::Button(b) = &parsed[2] else { panic!() };
    assert_eq!(b.shown_label(), "ir");
    let Block::Select { options, .. } = &parsed[3] else { panic!() };
    assert_eq!(mx_plugins::ui::blocks::select_choice(options, Some(&json!("v"))), Some(1));
    assert_eq!(mx_plugins::ui::blocks::select_choice(options, Some(&json!("x"))), None);
}
