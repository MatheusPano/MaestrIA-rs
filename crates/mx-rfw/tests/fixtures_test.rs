mod common;

use common::{content, find, fixture, fixtures_dir, problems, texts};
use mx_rfw::{DynamicContent, Value, evaluate, parse_library_file};

fn sources() -> Vec<(String, String)> {
    std::fs::read_to_string(fixtures_dir().join("SOURCES.txt"))
        .unwrap()
        .lines()
        .filter_map(|l| l.split_once(" <- "))
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
        .collect()
}

fn data_for(fixture: &str) -> (&'static str, &'static [&'static str]) {
    let stem = fixture.rsplit("__").next().unwrap();
    match stem {
        "float.rfwtxt" => ("pomodoro_float.json", &["root"]),
        "sidebar.rfwtxt" if fixture.contains("docker") => ("docker_sidebar.json", &["root"]),
        "sidebar.rfwtxt" => ("pomodoro_sidebar.json", &["root"]),
        "hosts.rfwtxt" => ("ssh_hosts.json", &["root"]),
        "argo.rfwtxt" => ("argo.json", &["home", "tree"]),
        "kanban.rfwtxt" => ("kanban.json", &["root"]),
        "devtools.rfwtxt" => ("devtools.json", &["root"]),
        "editor.rfwtxt" => ("editor.json", &["root"]),
        other => panic!("no data for {other}"),
    }
}

fn data(name: &str) -> DynamicContent {
    content(&std::fs::read_to_string(fixtures_dir().join("data").join(name)).unwrap())
}

#[test]
fn every_real_rfwtxt_file_is_copied_and_listed_in_sources() {
    let listed = sources();
    assert_eq!(listed.len(), 11);
    for (name, origin) in &listed {
        assert!(fixtures_dir().join(name).is_file(), "{name}");
        assert!(origin.ends_with(".rfwtxt"), "{origin}");
    }
}

#[test]
fn every_real_rfwtxt_file_parses() {
    for (name, _) in sources() {
        let lib = parse_library_file(&fixture(&name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let imports: Vec<String> = lib.imports.iter().map(|i| i.dotted()).collect();
        assert!(imports.contains(&"core.widgets".to_owned()), "{name}");
        assert!(imports.contains(&"maestria".to_owned()), "{name}");
        assert!(!lib.widgets.is_empty(), "{name}");
    }
}

#[test]
fn every_real_rfwtxt_file_evaluates_with_representative_data_without_errors() {
    for (name, _) in sources() {
        let lib = parse_library_file(&fixture(&name)).unwrap();
        let (data_file, roots) = data_for(&name);
        let content = data(data_file);
        for root in roots {
            let tree = evaluate(&lib, root, &content);
            let found = problems(&tree);
            assert!(found.is_empty(), "{name} root {root}: {found:#?}");
            assert!(!texts(&tree).is_empty() || *root == "tree", "{name} root {root} rendered no text");
        }
    }
}

#[test]
fn devtools_every_tab_and_the_offline_state_evaluate_without_errors() {
    let lib = parse_library_file(&fixture("maestria-plugins__flutter__devtools.rfwtxt")).unwrap();
    for (tab, live) in [("perf", true), ("cpu", true), ("mem", true), ("net", true), ("perf", false)] {
        let mut content = data("devtools.json");
        let mut head = content.root().get("head").unwrap().as_map().unwrap().clone();
        head.insert("tab".into(), Value::from(tab));
        head.insert("live".into(), Value::Bool(live));
        content.update("head", Value::Map(head));
        let tree = evaluate(&lib, "root", &content);
        assert!(problems(&tree).is_empty(), "tab {tab} live {live}: {:#?}", problems(&tree));
    }
}

#[test]
fn kanban_narrow_layout_skeleton_stats_and_drag_evaluate_without_errors() {
    let lib = parse_library_file(&fixture("maestria-plugins__wiboor__kanban.rfwtxt")).unwrap();
    for width in ["narrow", "wide", "other"] {
        let mut content = data("kanban.json");
        content.update("layout", Value::from_json(&serde_json::json!({ "width": width })));
        content.update("statsSkeleton", Value::Bool(true));
        content.update("drag", Value::Bool(true));
        let tree = evaluate(&lib, "root", &content);
        assert!(problems(&tree).is_empty(), "width {width}: {:#?}", problems(&tree));
    }
}

#[test]
fn kanban_loop_over_a_switch_yields_the_three_skeleton_pills() {
    let lib = parse_library_file(&fixture("maestria-plugins__wiboor__kanban.rfwtxt")).unwrap();
    let mut content = data("kanban.json");
    let bone = |n: &mx_rfw::Node| n.name == "Container" && n.arg("height").as_double() == Some(24.0);
    assert!(find(&evaluate(&lib, "root", &content), bone).is_empty());
    content.update("statsSkeleton", Value::Bool(true));
    let tree = evaluate(&lib, "root", &content);
    let widths: Vec<Option<f64>> = find(&tree, bone).iter().map(|n| n.arg("width").as_double()).collect();
    assert_eq!(widths, vec![Some(78.0), Some(112.0), Some(96.0)]);
}

#[test]
fn docker_sidebar_renders_the_head_title_and_every_row_kind() {
    let lib = parse_library_file(&fixture("maestria-plugins__docker__sidebar.rfwtxt")).unwrap();
    let tree = evaluate(&lib, "root", &data("docker_sidebar.json"));
    let t = texts(&tree);
    for expected in ["Containers", "loja", "api", "db", "logs"] {
        assert!(t.iter().any(|s| s == expected), "{expected} not in {t:?}");
    }
    let rotations = find(&tree, |n| n.name == "Rotation");
    assert!(rotations.iter().all(|r| r.arg("duration").as_int() == Some(140)));
}

#[test]
fn ssh_avatar_badge_keeps_right_but_the_positioned_reader_ignores_it() {
    let lib = parse_library_file(&fixture("maestria-plugins__ssh__hosts.rfwtxt")).unwrap();
    let tree = evaluate(&lib, "root", &data("ssh_hosts.json"));
    let badges = find(&tree, |n| n.name == "Positioned" && !n.arg("right").is_missing());
    assert!(!badges.is_empty());
    for b in badges {
        let p = mx_rfw::widgets::core_widgets::Positioned::from_node(b);
        assert_eq!((p.start, p.end), (None, None));
        assert!(p.bottom.is_some());
    }
}

#[test]
fn argo_tree_positions_nodes_from_graph_data() {
    let lib = parse_library_file(&fixture("argocd-plugin__ui__argo.rfwtxt")).unwrap();
    let tree = evaluate(&lib, "tree", &data("argo.json"));
    let placed = find(&tree, |n| n.name == "Positioned" && n.arg("start").as_double() == Some(10.0));
    assert_eq!(placed.len(), 1);
    assert_eq!(placed[0].arg("top").as_double(), Some(20.0));
}

#[test]
fn duplicated_plugin_copies_are_byte_identical() {
    for (a, b) in [
        ("argocd-plugin__ui__argo.rfwtxt", "maestria-plugins__argocd__argo.rfwtxt"),
        ("maestria-plugins__pomodoro__float.rfwtxt", "maestria-plugins__pomodoro_dart__float.rfwtxt"),
        ("maestria-plugins__pomodoro__sidebar.rfwtxt", "maestria-plugins__pomodoro_dart__sidebar.rfwtxt"),
    ] {
        assert_eq!(fixture(a), fixture(b));
    }
}
