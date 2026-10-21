use std::borrow::Cow;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub platform: bool,
}

impl Mods {
    fn none(self) -> bool {
        !(self.ctrl || self.alt || self.shift || self.platform)
    }

    fn xterm_code(self) -> u8 {
        1 + u8::from(self.shift) + 2 * u8::from(self.alt) + 4 * u8::from(self.ctrl)
    }
}

pub fn encode_key(
    key: &str,
    key_char: Option<&str>,
    mods: Mods,
    app_cursor: bool,
) -> Option<Cow<'static, [u8]>> {
    if mods.platform {
        return None;
    }
    if let Some(seq) = special(key, mods, app_cursor) {
        return Some(seq);
    }
    if mods.ctrl
        && !mods.alt
        && let Some(byte) = ctrl_byte(key)
    {
        return Some(Cow::Owned(vec![byte]));
    }
    if mods.alt && key.len() == 1 && key.is_ascii() {
        let k = key.as_bytes()[0];
        let byte = if mods.ctrl {
            ctrl_byte(key)?
        } else if mods.shift {
            k.to_ascii_uppercase()
        } else {
            k
        };
        return Some(Cow::Owned(vec![0x1b, byte]));
    }
    if mods.ctrl {
        return None;
    }
    key_char
        .filter(|s| !s.is_empty())
        .map(|s| Cow::Owned(s.as_bytes().to_vec()))
}

fn ctrl_byte(key: &str) -> Option<u8> {
    let mut chars = key.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return match key {
            "space" => Some(0),
            _ => None,
        };
    }
    match c.to_ascii_lowercase() {
        c @ 'a'..='z' => Some(c as u8 - b'a' + 1),
        '@' | '2' => Some(0),
        '[' | '3' => Some(0x1b),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '-' | '7' => Some(0x1f),
        '?' | '8' => Some(0x7f),
        _ => None,
    }
}

fn special(key: &str, mods: Mods, app_cursor: bool) -> Option<Cow<'static, [u8]>> {
    let fixed: Option<&'static [u8]> = match (key, mods.none()) {
        ("enter", true) => Some(b"\r"),
        ("enter", false) if mods.alt && !mods.ctrl => Some(b"\x1b\r"),
        ("enter", false) if mods.shift && !mods.ctrl => Some(b"\n"),
        ("tab", true) => Some(b"\t"),
        ("tab", false) if mods.shift && !mods.ctrl && !mods.alt => Some(b"\x1b[Z"),
        ("escape", true) => Some(b"\x1b"),
        ("backspace", true) => Some(b"\x7f"),
        ("backspace", false) if mods.ctrl && !mods.alt => Some(b"\x08"),
        ("backspace", false) if mods.alt && !mods.ctrl => Some(b"\x1b\x7f"),
        ("backspace", false) if mods.shift => Some(b"\x7f"),
        _ => None,
    };
    if let Some(seq) = fixed {
        return Some(Cow::Borrowed(seq));
    }

    let (csi_final, tilde) = match key {
        "up" => (b'A', None),
        "down" => (b'B', None),
        "right" => (b'C', None),
        "left" => (b'D', None),
        "home" => (b'H', None),
        "end" => (b'F', None),
        "f1" => (b'P', None),
        "f2" => (b'Q', None),
        "f3" => (b'R', None),
        "f4" => (b'S', None),
        "insert" => (b'~', Some(2)),
        "delete" => (b'~', Some(3)),
        "pageup" => (b'~', Some(5)),
        "pagedown" => (b'~', Some(6)),
        "f5" => (b'~', Some(15)),
        "f6" => (b'~', Some(17)),
        "f7" => (b'~', Some(18)),
        "f8" => (b'~', Some(19)),
        "f9" => (b'~', Some(20)),
        "f10" => (b'~', Some(21)),
        "f11" => (b'~', Some(23)),
        "f12" => (b'~', Some(24)),
        _ => return None,
    };
    let seq = match (tilde, mods.none()) {
        (Some(n), true) => format!("\x1b[{n}~"),
        (Some(n), false) => format!("\x1b[{n};{}~", mods.xterm_code()),
        (None, true) if key.starts_with('f') => format!("\x1bO{}", csi_final as char),
        (None, true) if app_cursor => format!("\x1bO{}", csi_final as char),
        (None, true) => format!("\x1b[{}", csi_final as char),
        (None, false) => format!("\x1b[1;{}{}", mods.xterm_code(), csi_final as char),
    };
    Some(Cow::Owned(seq.into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(key: &str, ch: Option<&str>, mods: Mods, app: bool) -> Vec<u8> {
        encode_key(key, ch, mods, app)
            .map(|c| c.into_owned())
            .unwrap_or_default()
    }

    const CTRL: Mods = Mods {
        ctrl: true,
        alt: false,
        shift: false,
        platform: false,
    };
    const ALT: Mods = Mods {
        ctrl: false,
        alt: true,
        shift: false,
        platform: false,
    };
    const SHIFT: Mods = Mods {
        ctrl: false,
        alt: false,
        shift: true,
        platform: false,
    };

    #[test]
    fn plain_and_special() {
        assert_eq!(enc("a", Some("a"), Mods::default(), false), b"a");
        assert_eq!(enc("enter", None, Mods::default(), false), b"\r");
        assert_eq!(enc("backspace", None, Mods::default(), false), b"\x7f");
        assert_eq!(enc("up", None, Mods::default(), false), b"\x1b[A");
        assert_eq!(enc("up", None, Mods::default(), true), b"\x1bOA");
        assert_eq!(enc("delete", None, Mods::default(), false), b"\x1b[3~");
        assert_eq!(enc("f1", None, Mods::default(), false), b"\x1bOP");
    }

    #[test]
    fn modified() {
        assert_eq!(enc("c", Some("c"), CTRL, false), b"\x03");
        assert_eq!(enc("space", Some(" "), CTRL, false), b"\x00");
        assert_eq!(enc("b", Some("b"), ALT, false), b"\x1bb");
        assert_eq!(enc("left", None, CTRL, false), b"\x1b[1;5D");
        assert_eq!(enc("tab", None, SHIFT, false), b"\x1b[Z");
        assert_eq!(enc("A", Some("A"), SHIFT, false), b"A");
    }
}
