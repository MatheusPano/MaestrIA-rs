use mx_theme::{Color, Palette, ThemeError};
use serde_json::{Value, json};

const AURORA: &str = include_str!("fixtures/aurora.json");

fn aurora() -> Value {
    serde_json::from_str(AURORA).unwrap()
}

#[test]
fn the_example_theme_reads_with_bright_colours_falling_back_to_normal() {
    let p = Palette::from_json_str(AURORA).unwrap();
    assert_eq!(p.id, "exemplo-aurora");
    assert_eq!(p.label, "Aurora");
    assert!(p.dark);
    assert_eq!(p.accent, Color::opaque(0x5EE3C1));
    assert_eq!(p.ansi.bright_black(), Color::opaque(0x5F7385));
    assert_eq!(p.ansi.bright_red(), p.ansi.red);
}

#[test]
fn a_missing_ui_colour_names_the_field() {
    let mut j = aurora();
    j.as_object_mut().unwrap().remove("bgHover");
    let err = Palette::from_json(&j).unwrap_err();
    assert_eq!(err.to_string(), "\"bgHover\" precisa ser uma cor #RRGGBB");
}

#[test]
fn a_bad_ansi_colour_names_the_field_with_the_ansi_prefix() {
    let mut j = aurora();
    j["ansi"]["cyan"] = "azul".into();
    let err = Palette::from_json(&j).unwrap_err();
    assert_eq!(err, ThemeError::BadColor("ansi.cyan".into()));
    assert_eq!(err.to_string(), "\"ansi.cyan\" precisa ser uma cor #RRGGBB");
}

#[test]
fn id_label_and_ansi_are_required() {
    for (key, value, message) in [
        ("id", json!(""), "\"id\" é obrigatório"),
        ("id", json!(3), "\"id\" é obrigatório"),
        ("label", json!(null), "\"label\" é obrigatório"),
        ("ansi", json!([]), "\"ansi\" é obrigatório"),
    ] {
        let mut j = aurora();
        j[key] = value;
        assert_eq!(
            Palette::from_json(&j).unwrap_err().to_string(),
            message,
            "{key}"
        );
    }
}

#[test]
fn a_document_that_is_not_an_object_is_rejected() {
    assert_eq!(
        Palette::from_json_str("[1]").unwrap_err().to_string(),
        "não é um objeto"
    );
    assert!(matches!(
        Palette::from_json_str("{"),
        Err(ThemeError::Syntax(_))
    ));
}

#[test]
fn dark_defaults_to_true_and_only_literal_false_makes_it_light() {
    for (value, dark) in [
        (json!(false), false),
        (json!(true), true),
        (json!("false"), true),
        (json!(0), true),
    ] {
        let mut j = aurora();
        j["dark"] = value;
        assert_eq!(Palette::from_json(&j).unwrap().dark, dark);
    }
    let mut j = aurora();
    j.as_object_mut().unwrap().remove("dark");
    assert!(Palette::from_json(&j).unwrap().dark);
}

#[test]
fn an_invalid_bright_colour_is_treated_as_absent() {
    let mut j = aurora();
    j["ansi"]["brightGreen"] = "verde".into();
    j["ansi"]["brightBlue"] = json!(42);
    j["ansi"]["brightCyan"] = "#ABCDEF".into();
    let p = Palette::from_json(&j).unwrap();
    assert_eq!(p.ansi.bright_green(), p.ansi.green);
    assert_eq!(p.ansi.bright_blue(), p.ansi.blue);
    assert_eq!(p.ansi.bright_cyan(), Color::opaque(0xABCDEF));
}

#[test]
fn hex_colours_accept_six_or_eight_digits_optional_hash_and_whitespace() {
    assert_eq!(Color::parse_hex("#0B1016"), Some(Color::opaque(0x0B1016)));
    assert_eq!(Color::parse_hex("  0b1016 "), Some(Color::opaque(0x0B1016)));
    assert_eq!(
        Color::parse_hex("#800B1016"),
        Some(Color::from_argb(0x800B1016))
    );
    assert_eq!(Color::parse_hex("#12345"), None);
    assert_eq!(Color::parse_hex("#GGGGGG"), None);
    assert_eq!(Color::parse_hex("##123456"), None);
    assert_eq!(Color::parse_hex(""), None);
}
