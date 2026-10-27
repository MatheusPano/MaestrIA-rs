use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Comment,
    String,
    Number,
    Literal,
    Keyword,
    Type,
    Function,
    Key,
    Variable,
    Meta,
    Added,
    Removed,
    Section,
    Bullet,
    Link,
    Emphasis,
    Strong,
}

pub type Spans = Vec<(Range<usize>, Tone)>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Code,
    Markup,
    Markdown,
    Diff,
}

struct Grammar {
    mode: Mode,
    line: &'static [&'static str],
    block: Option<(&'static str, &'static str)>,
    quotes: &'static str,
    multiline_quote: Option<char>,
    triple: bool,
    keys: bool,
    keywords: &'static [&'static str],
    fold_case: bool,
    dollar_vars: bool,
}

const SLASH: &[&str] = &["//"];
const HASH: &[&str] = &["#"];
const DASH: &[&str] = &["--"];
const C_BLOCK: Option<(&str, &str)> = Some(("/*", "*/"));

const JS: &[&str] = &[
    "as", "async", "await", "break", "case", "catch", "class", "const", "continue", "debugger", "declare", "default",
    "delete", "do", "else", "enum", "export", "extends", "false", "finally", "for", "from", "function", "get", "if",
    "implements", "import", "in", "instanceof", "interface", "let", "namespace", "new", "null", "of", "private",
    "protected", "public", "readonly", "return", "set", "static", "super", "switch", "this", "throw", "true", "try",
    "type", "typeof", "undefined", "var", "void", "while", "with", "yield",
];
const DART: &[&str] = &[
    "abstract", "as", "assert", "async", "await", "base", "break", "case", "catch", "class", "const", "continue",
    "covariant", "default", "deferred", "do", "dynamic", "else", "enum", "export", "extends", "extension",
    "external", "factory", "false", "final", "finally", "for", "get", "hide", "if", "implements", "import", "in",
    "interface", "is", "late", "library", "mixin", "new", "null", "on", "operator", "part", "required", "rethrow",
    "return", "sealed", "set", "show", "static", "super", "switch", "sync", "this", "throw", "true", "try", "typedef",
    "var", "void", "when", "while", "with", "widget", "yield",
];
const C_LIKE: &[&str] = &[
    "abstract", "as", "auto", "bool", "break", "case", "catch", "char", "class", "companion", "const", "continue",
    "data", "def", "default", "delete", "do", "double", "echo", "else", "elseif", "enum", "extends", "extension",
    "extern", "false", "final", "finally", "float", "for", "foreach", "fun", "func", "function", "guard", "if",
    "implements", "import", "in", "init", "inline", "instanceof", "int", "interface", "internal", "is", "lateinit",
    "let", "long", "namespace", "new", "nil", "null", "nullptr", "object", "open", "override", "package", "private",
    "protected", "protocol", "public", "return", "sealed", "self", "short", "signed", "sizeof", "static", "string",
    "struct", "super", "switch", "synchronized", "template", "this", "throw", "throws", "true", "try", "typealias",
    "typedef", "typename", "union", "unsigned", "using", "val", "var", "virtual", "void", "volatile", "when", "while",
];
const GO: &[&str] = &[
    "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "false", "for", "func",
    "go", "goto", "if", "import", "interface", "map", "nil", "package", "range", "return", "select", "struct",
    "switch", "true", "type", "var",
];
const RUST: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "static",
    "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while",
];
const PYTHON: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "case", "class", "continue", "def",
    "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda",
    "match", "nonlocal", "not", "or", "pass", "raise", "return", "self", "try", "while", "with", "yield",
];
const RUBY: &[&str] = &[
    "alias", "and", "begin", "break", "case", "class", "def", "do", "else", "elsif", "end", "ensure", "false", "for",
    "if", "in", "module", "next", "nil", "not", "or", "redo", "require", "rescue", "retry", "return", "self", "super",
    "then", "true", "undef", "unless", "until", "when", "while", "yield",
];
const SHELL: &[&str] = &[
    "break", "case", "continue", "declare", "do", "done", "echo", "elif", "else", "esac", "exit", "export", "fi",
    "for", "function", "if", "in", "local", "readonly", "return", "set", "source", "then", "unset", "until", "while",
];
const SQL: &[&str] = &[
    "add", "all", "alter", "and", "as", "asc", "begin", "between", "by", "case", "check", "commit", "create",
    "default", "delete", "desc", "distinct", "drop", "else", "end", "exists", "foreign", "from", "group", "having",
    "in", "index", "inner", "insert", "into", "is", "join", "key", "left", "like", "limit", "not", "null", "offset",
    "on", "or", "order", "outer", "primary", "references", "returning", "right", "rollback", "select", "set", "table",
    "then", "union", "unique", "update", "values", "view", "when", "where", "with",
];
const LUA: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in", "local", "nil",
    "not", "or", "repeat", "return", "then", "true", "until", "while",
];
const DOCKER: &[&str] = &[
    "add", "arg", "as", "cmd", "copy", "entrypoint", "env", "expose", "from", "healthcheck", "label", "onbuild", "run",
    "shell", "stopsignal", "user", "volume", "workdir",
];
const LITERALS: &[&str] = &["false", "no", "null", "off", "on", "true", "yes"];
const LITERAL_WORDS: &[&str] = &["False", "None", "True", "false", "nil", "null", "true", "undefined"];
const NONE: &[&str] = &[];

const fn code(line: &'static [&'static str], block: Option<(&'static str, &'static str)>, keywords: &'static [&'static str]) -> Grammar {
    Grammar {
        mode: Mode::Code,
        line,
        block,
        quotes: "\"'",
        multiline_quote: None,
        triple: false,
        keys: false,
        keywords,
        fold_case: false,
        dollar_vars: false,
    }
}

const fn markup(mode: Mode) -> Grammar {
    Grammar { mode, ..code(NONE, None, NONE) }
}

fn grammar(lang: &str) -> Option<Grammar> {
    let g = match lang {
        "javascript" | "typescript" => Grammar { quotes: "\"'`", multiline_quote: Some('`'), ..code(SLASH, C_BLOCK, JS) },
        "dart" => Grammar { triple: true, ..code(SLASH, C_BLOCK, DART) },
        "go" => Grammar { quotes: "\"'`", multiline_quote: Some('`'), ..code(SLASH, C_BLOCK, GO) },
        "rust" => Grammar { quotes: "\"", ..code(SLASH, C_BLOCK, RUST) },
        "java" | "kotlin" | "swift" | "c" | "cpp" | "csharp" | "objectivec" | "scala" | "groovy" | "gradle"
        | "protobuf" => Grammar { triple: true, ..code(SLASH, C_BLOCK, C_LIKE) },
        "php" => Grammar { dollar_vars: true, ..code(&["//", "#"], C_BLOCK, C_LIKE) },
        "css" | "scss" | "less" => code(SLASH, C_BLOCK, NONE),
        "python" => Grammar { triple: true, ..code(HASH, None, PYTHON) },
        "ruby" => code(HASH, None, RUBY),
        "bash" | "makefile" | "powershell" | "perl" | "nginx" => Grammar { dollar_vars: true, ..code(HASH, None, SHELL) },
        "r" | "elixir" => code(HASH, None, SHELL),
        "dockerfile" => Grammar { fold_case: true, ..code(HASH, None, DOCKER) },
        "yaml" => Grammar { keys: true, ..code(HASH, None, LITERALS) },
        "json" => Grammar { keys: true, ..code(SLASH, C_BLOCK, LITERALS) },
        "ini" | "properties" => Grammar { keys: true, ..code(&["#", ";"], None, LITERALS) },
        "graphql" => code(HASH, None, NONE),
        "sql" => Grammar { fold_case: true, ..code(DASH, C_BLOCK, SQL) },
        "lua" => code(DASH, Some(("--[[", "]]")), LUA),
        "haskell" => code(DASH, Some(("{-", "-}")), NONE),
        "xml" | "vue" => markup(Mode::Markup),
        "markdown" => markup(Mode::Markdown),
        "diff" => markup(Mode::Diff),
        _ => return None,
    };
    Some(g)
}

pub fn highlight(text: &str, lang: &str) -> Spans {
    let Some(g) = grammar(lang) else { return Vec::new() };
    match g.mode {
        Mode::Code => Lexer { text, at: 0, out: Vec::new() }.code(&g),
        Mode::Markup => Lexer { text, at: 0, out: Vec::new() }.markup(),
        Mode::Markdown => super::markdown::markdown(text),
        Mode::Diff => super::markdown::diff(text),
    }
}

struct Lexer<'a> {
    text: &'a str,
    at: usize,
    out: Spans,
}

fn ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '$'
}

fn ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

impl Lexer<'_> {
    fn rest(&self) -> &str {
        &self.text[self.at..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn push(&mut self, from: usize, tone: Tone) {
        if self.at > from {
            self.out.push((from..self.at, tone));
        }
    }

    fn until(&mut self, end: &str) {
        self.at = self.rest().find(end).map_or(self.text.len(), |i| self.at + i + end.len());
    }

    fn skip_line(&mut self) {
        self.at = self.rest().find('\n').map_or(self.text.len(), |i| self.at + i);
    }

    fn take_while(&mut self, f: impl Fn(char) -> bool) {
        let n = self.rest().find(|c: char| !f(c)).unwrap_or(self.rest().len());
        self.at += n;
    }

    fn quoted(&mut self, q: char, multiline: bool) {
        self.at += q.len_utf8();
        let mut escaped = false;
        while let Some(c) = self.peek() {
            if c == '\n' && !multiline {
                return;
            }
            self.at += c.len_utf8();
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                return;
            }
        }
    }

    fn followed_by_colon(&self) -> bool {
        let r = self.rest().trim_start_matches([' ', '\t']);
        r.starts_with(':') || r.starts_with('=')
    }

    fn code(mut self, g: &Grammar) -> Spans {
        let mut prev_ident = false;
        while let Some(c) = self.peek() {
            let from = self.at;
            let rest = self.rest();
            if let Some((open, close)) = g.block.filter(|(o, _)| rest.starts_with(o)) {
                self.at += open.len();
                self.until(close);
                self.push(from, Tone::Comment);
            } else if g.line.iter().any(|p| rest.starts_with(p)) {
                self.skip_line();
                self.push(from, Tone::Comment);
            } else if g.triple && (rest.starts_with("\"\"\"") || rest.starts_with("'''")) {
                let q = rest[..3].to_owned();
                self.at += 3;
                self.until(&q);
                self.push(from, Tone::String);
            } else if g.quotes.contains(c) {
                self.quoted(c, g.multiline_quote == Some(c));
                let tone = if g.keys && self.followed_by_colon() { Tone::Key } else { Tone::String };
                self.push(from, tone);
            } else if c.is_ascii_digit() && !prev_ident {
                self.take_while(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_');
                self.push(from, Tone::Number);
            } else if c == '@' {
                self.at += 1;
                self.take_while(|c| ident_char(c) || c == '.');
                self.push(from, Tone::Meta);
            } else if ident_start(c) {
                self.take_while(|c| ident_char(c) || (g.keys && c == '-'));
                let word = &self.text[from..self.at];
                let keyword = if g.fold_case {
                    g.keywords.iter().any(|k| k.eq_ignore_ascii_case(word))
                } else {
                    g.keywords.contains(&word)
                };
                let literal = if std::ptr::eq(g.keywords, LITERALS) { keyword } else { LITERAL_WORDS.contains(&word) };
                if g.keys && self.followed_by_colon() && !keyword {
                    self.push(from, Tone::Key);
                } else if literal {
                    self.push(from, Tone::Literal);
                } else if keyword {
                    self.push(from, Tone::Keyword);
                } else if g.dollar_vars && word.starts_with('$') {
                    self.push(from, Tone::Variable);
                } else if self.peek() == Some('(') && !g.keys {
                    self.push(from, Tone::Function);
                } else if word.starts_with(|c: char| c.is_uppercase()) && word.chars().any(char::is_lowercase) {
                    self.push(from, Tone::Type);
                }
                prev_ident = true;
                continue;
            } else {
                self.at += c.len_utf8();
            }
            prev_ident = false;
        }
        self.out
    }

    fn markup(mut self) -> Spans {
        while let Some(c) = self.peek() {
            let from = self.at;
            if self.rest().starts_with("<!--") {
                self.until("-->");
                self.push(from, Tone::Comment);
            } else if c == '<' {
                self.at += 1;
                self.take_while(|c| c == '/' || c == '!' || c == '?');
                self.push(from, Tone::Meta);
                let name = self.at;
                self.take_while(|c| ident_char(c) || c == '-' || c == ':' || c == '.');
                self.push(name, Tone::Variable);
                self.tag_body();
                let close = self.at;
                if self.peek() == Some('>') {
                    self.at += 1;
                    self.push(close, Tone::Meta);
                }
            } else {
                self.at += c.len_utf8();
            }
        }
        self.out
    }

    fn tag_body(&mut self) {
        while let Some(c) = self.peek() {
            let from = self.at;
            match c {
                '>' => return,
                '"' | '\'' => {
                    self.quoted(c, true);
                    self.push(from, Tone::String);
                }
                c if ident_start(c) => {
                    self.take_while(|c| ident_char(c) || c == '-' || c == ':' || c == '@');
                    self.push(from, Tone::Key);
                }
                _ => self.at += c.len_utf8(),
            }
        }
    }
}

#[cfg(test)]
#[path = "syntax_tests.rs"]
mod tests;
