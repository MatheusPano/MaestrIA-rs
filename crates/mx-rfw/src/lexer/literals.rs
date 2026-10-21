use super::{Lexer, Mode, Tok, describe, is_ident_char, is_ident_start, is_symbol, is_terminator};
use crate::error::ParseError;

impl Lexer {
    pub(super) fn integer(&mut self, c: Option<char>) -> Result<(), ParseError> {
        if is_terminator(c) {
            let tok = self.take_int(10)?;
            self.end_with(tok, c);
            return Ok(());
        }
        let suffix = match self.mode {
            Mode::Zero => " after zero",
            Mode::MinusInteger => " after negative zero",
            _ => "",
        };
        match c {
            Some('.') => {
                self.buf.push('.');
                self.mode = Mode::NumericDot;
            }
            Some(d) if d.is_ascii_digit() => {
                self.buf.push(d);
                self.mode = Mode::Integer;
            }
            Some(e @ ('e' | 'E')) => {
                self.buf.push(e);
                self.mode = Mode::E;
            }
            Some('x' | 'X') if self.mode == Mode::Zero => {
                self.buf.clear();
                self.mode = Mode::X;
            }
            _ => return Err(self.unexpected(c, "", suffix)),
        }
        Ok(())
    }

    pub(super) fn fraction(&mut self, c: Option<char>) -> Result<(), ParseError> {
        if is_terminator(c) {
            let tok = self.take_double()?;
            self.end_with(tok, c);
            return Ok(());
        }
        match c {
            Some(d) if d.is_ascii_digit() => self.buf.push(d),
            Some(e @ ('e' | 'E')) if self.mode == Mode::Fraction => {
                self.buf.push(e);
                self.mode = Mode::E;
            }
            _ if self.mode == Mode::Fraction => return Err(self.unexpected(c, "", " in fraction component")),
            _ => return Err(self.unexpected(c, "", " in exponent")),
        }
        Ok(())
    }

    pub(super) fn exponent_start(&mut self, c: Option<char>) -> Result<(), ParseError> {
        let first = self.mode == Mode::E;
        match c {
            Some('-') if first => {
                self.buf.push('-');
                self.mode = Mode::NegativeExponent;
                Ok(())
            }
            Some(d) if d.is_ascii_digit() => {
                self.buf.push(d);
                self.mode = Mode::Exponent;
                Ok(())
            }
            _ if first => Err(self.unexpected(
                c,
                "Unexpected end of file after exponent separator",
                " after exponent separator",
            )),
            _ => Err(self.unexpected(
                c,
                "Unexpected end of file after exponent separator and minus sign",
                " in exponent",
            )),
        }
    }

    pub(super) fn hex(&mut self, c: Option<char>) -> Result<(), ParseError> {
        let started = self.mode == Mode::Hex;
        match c {
            Some(h) if h.is_ascii_hexdigit() => {
                self.buf.push(h);
                self.mode = Mode::Hex;
                Ok(())
            }
            _ if started && is_terminator(c) => {
                let tok = self.take_int(16)?;
                self.end_with(tok, c);
                Ok(())
            }
            _ if started => Err(self.unexpected(c, "", " in hex literal")),
            _ => Err(self.unexpected(c, "Unexpected end of file after 0x prefix", " after 0x prefix")),
        }
    }

    pub(super) fn string(&mut self, c: Option<char>, quote: char) -> Result<(), ParseError> {
        match c {
            None => return Err(self.err("Unexpected end of file inside string")),
            Some('\n') => return Err(self.err("Unexpected end of line inside string")),
            Some(q) if q == quote => {
                let text = String::from_utf16_lossy(&std::mem::take(&mut self.text));
                self.emit(Tok::Str(text));
                self.mode = Mode::EndQuote;
            }
            Some('\\') => self.mode = Mode::Escape(quote),
            Some(ch) => self.push_text(ch),
        }
        Ok(())
    }

    pub(super) fn escape(&mut self, c: Option<char>, quote: char) -> Result<(), ParseError> {
        let unit = match c {
            None => return Err(self.err("Unexpected end of file inside string")),
            Some(ch @ ('"' | '\'' | '\\' | '/')) => ch as u16,
            Some('b') => 0x08,
            Some('f') => 0x0C,
            Some('n') => 0x0A,
            Some('r') => 0x0D,
            Some('t') => 0x09,
            Some('u') => {
                self.unicode.clear();
                self.mode = Mode::Unicode(quote, 0);
                return Ok(());
            }
            Some(ch) => {
                return Err(self.err(format!("Unexpected character {} after backslash in string", describe(ch))));
            }
        };
        self.text.push(unit);
        self.mode = Mode::Str(quote);
        Ok(())
    }

    pub(super) fn dot1(&mut self, c: Option<char>) -> Result<(), ParseError> {
        match c {
            Some('.') => {
                self.mode = Mode::Dot2;
                return Ok(());
            }
            None => {
                self.emit(Tok::Symbol('.'));
                self.emit(Tok::Eof);
                return Ok(());
            }
            Some(ch) if !(ch == ' ' || ch == '\n' || ch == '"' || ch == '\'' || is_symbol(ch) || is_ident_char(ch)) => {
                return Err(self.err(format!("Unexpected character {} after period", describe(ch))));
            }
            Some(_) => self.emit(Tok::Symbol('.')),
        }
        let Some(ch) = c else { return Ok(()) };
        self.mode = match ch {
            ' ' | '\n' => Mode::Main,
            '"' | '\'' => Mode::Str(ch),
            d if d.is_ascii_digit() => {
                self.buf.push(d);
                Mode::IntegerOnly
            }
            i if is_ident_start(i) => {
                self.buf.push(i);
                Mode::Identifier
            }
            s => {
                self.emit(Tok::Symbol(s));
                Mode::Main
            }
        };
        Ok(())
    }

    pub(super) fn identifier(&mut self, c: Option<char>) -> Result<(), ParseError> {
        match c {
            _ if is_terminator(c) => {
                let tok = Tok::Ident(self.buf.clone());
                self.end_with(tok, c);
            }
            Some('.') => {
                let tok = Tok::Ident(std::mem::take(&mut self.buf));
                self.emit(tok);
                self.mode = Mode::Dot1;
            }
            Some(i) if is_ident_char(i) => self.buf.push(i),
            _ => return Err(self.unexpected(c, "", " inside identifier")),
        }
        Ok(())
    }
}
