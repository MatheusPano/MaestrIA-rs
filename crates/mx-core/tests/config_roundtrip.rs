use mx_core::model::{FeatureOrHotfix, FeatureOrHotfixKind, Folder, Workspace};
use mx_core::persist::Config;
use proptest::prelude::*;
use serde_json::Value;

fn name() -> impl Strategy<Value = String> {
    "[a-zA-Zçã/ #-]{1,12}".prop_map(|s| format!("x{s}"))
}

fn config() -> impl Strategy<Value = Config> {
    (
        prop::collection::vec((name(), name(), any::<bool>()), 0..5),
        prop::collection::vec(
            (
                name(),
                any::<bool>(),
                prop::collection::vec(0usize..5, 0..4),
            ),
            0..3,
        ),
        prop::collection::vec(name(), 0..4),
        (
            248.0f64..680.0,
            any::<bool>(),
            any::<bool>(),
            0i64..1440,
            any::<bool>(),
        ),
        prop::collection::vec((name(), 0usize..5, any::<bool>()), 0..4),
    )
        .prop_map(
            |(folders, workspaces, order, (width, hidden, dnd, minutes, gc), features)| {
                let folders: Vec<Folder> = folders
                    .into_iter()
                    .map(|(root, name, collapsed)| Folder {
                        collapsed,
                        ..Folder::new(format!("/{root}"), name)
                    })
                    .collect();
                let workspaces = workspaces
                    .into_iter()
                    .enumerate()
                    .map(|(i, (name, collapsed, picks))| {
                        let mut w = Workspace::new(format!("ws{i}"), name.trim().to_owned() + "w");
                        w.collapsed = collapsed;
                        for p in picks {
                            if let Some(f) = folders.get(p) {
                                w.add_root(&f.root);
                            }
                        }
                        w
                    })
                    .collect();
                let features = features
                    .into_iter()
                    .enumerate()
                    .map(|(i, (name, at, hot))| {
                        let root = folders
                            .get(at)
                            .map_or("/home".to_owned(), |f| f.root.clone());
                        let kind = if hot {
                            FeatureOrHotfixKind::Hotfix
                        } else {
                            FeatureOrHotfixKind::Feature
                        };
                        FeatureOrHotfix::new(format!("pj{i}"), root, name, kind)
                    })
                    .collect();
                Config {
                    folders,
                    workspaces,
                    root_order: order.into_iter().map(|o| format!("folder:/{o}")).collect(),
                    features_or_hotfixes: features,
                    sidebar_width: width,
                    sidebar_hidden: hidden,
                    do_not_disturb: dnd,
                    hibernate_minutes: minutes,
                    groups_collapsed: gc,
                    ..Config::default()
                }
            },
        )
}

proptest! {
    #[test]
    fn config_survives_write_then_read(c in config()) {
        let written = c.to_json_string();
        let parsed: Value = serde_json::from_str(&written).unwrap();
        let (back, skipped) = Config::from_json(parsed.as_object().unwrap());
        prop_assert!(skipped.0.is_empty());
        prop_assert_eq!(&back.folders, &c.folders);
        prop_assert_eq!(&back.workspaces, &c.workspaces);
        prop_assert_eq!(&back.features_or_hotfixes, &c.features_or_hotfixes);
        prop_assert_eq!(back.sidebar_width, c.sidebar_width);
        prop_assert_eq!(back.to_json_string(), written);
    }
}
