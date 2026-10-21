mod common;

use std::collections::BTreeMap;

use common::fixture;
use mx_rfw::ast::{Expr, Item};
use mx_rfw::parse_library_file;

#[derive(Default, Debug, PartialEq)]
struct Counts {
    data: usize,
    args: usize,
    loops: usize,
    loop_vars: usize,
    switches: usize,
    events: usize,
}

fn count(e: &Expr, c: &mut Counts) {
    match e {
        Expr::Data(_) => c.data += 1,
        Expr::Args(_) => c.args += 1,
        Expr::LoopVar { .. } => c.loop_vars += 1,
        Expr::Map(entries) | Expr::Call { args: entries, .. } => entries.iter().for_each(|(_, v)| count(v, c)),
        Expr::Event { args, .. } => {
            c.events += 1;
            args.iter().for_each(|(_, v)| count(v, c));
        }
        Expr::Switch(sw) => {
            c.switches += 1;
            count(&sw.input, c);
            for (k, v) in &sw.cases {
                count(k, c);
                count(v, c);
            }
            if let Some(d) = &sw.default {
                count(d, c);
            }
        }
        Expr::List(items) => {
            for item in items {
                match item {
                    Item::Value(v) => count(v, c),
                    Item::Loop { input, template } => {
                        c.loops += 1;
                        count(input, c);
                        count(template, c);
                    }
                }
            }
        }
        Expr::Int(_) | Expr::Double(_) | Expr::Bool(_) | Expr::String(_) => {}
    }
}

#[test]
fn parsed_fixtures_match_the_independent_reference_inventory() {
    let reference: BTreeMap<&str, (&[&str], Counts)> = BTreeMap::from([
        ("maestria-plugins__docker__sidebar.rfwtxt", (
            &["root", "Head", "Label", "Avatar", "TitleText", "SubText", "Actions", "Pill", "ContainerRow", "ProjectRow", "TermRow"][..],
            Counts { data: 34, args: 58, loops: 2, loop_vars: 8, switches: 21, events: 10 },
        )),
        ("maestria-plugins__ssh__hosts.rfwtxt", (
            &["root", "Head", "Toolbar", "Action", "Crumb", "Section", "Tile", "Avatar"][..],
            Counts { data: 50, args: 25, loops: 2, loop_vars: 2, switches: 12, events: 8 },
        )),
        ("maestria-plugins__pomodoro__float.rfwtxt", (
            &["root", "Ring"][..],
            Counts { data: 16, args: 0, loops: 0, loop_vars: 0, switches: 4, events: 3 },
        )),
        ("maestria-plugins__pomodoro__sidebar.rfwtxt", (
            &["root", "Segmented", "Dial", "Controls", "Today", "Ruler", "Preset", "Stepper", "Toggle", "Caption", "IconChip", "Swatch"][..],
            Counts { data: 63, args: 30, loops: 10, loop_vars: 14, switches: 15, events: 11 },
        )),
        ("maestria-plugins__argocd__argo.rfwtxt", (
            &["home", "tree", "Head", "Stats", "Notice", "Dot", "Pill", "Node", "Tile", "Crumb", "ClusterTile"][..],
            Counts { data: 89, args: 42, loops: 7, loop_vars: 17, switches: 13, events: 10 },
        )),
        ("maestria-plugins__wiboor__kanban.rfwtxt", (
            &["root", "Head", "Toolbar", "PillBone", "Scopes", "Scope", "Pill", "Notice", "Dot", "Wide", "Lane", "LaneShape",
              "LaneBody", "LaneBox", "LaneGhost", "LaneStripe", "LaneHead", "Folded", "Skeletons", "Skeleton", "Block", "Bone",
              "Empty", "DragCard", "Card", "Kind", "Prio", "Bar", "Who", "Due", "Meta", "Claude"][..],
            Counts { data: 161, args: 136, loops: 7, loop_vars: 10, switches: 58, events: 17 },
        )),
        ("maestria-plugins__flutter__devtools.rfwtxt", (
            &["root", "Head", "Tab", "Offline", "Card", "CardTitle", "Note", "Warn", "Stats", "Button", "Segmented", "Key",
              "Toggle", "Perf", "FrameChart", "ScaleLabel", "FrameBar", "Bar", "Cpu", "CpuRow", "Pct", "Mem", "MemBar", "Net",
              "NetRow", "NetDetail"][..],
            Counts { data: 201, args: 74, loops: 12, loop_vars: 34, switches: 39, events: 18 },
        )),
    ]);
    for (file, (decls, expected)) in reference {
        let lib = parse_library_file(&fixture(file)).unwrap();
        let names: Vec<&str> = lib.widgets.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(names, decls, "{file}");
        let mut got = Counts::default();
        lib.widgets.iter().for_each(|w| count(&w.root, &mut got));
        assert_eq!(got, expected, "{file}");
    }
}
