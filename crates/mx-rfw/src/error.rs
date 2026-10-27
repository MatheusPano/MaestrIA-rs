use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorKind {
    Syntax,
    Unsupported,
    TooDeep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl ParseError {
    pub(crate) fn syntax(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self { kind: ParseErrorKind::Syntax, message: message.into(), line, column }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at line {} column {}.", self.message, self.line, self.column)
    }
}

impl std::error::Error for ParseError {}

pub(crate) fn dart_double(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    let abs = v.abs();
    if abs >= 1e21 || (abs != 0.0 && abs < 1e-6) {
        let s = format!("{v:e}");
        return match s.split_once('e') {
            Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
            _ => s,
        };
    }
    if v == v.trunc() {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}
