pub(crate) fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let text = text.replace("\r\n", "\r").replace('\n', "\r");
    if !bracketed {
        return text.replace('\x1b', "").into_bytes();
    }
    let mut out = Vec::with_capacity(text.len() + 12);
    out.extend_from_slice(b"\x1b[200~");
    for c in text.chars().filter(|c| !matches!(c, '\x1b' | '\x03' | '\u{9b}')) {
        let mut buf = [0u8; 4];
        out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
    }
    out.extend_from_slice(b"\x1b[201~");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bracketed_paste_cannot_close_the_bracket_early() {
        let out = paste_bytes("x\x1b[201~rm -rf ~\r", true);
        assert_eq!(out, b"\x1b[200~x[201~rm -rf ~\r\x1b[201~");
    }

    #[test]
    fn bracketed_paste_drops_ctrl_c_and_c1_csi() {
        assert_eq!(paste_bytes("a\x03b\u{9b}201~c", true), "\x1b[200~ab201~c\x1b[201~".as_bytes());
    }

    #[test]
    fn plain_paste_drops_escape_and_keeps_text() {
        assert_eq!(paste_bytes("ls\x1b[A\n", false), b"ls[A\r");
    }

    #[test]
    fn newlines_become_carriage_returns() {
        assert_eq!(paste_bytes("a\r\nb\nc", false), b"a\rb\rc");
        assert_eq!(paste_bytes("a\nb", true), b"\x1b[200~a\rb\x1b[201~");
    }

    #[test]
    fn accents_survive() {
        assert_eq!(paste_bytes("ação", true), "\x1b[200~ação\x1b[201~".as_bytes());
    }
}
