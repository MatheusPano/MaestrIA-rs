const MAX_FRAGMENT: usize = 32;

#[derive(Debug, Default, Clone)]
pub struct VtScrubber {
    held: Vec<u8>,
    joined: Vec<u8>,
}

impl VtScrubber {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scrub(&mut self, chunk: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(chunk.len());
        self.scrub_into(chunk, &mut out);
        out
    }

    pub fn scrub_into(&mut self, chunk: &[u8], out: &mut Vec<u8>) {
        if self.held.is_empty() {
            Self::run(chunk, out, &mut self.held);
            return;
        }
        let mut joined = std::mem::take(&mut self.joined);
        joined.clear();
        joined.extend_from_slice(&self.held);
        joined.extend_from_slice(chunk);
        self.held.clear();
        Self::run(&joined, out, &mut self.held);
        self.joined = joined;
    }

    fn run(input: &[u8], out: &mut Vec<u8>, held: &mut Vec<u8>) {
        let len = input.len();
        let mut cursor = 0;
        while cursor < len {
            let Some(csi) = find_csi(input, cursor) else {
                let esc = if input[len - 1] == 0x1b { len - 1 } else { len };
                out.extend_from_slice(&input[cursor..esc]);
                held.extend_from_slice(&input[esc..]);
                break;
            };
            out.extend_from_slice(&input[cursor..csi]);
            if csi + 2 >= len {
                hold(&input[csi..], out, held);
                break;
            }
            if !(0x3c..=0x3e).contains(&input[csi + 2]) {
                out.extend_from_slice(b"\x1b[");
                cursor = csi + 2;
                continue;
            }
            let mut end = csi + 3;
            while end < len && !(0x40..=0x7e).contains(&input[end]) {
                end += 1;
            }
            if end == len {
                hold(&input[csi..], out, held);
                break;
            }
            if input[end] != b'm' {
                out.extend_from_slice(&input[csi..=end]);
            }
            cursor = end + 1;
        }
    }
}

fn hold(fragment: &[u8], out: &mut Vec<u8>, held: &mut Vec<u8>) {
    if fragment.len() > MAX_FRAGMENT {
        out.extend_from_slice(fragment);
    } else {
        held.extend_from_slice(fragment);
    }
}

fn find_csi(input: &[u8], from: usize) -> Option<usize> {
    input[from..]
        .windows(2)
        .position(|w| w == b"\x1b[")
        .map(|i| i + from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn scrub(s: &str) -> String {
        String::from_utf8(VtScrubber::new().scrub(s.as_bytes())).unwrap()
    }

    #[test]
    fn private_m_sequence_is_dropped() {
        assert_eq!(scrub("\x1b[>4mABC"), "ABC");
        assert_eq!(scrub("\x1b[>4;2mABC"), "ABC");
    }

    #[test]
    fn ordinary_sgr_passes_untouched() {
        let s = "a\x1b[1mb\x1b[22m\x1b[38;5;208mc\x1b[39m\r\n";
        assert_eq!(scrub(s), s);
    }

    #[test]
    fn other_private_csi_pass_untouched() {
        let s = "\x1b[>c\x1b[<u\x1b[?25lA";
        assert_eq!(scrub(s), s);
    }

    #[test]
    fn private_sequence_split_after_params_is_held() {
        let mut s = VtScrubber::new();
        assert_eq!(s.scrub(b"A\x1b[>4"), b"A");
        assert_eq!(s.scrub(b";2mB"), b"B");
    }

    #[test]
    fn lone_trailing_escape_is_held() {
        let mut s = VtScrubber::new();
        assert_eq!(s.scrub(b"A\x1b"), b"A");
        assert_eq!(s.scrub(b"[>4mB"), b"B");
    }

    #[test]
    fn fragment_longer_than_32_is_emitted_as_is() {
        let s = format!("\x1b[>{}", "9".repeat(40));
        assert_eq!(scrub(&s), s);
    }

    #[test]
    fn unterminated_ordinary_csi_is_emitted_immediately() {
        let mut s = VtScrubber::new();
        assert_eq!(s.scrub(b"\x1b[3"), b"\x1b[3");
        assert_eq!(s.scrub(b"1mX"), b"1mX");
    }

    #[derive(Debug, Clone)]
    enum Token {
        Text(String),
        Csi { private: Option<char>, params: String, fin: char },
    }

    impl Token {
        fn raw(&self) -> String {
            match self {
                Token::Text(t) => t.clone(),
                Token::Csi { private, params, fin } => {
                    format!("\x1b[{}{params}{fin}", private.map(String::from).unwrap_or_default())
                }
            }
        }

        fn expected(&self) -> String {
            match self {
                Token::Csi { private: Some(_), fin: 'm', .. } => String::new(),
                other => other.raw(),
            }
        }
    }

    fn token() -> impl Strategy<Value = Token> {
        prop_oneof![
            "[a-zA-Z0-9 \r\n]{1,6}".prop_map(Token::Text),
            (
                prop::option::of(prop::sample::select(vec!['<', '=', '>'])),
                "[0-9;]{0,8}",
                prop::sample::select(vec!['m', 'c', 'u', 'h', 'l', 'J', 'H']),
            )
                .prop_map(|(private, params, fin)| Token::Csi { private, params, fin }),
        ]
    }

    proptest! {
        #[test]
        fn chunked_stream_equals_whole_and_drops_only_private_m(
            tokens in prop::collection::vec(token(), 0..24),
            cuts in prop::collection::vec(any::<prop::sample::Index>(), 0..8),
        ) {
            let raw: String = tokens.iter().map(Token::raw).collect();
            let expected: String = tokens.iter().map(Token::expected).collect();
            let bytes = raw.as_bytes();
            let mut points: Vec<usize> = cuts.iter().map(|i| i.index(bytes.len() + 1)).collect();
            points.sort_unstable();
            let mut scrubber = VtScrubber::new();
            let mut out = Vec::new();
            let mut start = 0;
            for p in points {
                scrubber.scrub_into(&bytes[start..p], &mut out);
                start = p;
            }
            scrubber.scrub_into(&bytes[start..], &mut out);
            prop_assert_eq!(String::from_utf8(out).unwrap(), expected);
        }
    }
}
