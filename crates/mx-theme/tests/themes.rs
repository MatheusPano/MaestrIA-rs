use std::collections::HashSet;

use mx_theme::{
    CLAUDE, Color, DEFAULT_THEME_ID, Palette, Rgb, TerminalPalette, ThemeError, Themes, built_in,
    built_ins, default_theme, is_built_in,
};

const AURORA: &str = include_str!("fixtures/aurora.json");

fn aurora_with_id(id: &str) -> Palette {
    let mut json: serde_json::Value = serde_json::from_str(AURORA).unwrap();
    json["id"] = id.into();
    Palette::from_json(&json).unwrap()
}

#[test]
fn ids_are_unique_and_follow_the_picker_order() {
    let ids: Vec<&str> = built_ins().iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
    assert_eq!(
        ids,
        [
            "maestria",
            "ayu-dark",
            "claude-dark",
            "cobalt",
            "everforest-dark",
            "high-contrast",
            "night-owl",
            "gruvbox-dark",
            "solarized-dark",
            "graphite",
            "synthwave-84",
            "slate",
            "monokai-pro",
            "nord",
            "dracula",
            "zenburn",
            "github-light",
            "rose-pine-dawn",
            "gruvbox-light",
        ]
    );
}

#[test]
fn there_are_at_least_three_light_themes_and_no_two_share_a_window() {
    assert!(built_ins().iter().filter(|t| !t.dark).count() >= 3);
    let windows: HashSet<_> = built_ins()
        .iter()
        .map(|t| (t.canvas, t.bg, t.bg_sidebar))
        .collect();
    assert_eq!(windows.len(), built_ins().len());
}

#[test]
fn dark_backgrounds_span_more_than_five_times_in_luminance() {
    let lum: Vec<f64> = built_ins()
        .iter()
        .filter(|t| t.dark)
        .map(|t| t.bg.luminance())
        .collect();
    let max = lum.iter().cloned().fold(f64::MIN, f64::max);
    let min = lum.iter().cloned().fold(f64::MAX, f64::min);
    assert!(max / min > 5.0);
}

#[test]
fn at_least_one_dark_window_is_a_neutral_grey() {
    assert!(
        built_ins()
            .iter()
            .any(|t| t.dark && t.bg.r == t.bg.g && t.bg.g == t.bg.b)
    );
}

#[test]
fn an_unknown_or_missing_id_falls_back_to_the_default() {
    let themes = Themes::new();
    assert_eq!(themes.by_id(Some("nord")).id, "nord");
    assert_eq!(
        themes.by_id(Some("theme-from-the-future")).id,
        DEFAULT_THEME_ID
    );
    assert_eq!(themes.by_id(None).id, DEFAULT_THEME_ID);
}

#[test]
fn a_retired_id_lands_on_its_nearest_look_alike() {
    let themes = Themes::new();
    assert_eq!(themes.by_id(Some("tokyo-night")).id, "maestria");
    assert_eq!(themes.by_id(Some("chatgpt-dark")).id, "graphite");
}

#[test]
fn without_the_catppuccin_plugin_its_ids_land_on_the_default() {
    let themes = Themes::new();
    assert_eq!(themes.by_id(Some("catppuccin-mocha")).id, DEFAULT_THEME_ID);
    assert_eq!(themes.by_id(Some("catppuccin-frappe")).id, DEFAULT_THEME_ID);
    assert!(!is_built_in("catppuccin-mocha"));
}

#[test]
fn a_plugin_theme_named_catppuccin_mocha_answers_for_its_retired_aliases_too() {
    let mut themes = Themes::new();
    let mocha = themes.register(aurora_with_id("catppuccin-mocha")).unwrap();
    for id in ["catppuccin-mocha", "catppuccin-frappe", "rose-pine"] {
        assert_eq!(themes.by_id(Some(id)), mocha, "{id}");
    }
}

#[test]
fn a_plugin_theme_is_found_by_id_and_cleared_on_reload() {
    let mut themes = Themes::new();
    themes.register(aurora_with_id("exemplo-aurora")).unwrap();
    assert_eq!(themes.by_id(Some("exemplo-aurora")).label, "Aurora");
    assert_eq!(themes.by_id(Some("nao-existe")).id, DEFAULT_THEME_ID);
    assert_eq!(themes.extra().len(), 1);
    themes.clear_extra();
    assert!(themes.extra().is_empty());
    assert_eq!(themes.by_id(Some("exemplo-aurora")).id, DEFAULT_THEME_ID);
}

#[test]
fn a_plugin_may_not_take_a_built_in_or_retired_id() {
    let mut themes = Themes::new();
    for id in ["nord", "tokyo-night"] {
        let err = themes.register(aurora_with_id(id)).unwrap_err();
        assert_eq!(err, ThemeError::BuiltInId(id.into()));
        assert_eq!(
            err.to_string(),
            format!("\"{id}\" é o id de um tema embutido")
        );
    }
    assert!(themes.extra().is_empty());
}

#[test]
fn two_plugins_may_not_register_the_same_theme_id() {
    let mut themes = Themes::new();
    themes.register(aurora_with_id("exemplo-aurora")).unwrap();
    let err = themes
        .register(aurora_with_id("exemplo-aurora"))
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "outro plugin já tem um tema \"exemplo-aurora\""
    );
    assert_eq!(themes.extra().len(), 1);
}

#[test]
fn no_built_in_uses_its_yellow_or_purple_as_accent() {
    for t in built_ins() {
        assert_ne!(t.accent, t.yellow, "{}", t.id);
        assert_ne!(t.accent, t.purple, "{}", t.id);
    }
}

#[test]
fn the_pty_inherits_the_pane_it_sits_in() {
    let nord = built_in("nord").unwrap();
    assert_eq!(nord.terminal().background, nord.bg.rgb());
    let light = built_in("github-light").unwrap();
    assert_eq!(light.terminal().foreground, light.fg.rgb());
}

#[test]
fn the_terminal_palette_derives_cursor_selection_search_and_bright_fallback() {
    let nord = built_in("nord").unwrap();
    let term = nord.terminal();
    assert_eq!(term.cursor, nord.accent.rgb());
    assert_eq!(term.selection, Color::from_argb(0x4D88C0D0));
    assert_eq!(term.search_hit_background, nord.yellow.rgb());
    assert_eq!(term.search_hit_background_current, nord.accent.rgb());
    assert_eq!(term.search_hit_foreground, nord.canvas.rgb());
    assert_eq!(term.ansi[8], Rgb::hex(0x4C566A));
    assert_eq!(term.ansi[9], Rgb::hex(0xBF616A));
    assert_eq!(term.ansi[14], Rgb::hex(0x8FBCBB));
}

#[test]
fn the_default_terminal_palette_is_the_maestria_theme() {
    let p = TerminalPalette::default();
    assert_eq!(p, default_theme().terminal());
    assert_eq!(p.foreground, Rgb::hex(0xE6E8EB));
    assert_eq!(p.background, Rgb::hex(0x16181D));
    assert_eq!(p.cursor, Rgb::hex(0x7AA2F7));
    assert_eq!(p.ansi[15], Rgb::hex(0xFFFFFF));
}

#[test]
fn xterm_cube_and_grey_ramp_follow_the_256_colour_table() {
    let p = TerminalPalette::default();
    assert_eq!(p.indexed(1), Rgb::hex(0xF07178));
    assert_eq!(p.indexed(16), Rgb::hex(0x000000));
    assert_eq!(p.indexed(196), Rgb::hex(0xFF0000));
    assert_eq!(p.indexed(231), Rgb::hex(0xFFFFFF));
    assert_eq!(p.indexed(232), Rgb::hex(0x080808));
    assert_eq!(p.indexed(255), Rgb::hex(0xEEEEEE));
}

#[test]
fn shadow_is_heavier_on_dark_themes() {
    assert_eq!(default_theme().shadow(), Color::from_argb(0x59000000));
    assert_eq!(
        built_in("github-light").unwrap().shadow(),
        Color::from_argb(0x14101828)
    );
}

#[test]
fn text_on_accent_is_canvas_on_bright_accents_and_white_otherwise() {
    let cobalt = built_in("cobalt").unwrap();
    assert_eq!(cobalt.on_accent(), cobalt.canvas);
    assert_eq!(built_in("github-light").unwrap().on_accent(), Color::WHITE);
}

#[test]
fn group_tints_come_from_ansi_and_are_picked_by_utf16_sum_of_the_id() {
    let t = default_theme();
    assert_eq!(
        t.group_tints(),
        [
            t.ansi.cyan,
            t.ansi.magenta,
            t.ansi.blue,
            t.ansi.bright_green()
        ]
    );
    assert_eq!(t.group_tint("a"), t.group_tints()[97 % 4]);
    assert_eq!(t.group_tint("ab"), t.group_tints()[(97 + 98) % 4]);
    assert_eq!(t.group_tint("😀"), t.group_tints()[(0xD83D + 0xDE00) % 4]);
}

#[test]
fn the_claude_mark_colour_is_fixed() {
    assert_eq!(CLAUDE.to_hex(), "#D97757");
}
