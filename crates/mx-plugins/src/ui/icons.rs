use std::path::{Path, PathBuf};

use mx_theme::{Color, Palette};

pub const ICONS: [(&str, &str); 66] = [
    ("check", "check_circle_outline"),
    ("error", "error_outline"),
    ("warning", "warning_amber_outlined"),
    ("info", "info_outline"),
    ("file", "description_outlined"),
    ("folder", "folder_outlined"),
    ("play", "play_arrow"),
    ("terminal", "terminal"),
    ("link", "link"),
    ("star", "star_outline"),
    ("clock", "schedule"),
    ("user", "person_outline"),
    ("task", "task_alt"),
    ("continue", "play_arrow_rounded"),
    ("pause", "pause_rounded"),
    ("step-over", "redo_rounded"),
    ("step-into", "south_rounded"),
    ("step-out", "north_rounded"),
    ("reload", "bolt_rounded"),
    ("restart", "replay_rounded"),
    ("stop", "stop_rounded"),
    ("devtools", "travel_explore_rounded"),
    ("bug", "bug_report_outlined"),
    ("device", "smartphone_outlined"),
    ("refresh", "refresh_rounded"),
    ("clear", "block_rounded"),
    ("copy", "copy_rounded"),
    ("search", "search_rounded"),
    ("settings", "settings_outlined"),
    ("calendar", "calendar_today_outlined"),
    ("report", "summarize_outlined"),
    ("receipt", "receipt_long_outlined"),
    ("open", "open_in_new_rounded"),
    ("add", "add_rounded"),
    ("remove", "remove_rounded"),
    ("undo", "undo_rounded"),
    ("branch", "alt_route_rounded"),
    ("commit", "commit_rounded"),
    ("push", "upload_rounded"),
    ("pull", "download_rounded"),
    ("sync", "sync_rounded"),
    ("server", "dns_outlined"),
    ("edit", "edit_outlined"),
    ("dot", "circle"),
    ("box", "inventory_2_outlined"),
    ("stack", "layers_outlined"),
    ("image", "album_outlined"),
    ("drive", "storage_rounded"),
    ("network", "lan_outlined"),
    ("cpu", "memory_rounded"),
    ("trash", "delete_outline_rounded"),
    ("globe", "public_rounded"),
    ("more", "more_horiz_rounded"),
    ("chevron", "chevron_right_rounded"),
    ("filter", "filter_list_rounded"),
    ("layers", "layers_outlined"),
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

pub const GENERIC_ICON: &str = "extension_outlined";

pub fn material_icon(name: &str) -> Option<&'static str> {
    ICONS.iter().find(|(n, _)| *n == name).map(|(_, m)| *m)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Glyph {
    Svg(PathBuf),
    Named(&'static str),
    Generic,
}

pub fn svg_of(icon: &str, dir: &Path) -> Option<PathBuf> {
    if !icon.to_lowercase().ends_with(".svg") || icon.contains("..") {
        return None;
    }
    let root = std::path::absolute(dir).ok()?;
    let file = std::path::absolute(dir.join(icon)).ok()?;
    (file.starts_with(&root) && file.is_file()).then_some(file)
}

pub fn glyph(icon: Option<&str>, dir: Option<&Path>) -> Glyph {
    let Some(icon) = icon else { return Glyph::Generic };
    if let Some(file) = dir.and_then(|d| svg_of(icon, d)) {
        return Glyph::Svg(file);
    }
    material_icon(icon).map_or(Glyph::Generic, Glyph::Named)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Hex(Option<Color>),
    Green,
    Red,
    Yellow,
    Accent,
    Purple,
    Faint,
    Blue,
    Cyan,
    Magenta,
    White,
    Black,
    Dim,
}

impl Tone {
    pub fn parse(raw: Option<&str>) -> Tone {
        match raw {
            Some(hex) if hex.starts_with('#') => Tone::Hex(Color::parse_hex(hex)),
            Some("green" | "success") => Tone::Green,
            Some("red" | "error") => Tone::Red,
            Some("yellow" | "warning") => Tone::Yellow,
            Some("accent") => Tone::Accent,
            Some("purple") => Tone::Purple,
            Some("faint") => Tone::Faint,
            Some("blue") => Tone::Blue,
            Some("cyan") => Tone::Cyan,
            Some("magenta") => Tone::Magenta,
            Some("white") => Tone::White,
            Some("black") => Tone::Black,
            _ => Tone::Dim,
        }
    }

    pub fn color(self, p: &Palette) -> Color {
        match self {
            Tone::Hex(c) => c.unwrap_or(p.fg_dim),
            Tone::Green => p.green,
            Tone::Red => p.red,
            Tone::Yellow => p.yellow,
            Tone::Accent => p.accent,
            Tone::Purple => p.purple,
            Tone::Faint | Tone::Black => p.fg_faint,
            Tone::Blue => p.ansi.blue,
            Tone::Cyan => p.ansi.cyan,
            Tone::Magenta => p.ansi.magenta,
            Tone::White => p.fg,
            Tone::Dim => p.fg_dim,
        }
    }
}

pub fn tone_color(raw: Option<&str>, fallback: Color, p: &Palette) -> Color {
    match raw {
        None => fallback,
        some => Tone::parse(some).color(p),
    }
}

pub fn plugin_color(value: &str, p: &Palette) -> Option<Color> {
    let v = value.trim();
    if v.is_empty() {
        return Some(p.fg);
    }
    if v.starts_with('#') {
        return Color::parse_hex(v);
    }
    match v {
        "red" => Some(p.red),
        "green" => Some(p.green),
        "yellow" => Some(p.yellow),
        "accent" => Some(p.accent),
        "purple" => Some(p.purple),
        "faint" => Some(p.fg_faint),
        "dim" => Some(p.fg_dim),
        _ => None,
    }
}
