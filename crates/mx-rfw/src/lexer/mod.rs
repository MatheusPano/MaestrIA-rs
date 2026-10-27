use std::collections::VecDeque;
use std::fmt;

use crate::error::{ParseError, dart_double};

mod literals;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Symbol(char),
    TripleDot,
    Int(i64),
    Double(f64),
    Ident(String),
    Str(String),
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tok::Symbol(c) => write!(f, "{c}"),
            Tok::TripleDot => f.write_str("\u{2026}"),
            Tok::Int(v) => write!(f, "{v}"),
            Tok::Double(v) => f.write_str(&dart_double(*v)),
            Tok::Ident(s) => f.write_str(s),
            Tok::Str(s) => write!(f, "\"{s}\""),
            Tok::Eof => f.write_str("<EOF>"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Main,
    Minus,
    Zero,
    MinusInteger,
    Integer,
    IntegerOnly,
    NumericDot,
    Fraction,
    E,
    NegativeExponent,
    Exponent,
    X,
    Hex,
    Dot1,
    Dot2,
    Identifier,
    Str(char),
    Escape(char),
    Unicode(char, u8),
    EndQuote,
    Slash,
    Comment,
    BlockComment,
    BlockCommentEnd,
}

pub struct Lexer {
    chars: Vec<char>,
    index: usize,
    line: usize,
    column: usize,
    mode: Mode,
    buf: String,
    text: Vec<u16>,
    unicode: String,
    queue: VecDeque<Token>,
    finished: bool,
}

fn is_symbol(c: char) -> bool {
    matches!(c, '(' | ')' | ',' | ':' | ';' | '=' | '[' | ']' | '{' | '}')
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_terminator(c: Option<char>) -> bool {
    match c {
        None | Some(' ' | '\n') => true,
        Some(c) => is_symbol(c),
    }
}

fn describe(c: char) -> String {
    let code = c as u32;
    if code > 0x20 { format!("U+{code:04X} (\"{c}\")") } else { format!("U+{code:04X}") }
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            index: 0,
            line: 1,
            column: 0,
            mode: Mode::Main,
            buf: String::new(),
            text: Vec::new(),
            unicode: String::new(),
            queue: VecDeque::new(),
            finished: false,
        }
    }

    pub fn next_token(&mut self) -> Result<Token, ParseError> {
        loop {
            if let Some(token) = self.queue.pop_front() {
                return Ok(token);
            }
            if self.finished {
                return Ok(Token { tok: Tok::Eof, line: self.line, column: self.column });
            }
            self.step()?;
        }
    }

    pub fn tokenize(source: &str) -> Result<Vec<Token>, ParseError> {
        let mut lexer = Self::new(source);
        let mut out = Vec::new();
        loop {
            let token = lexer.next_token()?;
            let eof = token.tok == Tok::Eof;
            out.push(token);
            if eof {
                return Ok(out);
            }
        }
    }

    fn emit(&mut self, tok: Tok) {
        if tok == Tok::Eof {
            self.finished = true;
        }
        self.queue.push_back(Token { tok, line: self.line, column: self.column });
    }

    fn err(&self, message: impl Into<String>) -> ParseError {
        ParseError::syntax(message, self.line, self.column)
    }

    fn unexpected(&self, c: Option<char>, eof: &str, suffix: &str) -> ParseError {
        match c {
            None => self.err(eof),
            Some(c) => self.err(format!("Unexpected character {}{suffix}", describe(c))),
        }
    }

    fn end_with(&mut self, tok: Tok, c: Option<char>) {
        self.emit(tok);
        self.buf.clear();
        match c {
            None => self.emit(Tok::Eof),
            Some(' ' | '\n') => self.mode = Mode::Main,
            Some(c) => {
                self.emit(Tok::Symbol(c));
                self.mode = Mode::Main;
            }
        }
    }

    fn take_int(&self, radix: u32) -> Result<Tok, ParseError> {
        i64::from_str_radix(&self.buf, radix)
            .map(Tok::Int)
            .map_err(|_| self.err(format!("Integer literal {} is out of range", self.buf)))
    }

    fn take_double(&self) -> Result<Tok, ParseError> {
        self.buf
            .parse::<f64>()
            .map(Tok::Double)
            .map_err(|_| self.err(format!("Invalid double literal {}", self.buf)))
    }

    fn push_text(&mut self, c: char) {
        let mut units = [0u16; 2];
        self.text.extend_from_slice(c.encode_utf16(&mut units));
    }

    fn step(&mut self) -> Result<(), ParseError> {
        let c = self.chars.get(self.index).copied();
        if let Some(ch) = c {
            if ch == '\n' {
                self.line += 1;
                self.column = 0;
            } else {
                self.column += 1;
            }
        }
        self.index += 1;
        match self.mode {
            Mode::Main => self.main(c),
            Mode::Minus => match c {
                Some(d) if d.is_ascii_digit() => {
                    self.buf.push(d);
                    self.mode = Mode::MinusInteger;
                    Ok(())
                }
                _ => Err(self.unexpected(c, "Unexpected end of file after minus sign", " after minus sign (expected digit)")),
            },
            Mode::Zero | Mode::MinusInteger | Mode::Integer => self.integer(c),
            Mode::IntegerOnly => match c {
                _ if is_terminator(c) => {
                    let tok = self.take_int(10)?;
                    self.end_with(tok, c);
                    Ok(())
                }
                Some('.') => {
                    let tok = self.take_int(10)?;
                    self.emit(tok);
                    self.buf.clear();
                    self.mode = Mode::Dot1;
                    Ok(())
                }
                Some(d) if d.is_ascii_digit() => {
                    self.buf.push(d);
                    Ok(())
                }
                _ => Err(self.unexpected(c, "", " in integer")),
            },
            Mode::NumericDot => match c {
                Some(d) if d.is_ascii_digit() => {
                    self.buf.push(d);
                    self.mode = Mode::Fraction;
                    Ok(())
                }
                _ => Err(self.unexpected(c, "Unexpected end of file after decimal point", " in fraction component")),
            },
            Mode::Fraction | Mode::Exponent => self.fraction(c),
            Mode::E | Mode::NegativeExponent => self.exponent_start(c),
            Mode::X | Mode::Hex => self.hex(c),
            Mode::Dot1 => self.dot1(c),
            Mode::Dot2 => match c {
                Some('.') => {
                    self.emit(Tok::TripleDot);
                    self.mode = Mode::Main;
                    Ok(())
                }
                _ => Err(self.unexpected(
                    c,
                    "Unexpected end of file inside \"...\" symbol",
                    " inside \"...\" symbol",
                )),
            },
            Mode::Identifier => self.identifier(c),
            Mode::Str(q) => self.string(c, q),
            Mode::Escape(q) => self.escape(c, q),
            Mode::Unicode(q, n) => match c {
                Some(h) if h.is_ascii_hexdigit() => {
                    self.unicode.push(h);
                    if n == 3 {
                        let unit = u16::from_str_radix(&self.unicode, 16).unwrap_or(0xFFFD);
                        self.text.push(unit);
                        self.unicode.clear();
                        self.mode = Mode::Str(q);
                    } else {
                        self.mode = Mode::Unicode(q, n + 1);
                    }
                    Ok(())
                }
                _ => Err(self.unexpected(c, "Unexpected end of file inside Unicode escape", " in Unicode escape")),
            },
            Mode::EndQuote => match c {
                None => {
                    self.emit(Tok::Eof);
                    Ok(())
                }
                Some(' ' | '\n') => {
                    self.mode = Mode::Main;
                    Ok(())
                }
                Some('.') => {
                    self.mode = Mode::Dot1;
                    Ok(())
                }
                Some(s) if is_symbol(s) => {
                    self.emit(Tok::Symbol(s));
                    self.mode = Mode::Main;
                    Ok(())
                }
                _ => Err(self.unexpected(c, "", " after end quote")),
            },
            Mode::Slash => match c {
                Some('*') => {
                    self.mode = Mode::BlockComment;
                    Ok(())
                }
                Some('/') => {
                    self.mode = Mode::Comment;
                    Ok(())
                }
                _ => Err(self.unexpected(
                    c,
                    "Unexpected end of file inside comment delimiter",
                    " inside comment delimiter",
                )),
            },
            Mode::Comment => {
                match c {
                    None => self.emit(Tok::Eof),
                    Some('\n') => self.mode = Mode::Main,
                    Some(_) => {}
                }
                Ok(())
            }
            Mode::BlockComment | Mode::BlockCommentEnd => match c {
                None => Err(self.err("Unexpected end of file in block comment")),
                Some('/') if self.mode == Mode::BlockCommentEnd => {
                    self.mode = Mode::Main;
                    Ok(())
                }
                Some('*') if self.mode == Mode::BlockComment => {
                    self.mode = Mode::BlockCommentEnd;
                    Ok(())
                }
                Some(_) => {
                    self.mode = Mode::BlockComment;
                    Ok(())
                }
            },
        }
    }

    fn main(&mut self, c: Option<char>) -> Result<(), ParseError> {
        let Some(ch) = c else {
            self.emit(Tok::Eof);
            return Ok(());
        };
        match ch {
            ' ' | '\n' => {}
            '>' => self.emit(Tok::Symbol('>')),
            s if is_symbol(s) => self.emit(Tok::Symbol(s)),
            '"' | '\'' => self.mode = Mode::Str(ch),
            '-' => {
                self.buf.push('-');
                self.mode = Mode::Minus;
            }
            '.' => self.mode = Mode::Dot1,
            '/' => self.mode = Mode::Slash,
            '0' => {
                self.buf.push('0');
                self.mode = Mode::Zero;
            }
            d if d.is_ascii_digit() => {
                self.buf.push(d);
                self.mode = Mode::Integer;
            }
            i if is_ident_start(i) => {
                self.buf.push(i);
                self.mode = Mode::Identifier;
            }
            _ => return Err(self.err(format!("Unexpected character {}", describe(ch)))),
        }
        Ok(())
    }
}
