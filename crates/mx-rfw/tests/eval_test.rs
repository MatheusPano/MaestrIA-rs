mod common;

use common::{content, find, texts};
use mx_rfw::widgets::maestria::{DropTarget, Pressable, menu_extra};
use mx_rfw::{DynamicContent, LocalLibrary, Map, Value, Widget, evaluate, parse_library_file};
use serde_json::json;

const HEAD: &str = "import core.widgets;\nimport maestria;\n";

fn eval(src: &str, data: serde_json::Value) -> Widget {
    let lib = parse_library_file(&format!("{HEAD}{src}")).unwrap();
    evaluate(&lib, "root", &content(&data.to_string()))
}

fn text_of(src: &str, data: serde_json::Value) -> String {
    texts(&eval(src, data)).join("|")
}

fn error_of(w: &Widget) -> &str {
    match w {
        Widget::Error(e) => e,
        other => panic!("expected an error widget, got {other:?}"),
    }
}

#[test]
fn a_switch_on_an_int_matches_a_double_case_and_vice_versa() {
    let src = "widget root = Text(text: switch data.v { 1.0: \"one\", 0: \"zero\", default: \"other\" });";
    assert_eq!(text_of(src, json!({"v": 1})), "one");
    assert_eq!(text_of(src, json!({"v": 0.0})), "zero");
    assert_eq!(text_of(src, json!({"v": 2})), "other");
}

#[test]
fn a_switch_on_strings_and_bools_matches_by_value() {
    let src = "widget root = Text(text: switch data.v { \"a\": \"A\", true: \"T\", default: \"D\" });";
    assert_eq!(text_of(src, json!({"v": "a"})), "A");
    assert_eq!(text_of(src, json!({"v": true})), "T");
    assert_eq!(text_of(src, json!({"v": "true"})), "D");
}

#[test]
fn a_switch_on_a_map_or_a_missing_value_falls_back_to_default() {
    let src = "widget root = Text(text: switch data.v { \"{}\": \"map\", default: \"D\" });";
    assert_eq!(text_of(src, json!({"v": {}})), "D");
    assert_eq!(text_of(src, json!({"v": []})), "D");
    assert_eq!(text_of(src, json!({})), "D");
}

#[test]
fn a_switch_case_key_may_not_start_with_a_symbol() {
    assert!(parse_library_file("widget root = switch 1 { {}: A(), default: B() };").is_err());
}

#[test]
fn a_switch_without_default_and_no_match_is_missing() {
    let w = eval("widget root = Text(text: switch data.v { 1: \"one\" });", json!({"v": 2}));
    assert!(w.as_node().unwrap().arg("text").is_missing());
}

#[test]
fn reference_parts_continue_into_the_chosen_switch_branch() {
    let w = eval(
        "widget root = Row(children: [Label(m: switch data.k { true: { t: \"yes\" }, default: { t: \"no\" } })]);\nwidget Label = Text(text: args.m.t);",
        json!({"k": true}),
    );
    assert_eq!(texts(&w), ["yes"]);
}

#[test]
fn a_root_switch_that_does_not_resolve_to_a_widget_renders_an_error() {
    let w = eval("widget root = switch data.v { 1: Text(text: \"x\") };", json!({"v": 2}));
    assert_eq!(error_of(&w), "Switch in plugin:root did not resolve to a widget (got <missing>).");
}

#[test]
fn loops_expand_in_place_between_fixed_items() {
    let src = "widget root = Column(children: [Text(text: \"a\"), ...for r in data.rows: Text(text: r.t), Text(text: \"z\")]);";
    assert_eq!(text_of(src, json!({"rows": [{"t": "1"}, {"t": "2"}]})), "a|1|2|z");
}

#[test]
fn a_loop_over_a_non_list_expands_to_nothing() {
    let src = "widget root = Column(children: [...for r in data.rows: Text(text: r)]);";
    for rows in [json!({"a": 1}), json!(3), json!(null)] {
        assert_eq!(text_of(src, json!({"rows": rows})), "");
    }
    assert_eq!(text_of(src, json!({})), "");
}

#[test]
fn a_loop_over_a_switch_of_literal_lists() {
    let src = "widget root = Column(children: [...for w in switch data.s { true: [\"x\", \"y\"], default: [] }: Text(text: w)]);";
    assert_eq!(text_of(src, json!({"s": true})), "x|y");
    assert_eq!(text_of(src, json!({"s": false})), "");
}

#[test]
fn nested_loops_can_reach_the_outer_variable() {
    let src = "widget root = Column(children: [...for g in data.groups: Row(children: [...for i in g.items: Text(text: [g.name, \":\", i])])]);";
    let data = json!({"groups": [{"name": "a", "items": ["1", "2"]}, {"name": "b", "items": ["3"]}]});
    assert_eq!(text_of(src, data), "a:1|a:2|b:3");
}

#[test]
fn list_indices_count_loop_expansions() {
    let w = eval(
        "widget root = Show(l: [\"first\", ...for r in data.rows: r, \"last\"]);\nwidget Show = Text(text: args.l.3);",
        json!({"rows": ["a", "b"]}),
    );
    assert_eq!(texts(&w), ["last"]);
}

#[test]
fn args_are_bound_lazily_in_the_callers_scope() {
    let w = eval(
        "widget root = Column(children: [...for t in data.items: Tile(t: t)]);\nwidget Tile = Text(text: args.t.title);",
        json!({"items": [{"title": "web"}, {"title": "db"}]}),
    );
    assert_eq!(texts(&w), ["web", "db"]);
}

#[test]
fn a_widget_passed_as_an_argument_is_built_where_it_is_used() {
    let w = eval(
        "widget root = Wide(child: Text(text: data.t));\nwidget Wide = SizedBox(width: 10.0, child: args.child);",
        json!({"t": "hi"}),
    );
    let n = w.as_node().unwrap();
    assert_eq!(n.name, "SizedBox");
    assert_eq!(texts(&w), ["hi"]);
}

#[test]
fn missing_args_and_data_resolve_to_missing() {
    let w = eval("widget root = Text(text: args.nothing, maxLines: data.no.such.path);", json!({}));
    let n = w.as_node().unwrap();
    assert!(n.arg("text").is_missing() && n.arg("maxLines").is_missing());
}

#[test]
fn indexing_into_a_scalar_gives_missing() {
    let w = eval("widget root = Text(text: data.x.y);", json!({"x": 3}));
    assert!(w.as_node().unwrap().arg("text").is_missing());
}

#[test]
fn json_ints_and_doubles_stay_distinct() {
    let w = eval("widget root = SizedBox(width: data.a, height: data.b);", json!({"a": 12, "b": 12.0}));
    let n = w.as_node().unwrap();
    assert_eq!(n.arg("width"), &Value::Int(12));
    assert_eq!(n.arg("height"), &Value::Double(12.0));
}

#[test]
fn local_widgets_resolve_through_imports_in_order() {
    let w = eval("widget root = Column(children: [IconButton(icon: \"x\"), Tooltip(message: \"m\", child: Spacer())]);", json!({}));
    let libs: Vec<LocalLibrary> = find(&w, |_| true).iter().map(|n| n.library).collect();
    assert_eq!(libs, [LocalLibrary::CoreWidgets, LocalLibrary::Maestria, LocalLibrary::Maestria, LocalLibrary::CoreWidgets]);
}

#[test]
fn a_remote_widget_shadows_a_local_one_with_the_same_name() {
    let w = eval("widget root = Card();\nwidget Card = Text(text: \"remote\");", json!({}));
    assert_eq!(texts(&w), ["remote"]);
}

#[test]
fn material_widgets_need_the_material_import() {
    let w = eval("widget root = CircularProgressIndicator();", json!({}));
    assert_eq!(error_of(&w), "Could not find remote widget named CircularProgressIndicator in plugin.");
}

#[test]
fn an_unknown_import_is_listed_as_a_missing_dependency() {
    let lib = parse_library_file("import foo.bar;\nwidget root = Nope();").unwrap();
    let w = evaluate(&lib, "root", &DynamicContent::new());
    assert_eq!(
        error_of(&w),
        "Could not find remote widget named Nope in plugin, possibly because some dependencies were missing: foo.bar"
    );
}

#[test]
fn an_unknown_root_renders_an_error_widget() {
    let lib = parse_library_file(HEAD).unwrap();
    let w = evaluate(&lib, "home", &DynamicContent::new());
    assert_eq!(error_of(&w), "Could not find remote widget named home in plugin.");
}

#[test]
fn recursive_widgets_render_a_widget_loop_error() {
    let w = eval("widget root = Column(children: [A()]);\nwidget A = Row(children: [B()]);\nwidget B = A();", json!({}));
    let errors: Vec<String> = common::problems(&w);
    assert_eq!(errors, ["Widget loop: Tried to call plugin:A constructor reentrantly."]);
}

#[test]
fn events_carry_their_name_and_deep_resolved_arguments() {
    let w = eval(
        "widget root = IconButton(onPressed: event \"act\" { a: data.a, l: [...for x in data.xs: x], m: { k: data.k } });",
        json!({"a": "go", "xs": [1, 2], "k": true}),
    );
    let n = w.as_node().unwrap();
    let ev = n.handlers("onPressed")[0];
    assert_eq!(ev.name, "act");
    assert_eq!(serde_json::Value::Object(ev.values(&Map::new())), json!({"a": "go", "l": [1, 2], "m": {"k": true}}));
}

#[test]
fn extra_event_arguments_are_appended_and_win_on_duplicates() {
    let w = eval("widget root = Pressable(onMenu: event \"act\" { action: \"old\", z: 1 }, menu: []);", json!({}));
    let ev = w.as_node().unwrap().handlers("onMenu")[0];
    let values = ev.values(&menu_extra("parar:1"));
    assert_eq!(serde_json::to_string(&values).unwrap(), r#"{"action":"parar:1","z":1}"#);
}

#[test]
fn missing_values_are_dropped_from_event_json() {
    let w = eval("widget root = IconButton(onPressed: event \"act\" { a: data.none, b: \"x\" });", json!({}));
    let ev = w.as_node().unwrap().handlers("onPressed")[0];
    assert_eq!(serde_json::Value::Object(ev.values(&Map::new())), json!({"b": "x"}));
}

#[test]
fn plugins_rfw_monta_e_eventos() {
    let lib = parse_library_file(concat!(
        "import core.widgets;\nimport maestria;\n",
        "widget root = Column(children: [\n",
        "  Text(text: data.head.title, style: { color: data.theme.accent }),\n",
        "  ...for row in data.rows:\n",
        "    Pressable(onTap: event \"act\" { a: row.action }, menu: row.menu, onMenu: event \"act\" {}, child: Text(text: row.title)),\n",
        "  IconButton(icon: \"stop\", tooltip: \"parar\", onPressed: event \"act\" { a: \"stop:1\" }),\n",
        "]);\n",
    ))
    .unwrap();
    let mut data = content(
        &json!({"head": {"title": "Containers"}, "rows": [{"title": "api", "action": "abrir:1", "menu": [
            {"label": "Parar", "icon": "stop", "action": "parar:1"}, {"divider": true},
            {"label": "Remover", "action": "remover:1", "red": true}]}]})
        .to_string(),
    );
    let tree = evaluate(&lib, "root", &data);
    let title = find(&tree, |n| n.name == "Text")[0];
    let style = mx_rfw::decoders::text_style(title, "style").unwrap();
    assert_eq!(style.color, Some(mx_rfw::decoders::Color(0xFFF2_762E)));
    let pressable = Pressable::from_node(find(&tree, |n| n.name == "Pressable")[0], false);
    assert_eq!(pressable.on_tap[0].values(&Map::new())["a"], "abrir:1");
    assert_eq!(pressable.menu.len(), 3);
    assert!(pressable.menu[1].is_divider());
    assert!(pressable.menu[2].red);
    let chosen = pressable.on_menu[0].values(&menu_extra(pressable.menu[0].action.as_deref().unwrap()));
    assert_eq!(serde_json::Value::Object(chosen), json!({"action": "parar:1"}));
    data.merge_plugin_data(json!({"rows": [{"title": "worker", "action": "abrir:2", "menu": []}]}).as_object().unwrap());
    let t = texts(&evaluate(&lib, "root", &data));
    assert_eq!(t, ["Containers", "worker"]);
}

#[test]
fn plugins_rfw_drag_drop_values() {
    let tree = eval(
        concat!(
            "widget root = Row(children: [...for col in data.cols: DropTarget(id: col.id, hint: col.hint, radius: 8.0, onDrop: event \"drop\" {},\n",
            "  child: Column(children: [Text(text: col.id), ...for card in col.cards: Draggable(payload: card.id, targets: card.targets, width: 120.0, child: Text(text: card.title))]))]);"
        ),
        json!({"cols": [
            {"id": "todo", "hint": "solte pra voltar", "cards": [{"id": "t1", "title": "cartão", "targets": ["doing"]}]},
            {"id": "doing", "hint": "solte pra iniciar", "cards": []},
            {"id": "done", "hint": "solte pra finalizar", "cards": []}]}),
    );
    let drag = mx_rfw::widgets::maestria::Draggable::from_node(find(&tree, |n| n.name == "Draggable")[0]);
    let targets: Vec<DropTarget> = find(&tree, |n| n.name == "DropTarget").into_iter().map(DropTarget::from_node).collect();
    assert!(!drag.fits(&targets[2].id));
    assert!(drag.fits(&targets[1].id));
    assert_eq!(targets[1].hint.as_deref(), Some("solte pra iniciar"));
    let values = targets[1].on_drop[0].values(&targets[1].drop_extra(&drag.payload));
    assert_eq!(targets[1].on_drop[0].name, "drop");
    assert_eq!(serde_json::to_string(&values).unwrap(), r#"{"payload":"t1","to":"doing"}"#);
}

#[test]
fn plugins_rfw_submenu_colours_are_integers() {
    let tree = eval(
        "widget root = Pressable(menu: data.menu, onMenu: event \"act\" {}, child: Text(text: \"api\"));",
        json!({"menu": [{"label": "Cor", "icon": "dot", "action": "", "children": [
            {"label": "Automática", "action": "cor:n:api:", "color": 0},
            {"label": "Laranja", "action": "cor:n:api:orange", "color": 0xFFF2762E_u32}]}]}),
    );
    let p = Pressable::from_node(tree.as_node().unwrap(), false);
    assert_eq!(p.menu[0].children[1].color, 0xFFF2_762E);
    assert_eq!(p.menu[0].children[0].color, 0);
    assert_eq!(p.menu[0].shown_label(), "Cor");
}

#[test]
fn the_theme_key_sent_by_a_plugin_is_ignored() {
    let mut data = content(&json!({}).to_string());
    data.merge_plugin_data(json!({"theme": {"fg": 1}}).as_object().unwrap());
    assert_eq!(data.root()["theme"].get("fg"), &Value::Int(0xFFEE_EEEE));
}

#[test]
fn absurdly_deep_remote_chains_stop_with_an_error_instead_of_overflowing() {
    let mut src = String::from(HEAD);
    src.push_str("widget root = W0();\n");
    for i in 0..400 {
        src.push_str(&format!("widget W{i} = Padding(child: W{}());\n", i + 1));
    }
    src.push_str("widget W400 = Spacer();\n");
    let lib = parse_library_file(&src).unwrap();
    let tree = evaluate(&lib, "root", &DynamicContent::new());
    assert!(common::problems(&tree).iter().any(|e| e.starts_with("Widget tree deeper than")));
}

#[test]
fn deep_lists_inside_long_chains_are_also_capped() {
    let nested = format!("{}Spacer(){}", "[".repeat(150), "]".repeat(150));
    let mut src = String::from(HEAD);
    src.push_str("widget root = W0();\n");
    for i in 0..10 {
        src.push_str(&format!("widget W{i} = Stack(children: [W{}(), Column(children: {nested})]);\n", i + 1));
    }
    src.push_str("widget W10 = Spacer();\n");
    let lib = parse_library_file(&src).unwrap();
    let tree = evaluate(&lib, "root", &DynamicContent::new());
    assert!(matches!(tree, Widget::Node(_)));
}
