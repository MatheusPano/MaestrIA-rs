#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Tab,
    Enter,
    Backspace,
    Delete,
    Escape,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    F(u8),
}

const NAMED: [(Key, &str); 13] = [
    (Key::Tab, "tab"),
    (Key::Enter, "enter"),
    (Key::Backspace, "backspace"),
    (Key::Delete, "delete"),
    (Key::Escape, "escape"),
    (Key::Left, "left"),
    (Key::Right, "right"),
    (Key::Up, "up"),
    (Key::Down, "down"),
    (Key::Home, "home"),
    (Key::End, "end"),
    (Key::PageUp, "pageup"),
    (Key::PageDown, "pagedown"),
];

impl Key {
    pub fn named(name: &str) -> Option<Key> {
        let mut chars = name.chars();
        if let (Some(c), None) = (chars.next(), chars.next())
            && c.len_utf16() == 1
        {
            return Some(Key::Char(c.to_ascii_lowercase()));
        }
        match name {
            "space" => return Some(Key::Char(' ')),
            "plus" => return Some(Key::Char('+')),
            _ => {}
        }
        if let Some(n) = name.strip_prefix('f').and_then(|n| n.parse::<u8>().ok())
            && (1..=12).contains(&n)
            && name == format!("f{n}")
        {
            return Some(Key::F(n));
        }
        NAMED.iter().find(|(_, n)| *n == name).map(|(k, _)| *k)
    }

    fn same(self, other: Key) -> bool {
        match (self, other) {
            (Key::Char(a), Key::Char(b)) => a.to_lowercase().eq(b.to_lowercase()),
            _ => self == other,
        }
    }

    fn known(self) -> bool {
        !matches!(self, Key::Insert)
    }

    fn is_letter(self) -> bool {
        matches!(self, Key::Char(c) if c.is_ascii_lowercase())
    }

    pub fn physical_usage(self) -> Option<u32> {
        match self {
            Key::Char(c @ 'a'..='z') => Some(0x04 + (c as u32 - 'a' as u32)),
            Key::Char(c @ '1'..='9') => Some(0x1e + (c as u32 - '1' as u32)),
            Key::Char('0') => Some(0x27),
            _ => None,
        }
    }

    fn mac_glyph(self) -> String {
        match self {
            Key::Char(' ') => "␣".into(),
            Key::Char(c) => c.to_uppercase().collect(),
            Key::Tab => "⇥".into(),
            Key::Enter => "⏎".into(),
            Key::Backspace => "⌫".into(),
            Key::Delete => "⌦".into(),
            Key::Escape => "⎋".into(),
            Key::Left => "←".into(),
            Key::Right => "→".into(),
            Key::Up => "↑".into(),
            Key::Down => "↓".into(),
            Key::Home => "↖".into(),
            Key::End => "↘".into(),
            Key::PageUp => "⇞".into(),
            Key::PageDown => "⇟".into(),
            Key::Insert => "INSERT".into(),
            Key::F(n) => format!("F{n}"),
        }
    }

    fn linux_glyph(self) -> String {
        match self {
            Key::Tab => "Tab".into(),
            Key::Enter => "Enter".into(),
            Key::Char(' ') => "Space".into(),
            Key::Backspace => "Backspace".into(),
            Key::Delete => "Del".into(),
            Key::Escape => "Esc".into(),
            Key::Home => "Home".into(),
            Key::End => "End".into(),
            Key::PageUp => "PgUp".into(),
            Key::PageDown => "PgDn".into(),
            Key::Insert => "Insert".into(),
            other => other.mac_glyph(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub key: Key,
    pub meta: bool,
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPress {
    pub key: Key,
    pub physical: Option<u32>,
    pub meta: bool,
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Chord {
    pub const fn new(key: Key) -> Self {
        Chord { key, meta: false, control: false, alt: false, shift: false }
    }

    const fn mods(key: Key, control: bool, shift: bool, alt: bool, meta: bool) -> Self {
        Chord { key, meta, control, alt, shift }
    }

    pub fn parse(id: &str) -> Option<Chord> {
        if id.is_empty() {
            return None;
        }
        let mut parts: Vec<&str> = id.split('+').collect();
        let key = Key::named(parts.pop()?)?;
        if parts.iter().any(|p| !matches!(*p, "ctrl" | "alt" | "shift" | "meta")) {
            return None;
        }
        let has = |m: &str| parts.contains(&m);
        Some(Chord::mods(key, has("ctrl"), has("shift"), has("alt"), has("meta")))
    }

    pub fn for_host(self, linux: bool) -> Chord {
        if linux && self.meta {
            Chord::mods(self.key, true, true, self.alt, false)
        } else {
            self
        }
    }

    pub fn label(&self, linux: bool) -> String {
        if linux {
            let mut parts: Vec<String> = Vec::new();
            for (on, name) in [
                (self.control, "Ctrl"),
                (self.shift, "Shift"),
                (self.alt, "Alt"),
                (self.meta, "Super"),
            ] {
                if on {
                    parts.push(name.into());
                }
            }
            parts.push(self.key.linux_glyph());
            return parts.join("+");
        }
        let mut s = String::new();
        for (on, glyph) in [(self.control, '⌃'), (self.alt, '⌥'), (self.shift, '⇧'), (self.meta, '⌘')] {
            if on {
                s.push(glyph);
            }
        }
        s + &self.key.mac_glyph()
    }

    pub fn rejection(&self, linux: bool) -> Option<String> {
        if !self.key.known() {
            return Some("essa tecla não dá pra usar num atalho".into());
        }
        if let Some(why) = reserved(linux).into_iter().find(|(c, _)| c == self).map(|(_, w)| w) {
            return Some(why.into());
        }
        if !self.meta && !self.control && !self.alt && !matches!(self.key, Key::F(_)) {
            return Some(if linux {
                "precisa de Ctrl ou Alt — sem modificador a tecla sumiria do terminal".into()
            } else {
                "precisa de ⌘, ⌃ ou ⌥ — sem modificador a tecla sumiria do terminal".into()
            });
        }
        if self.control && !self.shift && !self.meta && !self.alt && self.key.is_letter() {
            return Some(format!(
                "o terminal manda {} pro processo como código de controle",
                self.label(linux)
            ));
        }
        None
    }

    fn same_mods(&self, ev: &KeyPress) -> bool {
        self.meta == ev.meta && self.control == ev.control && self.alt == ev.alt && self.shift == ev.shift
    }

    pub fn accepts(&self, ev: &KeyPress) -> bool {
        self.key.same(ev.key) && self.same_mods(ev)
    }

    pub fn hits(&self, ev: &KeyPress) -> bool {
        if self.accepts(ev) {
            return true;
        }
        match (self.key.physical_usage(), ev.physical) {
            (Some(want), Some(got)) => want == got && self.same_mods(ev),
            _ => false,
        }
    }
}

fn reserved(linux: bool) -> Vec<(Chord, &'static str)> {
    let c = |ch: char| Key::Char(ch);
    if !linux {
        return vec![
            (Chord::mods(c('q'), false, false, false, true), "o macOS encerra o app com ⌘Q"),
            (Chord::mods(c('w'), false, false, false, true), "o macOS fecha a janela com ⌘W"),
            (Chord::mods(c('h'), false, false, false, true), "o macOS esconde o app com ⌘H"),
            (Chord::mods(c('m'), false, false, false, true), "o macOS minimiza a janela com ⌘M"),
            (Chord::mods(c('f'), true, false, false, true), "o macOS usa ⌃⌘F pra tela cheia"),
            (
                Chord::mods(c('h'), false, false, true, true),
                "o macOS usa ⌥⌘H pra esconder os outros apps",
            ),
            (Chord::mods(c('v'), false, false, false, true), "o painel cola com ⌘V"),
            (Chord::mods(c('c'), false, false, false, true), "o painel copia a seleção com ⌘C"),
        ];
    }
    let mut out = vec![
        (Chord::mods(c('c'), true, true, false, false), "o painel copia a seleção com Ctrl+Shift+C"),
        (Chord::mods(c('v'), true, true, false, false), "o painel cola com Ctrl+Shift+V"),
        (
            Chord::mods(c('e'), true, true, false, false),
            "o IBus abre o seletor de emoji com Ctrl+Shift+E",
        ),
        (
            Chord::mods(c('u'), true, true, false, false),
            "o IBus usa Ctrl+Shift+U pra digitar um caractere pelo código",
        ),
        (Chord::mods(c('t'), true, false, true, false), "o desktop abre um terminal com Ctrl+Alt+T"),
        (Chord::mods(Key::Delete, true, false, true, false), "o desktop usa Ctrl+Alt+Del"),
    ];
    for arrow in [Key::Left, Key::Right, Key::Up, Key::Down] {
        out.push((
            Chord::mods(arrow, true, false, true, false),
            "o desktop troca de área de trabalho com Ctrl+Alt+setas",
        ));
    }
    out.push((Chord::mods(Key::F(4), false, false, true, false), "o desktop fecha a janela com Alt+F4"));
    out.push((Chord::mods(Key::Tab, false, false, true, false), "o desktop alterna as janelas com Alt+Tab"));
    out
}
