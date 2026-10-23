use gpui::Keystroke;
use mx_plugins::chord::{Chord, Key, KeyPress};

use crate::chrome::settings::{self, ChordCheck};

pub const LINUX: bool = cfg!(target_os = "linux");

pub const UNUSABLE_KEY: &str = "essa tecla não dá pra usar num atalho";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pressed {
    Modifier,
    Unknown,
    Key(Key),
}

pub fn key_of(name: &str) -> Pressed {
    match name {
        "shift" | "control" | "ctrl" | "alt" | "platform" | "super" | "meta" | "cmd" | "fn" | "function"
        | "capslock" | "caps_lock" => Pressed::Modifier,
        "insert" => Pressed::Key(Key::Insert),
        _ => Key::named(name).map_or(Pressed::Unknown, Pressed::Key),
    }
}

pub fn chord_id(c: &Chord) -> String {
    let name = match c.key {
        Key::Char(' ') => "space".to_owned(),
        Key::Char('+') => "plus".to_owned(),
        Key::Char(ch) => ch.to_lowercase().collect(),
        Key::Tab => "tab".into(),
        Key::Enter => "enter".into(),
        Key::Backspace => "backspace".into(),
        Key::Delete => "delete".into(),
        Key::Escape => "escape".into(),
        Key::Left => "left".into(),
        Key::Right => "right".into(),
        Key::Up => "up".into(),
        Key::Down => "down".into(),
        Key::Home => "home".into(),
        Key::End => "end".into(),
        Key::PageUp => "pageup".into(),
        Key::PageDown => "pagedown".into(),
        Key::Insert => "insert".into(),
        Key::F(n) => format!("f{n}"),
    };
    let mut parts: Vec<String> = Vec::new();
    for (on, m) in [(c.control, "ctrl"), (c.alt, "alt"), (c.shift, "shift"), (c.meta, "meta")] {
        if on {
            parts.push(m.into());
        }
    }
    parts.push(name);
    parts.join("+")
}

pub fn is_valid(c: &Chord, linux: bool) -> bool {
    c.rejection(linux).is_none()
}

pub fn press(key: Key, ctrl: bool, alt: bool, shift: bool, meta: bool) -> KeyPress {
    KeyPress { key, physical: None, meta, control: ctrl, alt, shift }
}

pub fn press_of(k: &Keystroke) -> Option<KeyPress> {
    let m = k.modifiers;
    match key_of(&k.key) {
        Pressed::Key(key) => Some(press(key, m.control, m.alt, m.shift, m.platform)),
        _ => None,
    }
}

pub fn chord_of(p: &KeyPress) -> Chord {
    Chord { key: p.key, meta: p.meta, control: p.control, alt: p.alt, shift: p.shift }
}

pub fn check_chord(p: &settings::KeyPress) -> ChordCheck {
    check_chord_for(p, LINUX)
}

pub fn check_chord_for(p: &settings::KeyPress, linux: bool) -> ChordCheck {
    let key = match key_of(&p.key) {
        Pressed::Modifier => return ChordCheck::Ignore,
        Pressed::Unknown => return ChordCheck::Rejected(UNUSABLE_KEY.into()),
        Pressed::Key(k) => k,
    };
    let chord = Chord { key, meta: p.meta, control: p.ctrl, alt: p.alt, shift: p.shift };
    match chord.rejection(linux) {
        Some(why) => ChordCheck::Rejected(why),
        None => ChordCheck::Accepted { id: chord_id(&chord), label: chord.label(linux) },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalKey {
    Newline,
    Paste,
    Copy,
    SelectAll,
}

pub fn copy_chord(linux: bool) -> Chord {
    let k = Key::Char('c');
    if linux { Chord { key: k, meta: false, control: true, alt: false, shift: true } } else { Chord { meta: true, ..Chord::new(k) } }
}

pub fn paste_chords(linux: bool) -> Vec<Chord> {
    let v = Key::Char('v');
    if !linux {
        return vec![Chord { meta: true, ..Chord::new(v) }];
    }
    vec![
        Chord { control: true, shift: true, ..Chord::new(v) },
        Chord { control: true, ..Chord::new(v) },
        Chord { shift: true, ..Chord::new(Key::Insert) },
    ]
}

pub fn select_all_chord(linux: bool) -> Chord {
    let a = Key::Char('a');
    if linux { Chord { control: true, shift: true, ..Chord::new(a) } } else { Chord { meta: true, ..Chord::new(a) } }
}

pub fn terminal_key(p: &KeyPress, linux: bool) -> Option<TerminalKey> {
    if !(p.control || p.alt || p.shift || p.meta) {
        return None;
    }
    if (Chord { shift: true, ..Chord::new(Key::Enter) }).accepts(p) {
        return Some(TerminalKey::Newline);
    }
    if paste_chords(linux).iter().any(|c| c.accepts(p)) {
        return Some(TerminalKey::Paste);
    }
    if copy_chord(linux).accepts(p) {
        return Some(TerminalKey::Copy);
    }
    select_all_chord(linux).accepts(p).then_some(TerminalKey::SelectAll)
}

pub fn slot_of(p: &KeyPress, linux: bool) -> Option<usize> {
    (0..9u8).find(|i| Chord::slot(*i, linux).accepts(p)).map(usize::from)
}

pub fn may_be_shortcut(p: &KeyPress) -> bool {
    p.control || p.alt || p.meta || matches!(p.key, Key::F(_))
}

pub fn is_editing_key(key: Key) -> bool {
    matches!(
        key,
        Key::Backspace
            | Key::Delete
            | Key::Left
            | Key::Right
            | Key::Up
            | Key::Down
            | Key::Home
            | Key::End
            | Key::Char('a' | 'c' | 'v' | 'x' | 'z')
    )
}
