mod common;

use common::obj;
use mx_plugins::ui::blocks::Block;
use mx_plugins::ui::icons::material_icon;
use serde_json::json;

#[test]
fn ic1_clean_and_clear_are_different_icons() {
    assert_eq!(material_icon("clean"), Some("cleaning_services_outlined"));
    assert_eq!(material_icon("clear"), Some("block_rounded"));
}

#[test]
fn the_ten_new_plugin_icons_map_to_material_names() {
    let expected = [
        ("clean", "cleaning_services_outlined"),
        ("send", "send_rounded"),
        ("chat", "chat_bubble_outline_rounded"),
        ("pets", "pets_rounded"),
        ("photo", "image_outlined"),
        ("home", "home_outlined"),
        ("moon", "bedtime_outlined"),
        ("shield", "verified_user_outlined"),
        ("close", "close_rounded"),
        ("palette", "palette_outlined"),
    ];
    for (name, icon) in expected {
        assert_eq!(material_icon(name), Some(icon), "{name}");
    }
    assert_eq!(material_icon("box"), Some("inventory_2_outlined"));
}

#[test]
fn li1_list_items_read_avatar_size_clamped_and_title_tone() {
    let block = obj(json!({
        "type": "list",
        "flat": true,
        "items": [
            {"title": "app.js", "avatar": {"svg": "<svg/>", "size": 16}, "titleTone": "yellow"},
            {"title": "docker", "avatar": {"svg": "<svg/>"}},
            {"title": "big", "avatar": {"svg": "<svg/>", "size": 90.5}},
            {"title": "tiny", "avatar": {"icon": "file", "size": 2}},
            {"title": "text", "avatar": "file"},
        ]
    }));
    let Block::List { items, .. } = Block::parse(&block) else { panic!("not a list") };
    let sizes: Vec<Option<f64>> = items.iter().map(|i| i.avatar_size).collect();
    assert_eq!(sizes, [Some(16.0), None, Some(40.0), Some(10.0), None]);
    assert_eq!(items[0].title_tone.as_deref(), Some("yellow"));
    assert_eq!(items[1].title_tone, None);
}
